use crate::gfx::{glyph, FrameBuf, Rgb};
use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

#[derive(Clone, Copy, Default)]
struct Vector {
    lateral: f32,
    height: f32,
    depth: f32,
}

impl Vector {
    fn new(lateral: f32, height: f32, depth: f32) -> Self {
        Self { lateral, height, depth }
    }

    fn subtract(self, other: Self) -> Self {
        Self::new(self.lateral - other.lateral, self.height - other.height, self.depth - other.depth)
    }

    fn cross(self, other: Self) -> Self {
        Self::new(
            self.height * other.depth - self.depth * other.height,
            self.depth * other.lateral - self.lateral * other.depth,
            self.lateral * other.height - self.height * other.lateral,
        )
    }

    fn dot(self, other: Self) -> f32 {
        self.lateral * other.lateral + self.height * other.height + self.depth * other.depth
    }

    fn normalize(self) -> Self {
        let length = self.dot(self).sqrt().max(0.0001);
        Self::new(self.lateral / length, self.height / length, self.depth / length)
    }

    fn rotate(self, angle: f32) -> Self {
        let (sine, cosine) = angle.sin_cos();
        Self::new(self.lateral * cosine + self.depth * sine, self.height, self.depth * cosine - self.lateral * sine)
    }
}

#[derive(Clone, Copy)]
enum Material {
    Paint,
    White,
    Carbon,
    Rubber,
    Alloy,
    Gold,
    Visor,
    Lamp,
    Shadow(f32),
}

#[derive(Clone, Copy)]
struct Wheel {
    center: Vector,
    radius: f32,
}

impl Wheel {
    fn phase(self, travel: f32) -> f32 {
        (travel / self.radius).rem_euclid(TAU)
    }

    fn rotate(self, point: Vector, phase: f32) -> Vector {
        let (sine, cosine) = phase.sin_cos();
        let height = point.height - self.center.height;
        let depth = point.depth - self.center.depth;
        Vector::new(point.lateral, self.center.height + height * cosine - depth * sine,
            self.center.depth + depth * cosine + height * sine)
    }
}

#[derive(Clone, Copy)]
enum Attachment {
    Chassis,
    Tyre(Wheel),
    Rotating(Wheel),
}

#[derive(Clone, Copy)]
struct Triangle {
    points: [Vector; 3],
    normals: [Vector; 3],
    material: Material,
    attachment: Attachment,
}

#[derive(Default)]
struct Mesh {
    triangles: Vec<Triangle>,
}

impl Mesh {
    fn triangle(&mut self, points: [Vector; 3], material: Material) {
        let normal = points[1].subtract(points[0]).cross(points[2].subtract(points[0])).normalize();
        self.triangles.push(Triangle { points, normals: [normal; 3], material, attachment: Attachment::Chassis });
    }

    fn quad(&mut self, points: [Vector; 4], material: Material) {
        self.triangle([points[0], points[1], points[2]], material);
        self.triangle([points[0], points[2], points[3]], material);
    }

    fn cuboid(&mut self, minimum: Vector, maximum: Vector, material: Material) {
        let points = [
            Vector::new(minimum.lateral, minimum.height, minimum.depth),
            Vector::new(maximum.lateral, minimum.height, minimum.depth),
            Vector::new(maximum.lateral, maximum.height, minimum.depth),
            Vector::new(minimum.lateral, maximum.height, minimum.depth),
            Vector::new(minimum.lateral, minimum.height, maximum.depth),
            Vector::new(maximum.lateral, minimum.height, maximum.depth),
            Vector::new(maximum.lateral, maximum.height, maximum.depth),
            Vector::new(minimum.lateral, maximum.height, maximum.depth),
        ];
        for indices in [[0, 3, 2, 1], [4, 5, 6, 7], [0, 4, 7, 3], [1, 2, 6, 5], [3, 7, 6, 2], [0, 1, 5, 4]] {
            self.quad(indices.map(|index| points[index]), material);
        }
    }

    fn loft(&mut self, sections: &[[f32; 5]], material: Material) {
        let rings: Vec<_> = sections.iter().map(|&[depth, half_width, bottom, shoulder, top]| [
            Vector::new(-half_width, bottom, depth),
            Vector::new(half_width, bottom, depth),
            Vector::new(half_width, shoulder, depth),
            Vector::new(half_width * 0.68, top, depth),
            Vector::new(-half_width * 0.68, top, depth),
            Vector::new(-half_width, shoulder, depth),
        ]).collect();
        for pair in rings.windows(2) {
            for index in 0..6 {
                let next = (index + 1) % 6;
                self.quad([pair[0][index], pair[0][next], pair[1][next], pair[1][index]], material);
            }
        }
        for (ring, reverse) in [(rings[0], true), (rings[rings.len() - 1], false)] {
            for index in 1..5 {
                let mut points = [ring[0], ring[index], ring[index + 1]];
                if reverse {
                    points.swap(1, 2);
                }
                self.triangle(points, material);
            }
        }
    }

    fn rod(&mut self, start: Vector, end: Vector, radius: f32, material: Material) {
        let direction = end.subtract(start).normalize();
        let reference = if direction.height.abs() < 0.9 { Vector::new(0.0, 1.0, 0.0) } else { Vector::new(1.0, 0.0, 0.0) };
        let first = direction.cross(reference).normalize();
        let second = direction.cross(first).normalize();
        let offset = |center: Vector, angle: f32| Vector::new(
            center.lateral + radius * (first.lateral * angle.cos() + second.lateral * angle.sin()),
            center.height + radius * (first.height * angle.cos() + second.height * angle.sin()),
            center.depth + radius * (first.depth * angle.cos() + second.depth * angle.sin()),
        );
        for index in 0..6 {
            let angle = index as f32 * TAU / 6.0;
            let next = (index + 1) as f32 * TAU / 6.0;
            self.quad([offset(start, angle), offset(start, next), offset(end, next), offset(end, angle)], material);
        }
    }

    fn wheel(&mut self, side: f32, depth: f32, width: f32, radius: f32) {
        let center = side * 0.83;
        let wheel = Wheel { center: Vector::new(center, radius, depth), radius };
        let start = self.triangles.len();
        let point = |lateral: f32, distance: f32, angle: f32| Vector::new(lateral, radius + distance * angle.sin(), depth + distance * angle.cos());
        let left = center - width / 2.0;
        let right = center + width / 2.0;
        for index in 0..24 {
            let angle = index as f32 * TAU / 24.0;
            let next = (index + 1) as f32 * TAU / 24.0;
            let normal = |angle: f32| Vector::new(0.0, angle.sin(), angle.cos());
            let points = [point(left, radius, angle), point(right, radius, angle), point(right, radius, next), point(left, radius, next)];
            let normals = [normal(angle), normal(angle), normal(next), normal(next)];
            for indices in [[0, 1, 2], [0, 2, 3]] {
                self.triangles.push(Triangle { points: indices.map(|index| points[index]), normals: indices.map(|index| normals[index]), material: Material::Rubber, attachment: Attachment::Tyre(wheel) });
            }
            for lateral in [left - 0.001, right + 0.001] {
                for (outer, inner, material) in [(radius, radius * 0.91, Material::Rubber), (radius * 0.91, radius * 0.50, Material::Carbon), (radius * 0.50, radius * 0.46, Material::Alloy), (radius * 0.46, radius * 0.18, Material::Carbon)] {
                    let mut points = [point(lateral, outer, angle), point(lateral, inner, angle), point(lateral, inner, next), point(lateral, outer, next)];
                    if lateral < center {
                        points.reverse();
                    }
                    self.quad(points, material);
                }
                self.triangle([point(lateral, 0.0, angle), point(lateral, radius * 0.18, angle), point(lateral, radius * 0.18, next)], Material::Gold);
            }
        }
        for triangle in &mut self.triangles[start..] {
            triangle.attachment = Attachment::Tyre(wheel);
        }
        let start = self.triangles.len();
        for lateral in [left - 0.003, right + 0.003] {
            for spoke in 0..6 {
                let angle = spoke as f32 * TAU / 6.0;
                self.quad([
                    point(lateral, radius * 0.16, angle - 0.18),
                    point(lateral, radius * 0.47, angle - 0.085),
                    point(lateral, radius * 0.47, angle + 0.085),
                    point(lateral, radius * 0.16, angle + 0.18),
                ], Material::Alloy);
            }
            self.quad([
                point(lateral, radius * 0.68, 0.10), point(lateral, radius * 0.87, 0.10),
                point(lateral, radius * 0.87, 0.21), point(lateral, radius * 0.68, 0.21),
            ], Material::Gold);
        }
        for triangle in &mut self.triangles[start..] {
            triangle.attachment = Attachment::Rotating(wheel);
        }
    }

    fn rear_lettering(&mut self) {
        let text = "SLOPRACER";
        let unit = 1.38 / (text.len() * 6 - 1) as f32;
        for (index, character) in text.chars().enumerate() {
            if let Some(rows) = glyph(character) {
                for (row, line) in rows.iter().enumerate() {
                    for (column, pixel) in line.bytes().enumerate() {
                        if pixel == b'#' {
                            let lateral = -0.69 + (index * 6 + column) as f32 * unit;
                            let height = 0.985 - row as f32 * 0.018;
                            self.quad([
                                Vector::new(lateral, height, -0.081),
                                Vector::new(lateral + unit * 0.97, height, -0.081),
                                Vector::new(lateral + unit * 0.97, height - 0.017, -0.081),
                                Vector::new(lateral, height - 0.017, -0.081),
                            ], Material::White);
                        }
                    }
                }
            }
        }
    }
}

fn car_mesh() -> &'static Mesh {
    static CAR: OnceLock<Mesh> = OnceLock::new();
    CAR.get_or_init(|| {
        let mut mesh = Mesh::default();
        mesh.loft(&[
            [0.22, 0.29, 0.14, 0.35, 0.43], [0.85, 0.43, 0.14, 0.43, 0.55],
            [1.55, 0.37, 0.16, 0.56, 0.66], [2.25, 0.29, 0.20, 0.52, 0.60],
            [3.40, 0.16, 0.26, 0.43, 0.48], [3.94, 0.10, 0.28, 0.34, 0.38],
        ], Material::Paint);
        mesh.loft(&[[0.45, 0.17, 0.40, 0.52, 0.59], [1.10, 0.26, 0.44, 0.75, 0.89], [1.52, 0.14, 0.48, 0.90, 1.06]], Material::Paint);
        mesh.cuboid(Vector::new(-0.085, 0.91, 1.39), Vector::new(0.085, 1.015, 1.53), Material::Carbon);
        for side in [-1.0, 1.0] {
            let mut pod = Mesh::default();
            pod.loft(&[[0.65, 0.17, 0.17, 0.34, 0.40], [1.10, 0.21, 0.16, 0.45, 0.52], [2.10, 0.21, 0.17, 0.43, 0.48], [2.32, 0.14, 0.19, 0.39, 0.44]], Material::Paint);
            for mut triangle in pod.triangles {
                for point in &mut triangle.points {
                    point.lateral += side * 0.50;
                }
                mesh.triangles.push(triangle);
            }
            let inner = (side * 0.50 - 0.14).min(side * 0.50 + 0.14);
            mesh.cuboid(Vector::new(inner, 0.22, 2.323), Vector::new(inner + 0.28, 0.38, 2.33), Material::Carbon);
            mesh.cuboid(Vector::new(side * 0.69 - 0.018, 0.31, 1.18), Vector::new(side * 0.69 + 0.018, 0.38, 2.02), Material::White);
            mesh.cuboid(Vector::new(side * 0.24 - 0.035, 0.30, 0.03), Vector::new(side * 0.24 + 0.035, 0.99, 0.18), Material::Carbon);
            for axle in [0.58, 3.27] {
                mesh.wheel(side, axle, if axle < 1.0 { 0.42 } else { 0.32 }, if axle < 1.0 { 0.36 } else { 0.34 });
                for mount in [-0.35, 0.35] {
                    mesh.rod(Vector::new(side * 0.30, 0.31, axle + mount), Vector::new(side * 0.83, 0.30, axle), 0.025, Material::Carbon);
                    mesh.rod(Vector::new(side * 0.26, 0.51, axle + mount), Vector::new(side * 0.80, 0.37, axle), 0.014, Material::Alloy);
                }
            }
            mesh.cuboid(Vector::new(side * 0.96 - 0.025, 0.12, 3.68), Vector::new(side * 0.96 + 0.025, 0.36, 4.13), Material::White);
            mesh.cuboid(Vector::new(side * 0.96 - 0.025, 0.82, -0.09), Vector::new(side * 0.96 + 0.025, 1.13, 0.35), Material::White);
            mesh.cuboid(Vector::new(side * 0.19 - 0.018, 0.54, 2.18), Vector::new(side * 0.19 + 0.018, 0.71, 2.28), Material::Carbon);
            mesh.cuboid(Vector::new(side * 0.32 - 0.065, 0.69, 2.16), Vector::new(side * 0.32 + 0.065, 0.76, 2.29), Material::Paint);
        }
        mesh.cuboid(Vector::new(-0.99, 0.91, -0.08), Vector::new(0.99, 1.02, 0.34), Material::Paint);
        mesh.cuboid(Vector::new(-0.89, 0.86, -0.05), Vector::new(0.89, 0.91, 0.22), Material::Carbon);
        mesh.cuboid(Vector::new(-0.97, 0.16, 3.78), Vector::new(0.97, 0.23, 4.14), Material::White);
        mesh.cuboid(Vector::new(-0.84, 0.23, 3.72), Vector::new(0.84, 0.30, 3.88), Material::Paint);
        mesh.cuboid(Vector::new(-0.07, 0.479, 2.55), Vector::new(0.07, 0.492, 3.39), Material::White);
        mesh.loft(&[[1.58, 0.20, 0.61, 0.64, 0.69], [1.88, 0.22, 0.61, 0.64, 0.69], [2.15, 0.17, 0.59, 0.62, 0.65]], Material::Carbon);
        for ring in 0..5 {
            let lower = ring as f32 * PI / 5.0;
            let upper = (ring + 1) as f32 * PI / 5.0;
            for segment in 0..12 {
                let angle = segment as f32 * TAU / 12.0;
                let next = (segment + 1) as f32 * TAU / 12.0;
                let point = |latitude: f32, longitude: f32| Vector::new(
                    latitude.sin() * longitude.cos() * 0.17,
                    0.81 + latitude.cos() * 0.19,
                    1.84 + latitude.sin() * longitude.sin() * 0.18,
                );
                let material = if ring == 2 && angle.sin() > 0.0 { Material::Visor } else if ring == 0 { Material::White } else { Material::Gold };
                mesh.quad([point(lower, angle), point(lower, next), point(upper, next), point(upper, angle)], material);
            }
        }
        mesh.cuboid(Vector::new(-0.075, 0.22, -0.11), Vector::new(0.075, 0.31, -0.04), Material::Lamp);
        mesh.cuboid(Vector::new(-0.32, 0.12, -0.04), Vector::new(0.32, 0.19, 0.40), Material::Carbon);
        for lateral in [-0.27, -0.13, 0.0, 0.13, 0.27] {
            mesh.cuboid(Vector::new(lateral - 0.015, 0.10, -0.10), Vector::new(lateral + 0.015, 0.22, 0.45), Material::Carbon);
        }
        mesh.rear_lettering();
        mesh
    })
}

#[derive(Clone, Copy)]
pub(super) struct GroundProfile {
    pub start: f32,
    pub heights: [f32; 6],
}

#[derive(Clone, Copy)]
pub(super) struct ModelView {
    pub focal_x: f32,
    pub focal_y: f32,
    pub horizon: f32,
    pub camera_height: f32,
    pub lateral: f32,
    pub distance: f32,
    pub grade: f32,
    pub ground: Option<GroundProfile>,
    pub lift: f32,
    pub travel: f32,
    pub yaw: f32,
    pub clip: f32,
    pub body: Rgb,
    pub light: f32,
    pub fog_color: Rgb,
    pub fog: f32,
}

impl ModelView {
    fn ground_height(&self, offset: f32) -> f32 {
        if let Some(ground) = self.ground {
            let position = ((offset - ground.start) / super::SEG).clamp(0.0, 5.0);
            let index = (position as usize).min(4);
            ground.heights[index] + (ground.heights[index + 1] - ground.heights[index]) * (position - index as f32)
        } else {
            offset * self.grade
        }
    }
}

#[derive(Clone, Copy)]
struct ScreenVertex {
    horizontal: f32,
    vertical: f32,
    inverse_depth: f32,
    light: f32,
    tyre_height: f32,
    tyre_depth: f32,
}

fn material_color(material: Material, body: Rgb) -> Rgb {
    match material {
        Material::Paint => body,
        Material::White => Rgb(232, 237, 231),
        Material::Carbon => Rgb(28, 33, 43),
        Material::Rubber => Rgb(42, 46, 54),
        Material::Alloy => Rgb(156, 170, 181),
        Material::Gold => Rgb(244, 186, 34),
        Material::Visor => Rgb(30, 67, 89),
        Material::Lamp => Rgb(255, 34, 22),
        Material::Shadow(_) => Rgb(8, 10, 17),
    }
}

pub(super) fn draw_car(frame: &mut FrameBuf, view: &ModelView) {
    static SHADOW: OnceLock<Mesh> = OnceLock::new();
    let shadow = SHADOW.get_or_init(|| {
        let mut mesh = Mesh::default();
        for (width, near, far, opacity) in [(0.83, 0.16, 3.94, 0.13), (0.67, 0.22, 3.85, 0.22)] {
            let height = 0.004 + opacity * 0.01;
            mesh.quad([
                Vector::new(-width, height, near), Vector::new(width, height, near),
                Vector::new(width * 0.75, height, far), Vector::new(-width * 0.75, height, far),
            ], Material::Shadow(opacity));
        }
        for axle in [0.58, 3.27] {
            for side in [-1.0, 1.0] {
                for (width, length, opacity) in [(0.26, 0.24, 0.18), (0.21, 0.15, 0.42)] {
                    let center = side * 0.83;
                    let height = 0.008 + opacity * 0.01;
                    mesh.quad([
                        Vector::new(center - width, height, axle - length), Vector::new(center + width, height, axle - length),
                        Vector::new(center + width, height, axle + length), Vector::new(center - width, height, axle + length),
                    ], Material::Shadow(opacity));
                }
            }
        }
        mesh
    });
    draw_model(frame, &ModelView { lift: 0.0, ..*view }, shadow, 1.94);
    draw_model(frame, view, car_mesh(), 1.94);
}

pub(super) fn draw_billboard(frame: &mut FrameBuf, view: &ModelView, index: usize) {
    static BOARDS: OnceLock<Vec<Mesh>> = OnceLock::new();
    let meshes = BOARDS.get_or_init(|| crate::sprites::BILLBOARD_TEXT.iter().map(|text| {
        let mut mesh = Mesh::default();
        for side in [-1.0, 1.0] {
            mesh.cuboid(Vector::new(side * 3.15 - 0.11, 0.0, -0.02), Vector::new(side * 3.15 + 0.11, 2.45, 0.22), Material::Alloy);
        }
        mesh.cuboid(Vector::new(-4.5, 2.0, -0.14), Vector::new(4.5, 5.625, 0.18), Material::White);
        mesh.cuboid(Vector::new(-4.30, 2.20, -0.18), Vector::new(4.30, 5.425, -0.14), Material::Paint);
        let count = text.chars().count();
        let unit = (7.6 / (count * 6 - 1) as f32).min(2.0 / 7.0);
        let start = -((count * 6 - 1) as f32) * unit / 2.0;
        for (index, character) in text.chars().enumerate() {
            if let Some(rows) = glyph(character) {
                for (row, line) in rows.iter().enumerate() {
                    for (column, pixel) in line.bytes().enumerate() {
                        if pixel == b'#' {
                            let left = start + (index * 6 + column) as f32 * unit;
                            let top = 3.8125 + 3.5 * unit - row as f32 * unit;
                            mesh.quad([
                                Vector::new(left, top, -0.185), Vector::new(left + unit, top, -0.185),
                                Vector::new(left + unit, top - unit, -0.185), Vector::new(left, top - unit, -0.185),
                            ], Material::White);
                        }
                    }
                }
            }
        }
        mesh
    }).collect());
    draw_model(frame, view, &meshes[index], 0.0);
}

fn draw_model(frame: &mut FrameBuf, view: &ModelView, mesh: &Mesh, pivot: f32) {
    let center_depth = view.distance + pivot * view.yaw.cos().abs();
    let sun = Vector::new(-0.45, 0.82, -0.35).normalize();
    let mut triangles = Vec::with_capacity(mesh.triangles.len());
    let mut bounds = [frame.w as f32, frame.h as f32, 0.0f32, 0.0f32];
    for triangle in &mesh.triangles {
        let mut points = [ScreenVertex { horizontal: 0.0, vertical: 0.0, inverse_depth: 0.0, light: 0.0, tyre_height: 0.0, tyre_depth: 0.0 }; 3];
        let mut visible = true;
        for (index, point) in triangle.points.iter().enumerate() {
            let mut point = *point;
            let mut normal = triangle.normals[index];
            let mut tyre_height = 0.0;
            let mut tyre_depth = 0.0;
            match triangle.attachment {
                Attachment::Rotating(wheel) => {
                    let phase = wheel.phase(view.travel);
                    point = wheel.rotate(point, phase);
                    normal = Wheel { center: Vector::default(), ..wheel }.rotate(normal, phase);
                }
                Attachment::Tyre(wheel) => {
                    tyre_height = (point.height - wheel.center.height) / wheel.radius;
                    tyre_depth = (point.depth - wheel.center.depth) / wheel.radius;
                }
                Attachment::Chassis => {}
            }
            let point = Vector::new(point.lateral, point.height, point.depth - pivot).rotate(view.yaw);
            let depth = center_depth + point.depth;
            if depth < 0.3 {
                visible = false;
                break;
            }
            let height = point.height + view.lift + view.ground_height(depth - view.distance);
            let normal = normal.rotate(view.yaw);
            let diffuse = normal.dot(sun).max(0.0);
            let highlight = diffuse.powi(10) * if matches!(triangle.material, Material::Rubber | Material::Carbon) { 0.06 } else { 0.20 };
            points[index] = ScreenVertex {
                horizontal: frame.w as f32 * 0.5 + (view.lateral + point.lateral) * view.focal_x / depth,
                vertical: view.horizon + (view.camera_height - height) * view.focal_y / depth,
                inverse_depth: 1.0 / depth,
                light: (0.50 + diffuse * 0.60 + highlight) * view.light.max(0.40),
                tyre_height,
                tyre_depth,
            };
        }
        if visible {
            for point in &points {
                bounds[0] = bounds[0].min(point.horizontal);
                bounds[1] = bounds[1].min(point.vertical);
                bounds[2] = bounds[2].max(point.horizontal);
                bounds[3] = bounds[3].max(point.vertical);
            }
            triangles.push((points, triangle.material, triangle.attachment));
        }
    }
    let left = bounds[0].floor().max(0.0) as usize;
    let top = bounds[1].floor().max(0.0) as usize;
    let right = bounds[2].ceil().min(frame.w as f32).max(0.0) as usize;
    let bottom = bounds[3].ceil().min(view.clip).min(frame.h as f32).max(0.0) as usize;
    if left >= right || top >= bottom {
        return;
    }
    let width = right - left;
    let mut depth_buffer = vec![0.0f32; width * (bottom - top)];
    for (points, material, attachment) in triangles {
        let edge = |first: ScreenVertex, second: ScreenVertex, horizontal: f32, vertical: f32| {
            (second.horizontal - first.horizontal) * (vertical - first.vertical) - (second.vertical - first.vertical) * (horizontal - first.horizontal)
        };
        let area = edge(points[0], points[1], points[2].horizontal, points[2].vertical);
        if area.abs() < 0.001 {
            continue;
        }
        let min_x = points.iter().map(|point| point.horizontal).fold(f32::INFINITY, f32::min).floor().max(left as f32) as usize;
        let max_x = points.iter().map(|point| point.horizontal).fold(f32::NEG_INFINITY, f32::max).ceil().min(right as f32).max(0.0) as usize;
        let min_y = points.iter().map(|point| point.vertical).fold(f32::INFINITY, f32::min).floor().max(top as f32) as usize;
        let max_y = points.iter().map(|point| point.vertical).fold(f32::NEG_INFINITY, f32::max).ceil().min(bottom as f32).max(0.0) as usize;
        let base_color = material_color(material, view.body);
        for pixel_y in min_y..max_y {
            for pixel_x in min_x..max_x {
                let horizontal = pixel_x as f32 + 0.5;
                let vertical = pixel_y as f32 + 0.5;
                let weights = [
                    edge(points[1], points[2], horizontal, vertical) / area,
                    edge(points[2], points[0], horizontal, vertical) / area,
                    edge(points[0], points[1], horizontal, vertical) / area,
                ];
                if weights.iter().any(|&weight| weight < -0.0001) {
                    continue;
                }
                let inverse_depth: f32 = (0..3).map(|index| weights[index] * points[index].inverse_depth).sum();
                let buffer_index = (pixel_y - top) * width + pixel_x - left;
                if inverse_depth <= depth_buffer[buffer_index] {
                    continue;
                }
                depth_buffer[buffer_index] = inverse_depth;
                let brightness: f32 = (0..3).map(|index| weights[index] * points[index].light * points[index].inverse_depth).sum::<f32>() / inverse_depth;
                let mut color = if matches!(material, Material::Lamp) { base_color } else { base_color.scale(brightness) };
                if let Attachment::Tyre(wheel) = attachment {
                    if matches!(material, Material::Rubber) {
                        let height: f32 = (0..3).map(|index| weights[index] * points[index].tyre_height * points[index].inverse_depth).sum::<f32>() / inverse_depth;
                        let depth: f32 = (0..3).map(|index| weights[index] * points[index].tyre_depth * points[index].inverse_depth).sum::<f32>() / inverse_depth;
                        if ((height.atan2(depth) + wheel.phase(view.travel)) * 12.0).sin() > 0.80 {
                            color = color.scale(0.68);
                        }
                    }
                }
                let color = if let Material::Shadow(opacity) = material {
                    frame.get(pixel_x, pixel_y).lerp(base_color, opacity * (1.0 - view.fog))
                } else {
                    color.lerp(view.fog_color, view.fog)
                };
                frame.set(pixel_x as i32, pixel_y as i32, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slopracer_lettering_fits_the_rear_wing() {
        let mut mesh = Mesh::default();
        mesh.rear_lettering();
        assert!(!mesh.triangles.is_empty());
        for point in mesh.triangles.iter().flat_map(|triangle| triangle.points) {
            assert!((-0.69..=0.69).contains(&point.lateral));
            assert!((0.85..=1.0).contains(&point.height));
        }
    }

    fn test_view() -> ModelView {
        ModelView {
            focal_x: 240.0, focal_y: 240.0, horizon: 70.0, camera_height: 2.8,
            lateral: 0.0, distance: 4.6, grade: 0.0, yaw: 0.35, clip: 300.0,
            ground: None, lift: 0.0, travel: 0.0,
            body: Rgb(220, 35, 45), light: 1.0, fog_color: Rgb(70, 75, 80), fog: 0.0,
        }
    }

    #[test]
    fn all_tyres_touch_the_road_at_every_rotation_phase() {
        for side in [-1.0, 1.0] {
            for (axle, radius) in [(0.58, 0.36), (3.27, 0.34)] {
                let mut mesh = Mesh::default();
                mesh.wheel(side, axle, 0.4, radius);
                let minimum = mesh.triangles.iter().filter(|triangle| matches!(triangle.attachment, Attachment::Tyre(_)))
                    .flat_map(|triangle| triangle.points).map(|point| point.height).fold(f32::INFINITY, f32::min);
                assert!(minimum.abs() < 0.00001, "axle {axle}: tyre bottom {minimum}");
                for grade in [-0.25, 0.0, 0.25] {
                    for yaw in [-0.09, 0.0, 0.09, PI / 2.0, PI] {
                        let view = ModelView { grade, yaw, ..test_view() };
                        let contact = Vector::new(side * 0.83, minimum, axle - 1.94).rotate(yaw);
                        let depth = view.distance + 1.94 * yaw.cos().abs() + contact.depth;
                        let tyre_bottom = contact.height + view.ground_height(depth - view.distance);
                        let road = (depth - view.distance) * grade;
                        assert!((tyre_bottom - road).abs() < 0.00001);
                    }
                }
            }
        }
    }

    #[test]
    fn ground_profile_uses_the_same_segment_interpolation_as_the_road() {
        let view = ModelView { ground: Some(GroundProfile {
            start: -2.6, heights: [-0.2, -0.1, 0.3, 0.1, -0.3, -0.5],
        }), ..test_view() };
        for (offset, height) in [(-2.6, -0.2), (-1.6, -0.15), (0.4, 0.1), (1.4, 0.3), (2.4, 0.2), (4.4, -0.1)] {
            assert!((view.ground_height(offset) - height).abs() < 0.00001);
        }
    }

    #[test]
    fn wheel_rotation_rolls_forward_and_scales_with_distance_and_radius() {
        for radius in [0.34, 0.36] {
            let wheel = Wheel { center: Vector::new(0.83, radius, 0.58), radius };
            let top = Vector::new(0.83, 2.0 * radius, 0.58);
            let rotated = wheel.rotate(top, wheel.phase(radius * PI / 2.0));
            assert!((rotated.height - radius).abs() < 0.00001);
            assert!((rotated.depth - (0.58 + radius)).abs() < 0.00001);
            assert!((wheel.phase(0.05) * 2.0 - wheel.phase(0.10)).abs() < 0.00001);
            assert!(wheel.phase(TAU * radius).abs() < 0.00001);
        }
    }

    #[test]
    fn wheel_pixels_move_but_the_body_and_stationary_car_do_not() {
        let render = |travel| {
            let mut frame = FrameBuf::new();
            frame.resize(480, 300);
            frame.fill_rect(0, 0, 480, 300, Rgb(70, 75, 80));
            draw_car(&mut frame, &ModelView { travel, ..test_view() });
            frame
        };
        let first = render(0.0);
        let stopped = render(0.0);
        let moving = render(0.11);
        let mut mask = FrameBuf::new();
        mask.resize(480, 300);
        let wheels = Mesh { triangles: car_mesh().triangles.iter().filter(|triangle| !matches!(triangle.attachment, Attachment::Chassis)).copied().collect() };
        draw_model(&mut mask, &test_view(), &wheels, 1.94);
        let mut changed = 0;
        for pixel_y in 0..300 {
            for pixel_x in 0..480 {
                assert_eq!(first.get(pixel_x, pixel_y), stopped.get(pixel_x, pixel_y));
                if first.get(pixel_x, pixel_y) != moving.get(pixel_x, pixel_y) {
                    assert_ne!(mask.get(pixel_x, pixel_y), Rgb::default(), "body changed at {pixel_x},{pixel_y}");
                    changed += 1;
                }
            }
        }
        assert!(changed > 30, "only {changed} animated pixels");
    }

    #[test]
    fn detailed_mesh_has_finite_geometry_and_required_materials() {
        let mesh = car_mesh();
        assert!(mesh.triangles.len() > 1500);
        for triangle in &mesh.triangles {
            for point in triangle.points.iter().chain(triangle.normals.iter()) {
                assert!(point.lateral.is_finite() && point.height.is_finite() && point.depth.is_finite());
            }
        }
        assert!(mesh.triangles.iter().any(|triangle| matches!(triangle.material, Material::Rubber)));
        assert!(mesh.triangles.iter().any(|triangle| matches!(triangle.material, Material::White)));
    }

    #[test]
    fn body_top_normals_face_the_sun_instead_of_the_road() {
        let mut mesh = Mesh::default();
        mesh.loft(&[[0.0, 0.5, 0.0, 0.8, 1.0], [2.0, 0.5, 0.0, 0.8, 1.0]], Material::Paint);
        let top_faces: Vec<_> = mesh.triangles.iter().filter(|triangle| triangle.points.iter().all(|point| point.height > 0.99)).collect();
        assert_eq!(top_faces.len(), 2);
        assert!(top_faces.iter().all(|triangle| triangle.normals[0].height > 0.99));
    }

    #[test]
    fn offscreen_models_are_safe_and_billboard_text_is_visible() {
        let clear = Rgb(9, 11, 17);
        let mut frame = FrameBuf::new();
        frame.resize(240, 160);
        frame.fill_rect(0, 0, 240, 160, clear);
        let mut view = ModelView {
            focal_x: 96.0, focal_y: 96.0, horizon: 60.0, camera_height: 3.5,
            lateral: 1000.0, distance: 10.0, grade: 0.0, yaw: 0.0, clip: 160.0,
            ground: None, lift: 0.0, travel: 0.0,
            body: Rgb(20, 120, 80), light: 1.0, fog_color: clear, fog: 0.0,
        };
        draw_car(&mut frame, &view);
        for pixel_y in 0..frame.h {
            assert!(frame.row_mut(pixel_y).iter().all(|&color| color == clear));
        }
        view.lateral = 0.0;
        view.distance = -10.0;
        draw_car(&mut frame, &view);
        for pixel_y in 0..frame.h {
            assert!(frame.row_mut(pixel_y).iter().all(|&color| color == clear));
        }
        view.distance = 10.0;
        draw_billboard(&mut frame, &view, 3);
        let mut lettering = 0;
        for pixel_y in 0..frame.h {
            lettering += frame.row_mut(pixel_y).iter().filter(|color| color.0 > 120 && color.1 > 120 && color.2 > 120).count();
        }
        assert!(lettering > 50);
    }

    #[test]
    fn car_renders_all_angles_and_respects_crest_clipping() {
        let clear = Rgb(9, 11, 17);
        for yaw in [0.0, 0.3, PI / 2.0, PI, -PI / 2.0] {
            let mut frame = FrameBuf::new();
            frame.resize(240, 160);
            frame.fill_rect(0, 0, 240, 160, clear);
            draw_car(&mut frame, &ModelView {
                focal_x: 96.0, focal_y: 96.0, horizon: 60.0, camera_height: 2.8,
                lateral: 0.0, distance: 4.6, grade: 0.0, yaw, clip: 105.0,
                ground: None, lift: 0.0, travel: 0.0,
                body: Rgb(240, 35, 45), light: 1.0, fog_color: clear, fog: 0.0,
            });
            let mut changed = 0;
            for pixel_y in 0..frame.h {
                for pixel_x in 0..frame.w {
                    if pixel_y >= 105 {
                        assert_eq!(frame.get(pixel_x, pixel_y), clear);
                    } else if frame.get(pixel_x, pixel_y) != clear {
                        changed += 1;
                    }
                }
            }
            assert!(changed > 10, "yaw {yaw}");
        }
    }
}
