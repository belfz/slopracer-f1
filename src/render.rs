//! Racing scene with textured terrain, atmospheric backdrops and perspective
//! car meshes. Road projection and scenery placement share the race geometry.

use crate::gfx::{FrameBuf, Rgb};
use crate::sprites::Bitmap;
use crate::track::{Backdrop, Palette, Track, ROAD_HALF};
use std::f32::consts::TAU;

#[path = "motorsport.rs"]
mod motorsport;

#[path = "night_city.rs"]
mod night_city;

/// Distance (m) from the camera to the ground at the bottom screen row.
pub const Z_NEAR: f32 = 4.0;
/// Distance from the camera to the player's car.
pub const PLAYER_Z: f32 = 4.6;
const Z_START: f32 = 0.8;
const SEG: f32 = 2.0;
const DRAW_DIST: f32 = 360.0;
const STRIPE: f32 = 6.0;
const LANE: f32 = ROAD_HALF / 3.0;
const HORIZON: f32 = 0.42;
/// Neon ground grid spacing and line width (m).
const GRID: f32 = 8.0;
const GRID_LINE: f32 = 0.35;

pub struct Camera {
    pub d: f32,
    pub x: f32,
    /// Horizontal scroll of the background, in screen widths.
    pub bg: f32,
}

pub struct SpriteInst {
    pub z: f32,
    pub x: f32,
    pub width_m: f32,
    pub bmp: &'static Bitmap,
    pub body: Rgb,
    pub flip: bool,
    /// Horizontal lean of the top rows (car steering), in bitmap pixels.
    pub skew: i32,
    /// Vertical offset in metres (bumps).
    pub lift: f32,
    pub shadow: bool,
}

#[derive(Clone, Copy)]
struct Proj {
    z: f32,
    sy: f32,
    cx: f32,
    xo: f32,
    y: f32,
}

fn hash(n: u32) -> u32 {
    let mut x = n.wrapping_mul(0x9E37_79B9);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^ (x >> 13)
}

fn hashf(n: u32) -> f32 {
    (hash(n) % 10_000) as f32 / 10_000.0
}

const BAYER: [f32; 16] = [0., 8., 2., 10., 12., 4., 14., 6., 3., 11., 1., 9., 15., 7., 13., 5.];

#[inline]
fn bayer(x: usize, y: usize) -> f32 {
    BAYER[(y & 3) * 4 + (x & 3)] / 16.0 - 0.5
}

/// Screen geometry shared by background, road and sprites.
struct View {
    w: usize,
    h: usize,
    hz: f32,
    focal_y: f32,
    /// Pixel width / pixel height.
    aspect: f32,
}

pub fn render_world(fb: &mut FrameBuf, track: &Track, cam: &Camera, sprites: &mut [SpriteInst], time: f32, aspect: f32) {
    let (w, h) = (fb.w, fb.h);
    if w < 8 || h < 8 {
        return;
    }
    let focal_x = 0.4 * w as f32;
    let v = View { w, h, hz: h as f32 * HORIZON, focal_y: focal_x * aspect, aspect };
    let pal = &track.palette;

    draw_backdrop(fb, &v, pal, cam.bg, time);

    // --- Project segment boundaries ---------------------------------------
    let cam_h = (Z_NEAR * (h as f32 - v.hz) / v.focal_y).clamp(1.6, 5.0);
    let cam_y = track.height_at(cam.d) + cam_h;
    let n = (DRAW_DIST / SEG) as usize;
    let mut pts = Vec::with_capacity(n + 1);
    let (mut dx, mut xo) = (0.0f32, 0.0f32);
    for i in 0..=n {
        let z = Z_START + i as f32 * SEG;
        if i > 0 {
            let c = track.curve_at(cam.d + z - SEG * 0.5);
            xo += dx * SEG + 0.5 * c * SEG * SEG;
            dx += c * SEG;
        }
        let y = track.height_at(cam.d + z);
        pts.push(Proj {
            z,
            sy: v.hz + (cam_y - y) * v.focal_y / z,
            cx: w as f32 / 2.0 + (xo - cam.x) * focal_x / z,
            xo,
            y,
        });
    }
    let fog_at = |z: f32| ((z - 30.0) / (DRAW_DIST - 30.0)).clamp(0.0, 1.0).powf(1.2) * 0.92;

    // --- Road, near to far, with a running clip line for hill crests -------
    let mut clip = h as f32;
    let mut seg_clip = vec![h as f32; n + 1];
    for i in 0..n {
        seg_clip[i] = clip;
        let (a, b) = (pts[i], pts[i + 1]);
        if b.sy >= a.sy || b.sy >= clip {
            continue;
        }
        let y0 = (b.sy - 0.5).ceil().max(0.0) as i32;
        let y1 = (a.sy.min(clip) - 0.5).ceil().min(h as f32) as i32;
        let iz_at = |yy: f32| {
            let t = (a.sy - yy) / (a.sy - b.sy);
            (1.0 / a.z) * (1.0 - t) + (1.0 / b.z) * t
        };
        for y in y0..y1 {
            let yc = y as f32 + 0.5;
            let t = (a.sy - yc) / (a.sy - b.sy);
            let iz = iz_at(yc);
            let z = 1.0 / iz;
            let cx = a.cx + (b.cx - a.cx) * t;
            // Depth covered by this pixel row, for filtering grid lines.
            let span = (1.0 / iz_at(y as f32).max(1e-4) - 1.0 / iz_at(y as f32 + 1.0).max(1e-4)).abs();
            draw_road_row(fb, y as usize, cx, focal_x * iz, cam.d + z, span, track, fog_at(z));
        }
        clip = clip.min(b.sy);
    }
    seg_clip[n] = clip;

    // --- Sprites, far to near ----------------------------------------------
    sprites.sort_by(|a, b| b.z.partial_cmp(&a.z).unwrap());
    let z_end = Z_START + n as f32 * SEG;
    for s in sprites.iter() {
        if s.z < Z_START || s.z >= z_end {
            continue;
        }
        let f = (s.z - Z_START) / SEG;
        let i = (f as usize).min(n - 1);
        let t = f - i as f32;
        let (a, b) = (pts[i], pts[i + 1]);
        let xo = a.xo + (b.xo - a.xo) * t;
        let road_y = a.y + (b.y - a.y) * t;
        let y = road_y + s.lift;
        let base = v.hz + (cam_y - y) * v.focal_y / s.z;
        let sx = w as f32 / 2.0 + (xo + s.x - cam.x) * focal_x / s.z;
        let wpx = s.width_m * focal_x / s.z;
        let hpx = s.width_m * s.bmp.h as f32 / s.bmp.w as f32 * v.focal_y / s.z;
        let clip_y = seg_clip[i];
        let art = crate::sprites::get();
        let car_angle = if std::ptr::eq(s.bmp, &art.car_rear) {
            Some(0.0)
        } else if std::ptr::eq(s.bmp, &art.car_front) {
            Some(std::f32::consts::PI)
        } else if std::ptr::eq(s.bmp, &art.car_side) {
            Some(if s.flip { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 })
        } else {
            None
        };
        if s.shadow && !(car_angle.is_some() && wpx >= 4.0) {
            let ry = (wpx * 0.09 * v.aspect).max(1.0);
            let sy = base - ry * 0.3;
            if sy < clip_y {
                for (size, opacity) in [(1.16, 0.07), (1.0, 0.13), (0.82, 0.19)] {
                    fb.ellipse(sx, sy, wpx * 0.56 * size, ry * size, Rgb(8, 12, 20), opacity);
                }
            }
        }
        if let Some(angle) = car_angle {
            if wpx >= 4.0 {
                let start = i.saturating_sub(1).min(n - 5);
                let ground = motorsport::GroundProfile {
                    start: pts[start].z - s.z,
                    heights: std::array::from_fn(|offset| pts[start + offset].y - road_y),
                };
                motorsport::draw_car(fb, &motorsport::ModelView {
                    focal_x, focal_y: v.focal_y, horizon: v.hz,
                    camera_height: cam_y - road_y, lateral: xo + s.x - cam.x,
                    distance: s.z, grade: (b.y - a.y) / SEG,
                    ground: Some(ground), lift: s.lift, travel: cam.d + s.z,
                    yaw: car_yaw(angle, (b.xo - a.xo) / SEG, s.skew),
                    clip: clip_y, body: s.body, light: pal.light,
                    fog_color: pal.fog, fog: fog_at(s.z),
                });
                continue;
            }
        }
        if wpx > 8.0 {
            if let Some(index) = art.billboards.iter().position(|bitmap| std::ptr::eq(s.bmp, bitmap)) {
                motorsport::draw_billboard(fb, &motorsport::ModelView {
                    focal_x, focal_y: v.focal_y, horizon: v.hz,
                    camera_height: cam_y - y, lateral: xo + s.x - cam.x,
                    distance: s.z, grade: 0.0, yaw: s.x.signum() * 0.25,
                    ground: None, lift: 0.0, travel: 0.0,
                    clip: clip_y, body: s.body, light: pal.light,
                    fog_color: pal.fog, fog: fog_at(s.z),
                }, index);
                continue;
            }
        }
        draw_sprite(fb, s, sx, base, wpx, hpx, clip_y, pal.light, pal.fog, fog_at(s.z));
    }
}

fn car_yaw(angle: f32, road_direction: f32, steering: i32) -> f32 {
    angle + road_direction.atan() + steering as f32 * 0.045
}

#[allow(clippy::too_many_arguments)]
fn draw_road_row(fb: &mut FrameBuf, y: usize, cx: f32, scale: f32, d: f32, span: f32, track: &Track, fog: f32) {
    let pal = &track.palette;
    let stripe = ((d / STRIPE).floor() as i64).rem_euclid(2) as usize;
    let f = |c: Rgb| c.lerp(pal.fog, fog);
    let midnight = pal.backdrop == Backdrop::Midnight;
    let grass_base = if pal.snow || pal.grid.is_some() || midnight { pal.grass[stripe] } else { pal.grass[stripe].lerp(Rgb(91, 110, 60), 0.32) };
    let grass = f(grass_base);
    let road_base = pal.road[0].lerp(pal.road[1], 0.5).lerp(pal.road[stripe], 0.18);
    let road = f(road_base);
    let rumble = f(pal.rumble[stripe]);
    let lane = f(pal.lane);
    let (white, black) = (f(Rgb(236, 236, 236)), f(Rgb(22, 22, 22)));
    let dm = d.rem_euclid(track.length);
    let start_line = dm < 3.0;
    // Painted grid slots just behind the line.
    let grid_mark = (track.length - dm) < 75.0 && (track.length - dm) % 12.0 < 0.5;
    let dashes = stripe == 0;
    let mpp = 1.0 / scale;
    let detail = (1.0 - mpp / 0.24).clamp(0.0, 1.0) * (1.0 - fog);
    let texture_row = (d * 12.0).floor() as i64 as u32;
    let grass_row = (d * 2.0).floor() as i64 as u32;
    let rubber_amount = 0.045 + track.curve_at(d).abs().min(0.01) * 4.0;
    // Neon grid: the share of this row's depth range covered by cross lines.
    let grid = pal.grid.map(|g| {
        let covered = |x: f32| {
            let y = x + GRID_LINE / 2.0;
            (y / GRID).floor() * GRID_LINE + y.rem_euclid(GRID).min(GRID_LINE)
        };
        let span = span.max(0.02);
        let cross = ((covered(d + span / 2.0) - covered(d - span / 2.0)) / span).clamp(0.0, 1.0);
        (g.lerp(pal.fog, fog * 0.8), cross, grass)
    });

    let color_at = |rel: f32| -> Rgb {
        let a = rel.abs();
        if a <= ROAD_HALF {
            if start_line {
                if (rel.floor() as i64 + dm.floor() as i64).rem_euclid(2) == 0 { white } else { black }
            } else if a > ROAD_HALF - 0.25 || (dashes && (a - LANE).abs() < 0.12) {
                lane
            } else if grid_mark && (1.2..3.2).contains(&a) {
                white
            } else {
                let cell = (rel * 12.0).floor() as i32 as u32;
                let grain = (hash(cell.wrapping_mul(73_856_093) ^ texture_row.wrapping_mul(19_349_663)) & 255) as f32 / 255.0 - 0.5;
                let groove = (1.0 - ((a - 2.1) / 0.40).abs()).max(0.0);
                let rubber = groove * rubber_amount;
                let asphalt = road.scale(1.0 + grain * 0.11 * detail - rubber * (1.0 - fog));
                if midnight {
                    let neon = if rel < 0.0 { Rgb(220, 50, 150) } else { Rgb(45, 175, 215) };
                    let streak = (1.0 - ((a - 3.5) / 1.3).abs()).max(0.0);
                    let pulse = (d * 0.19635).sin() * 0.5 + 0.5;
                    let broken = 0.5 + 0.5 * (d * 8.0 + rel * 3.0).sin().abs();
                    asphalt.lerp(f(neon), streak * pulse * broken * 0.32 * (1.0 - fog))
                } else {
                    asphalt
                }
            }
        } else if a <= ROAD_HALF + 0.9 {
            let bevel = ((a - ROAD_HALF) / 0.9 * 3.0).floor() / 3.0;
            rumble.scale(1.06 - bevel * 0.18)
        } else if midnight {
            let seam = rel.rem_euclid(2.0) < 0.05 || d.rem_euclid(4.0) < 0.06;
            let curb = a < ROAD_HALF + 1.2;
            let tile = if curb { Rgb(72, 85, 108) } else if seam { Rgb(13, 17, 30) } else { pal.grass[stripe] };
            f(tile)
        } else if let Some((line, cross, ground)) = grid {
            let along = (rel.rem_euclid(GRID) < GRID_LINE) as u8 as f32;
            ground.lerp(line, along.max(cross))
        } else {
            let cell = (rel * 12.0).floor() as i32 as u32;
            let grain = (hash(cell.wrapping_mul(83_492_791) ^ texture_row.wrapping_mul(29_765_719)) & 255) as f32 / 255.0 - 0.5;
            let patch = (hash(((rel * 0.5).floor() as i32 as u32) ^ grass_row) & 255) as f32 / 255.0 - 0.5;
            if a < ROAD_HALF + 1.35 && !pal.snow {
                f(Rgb(140, 133, 104)).scale(1.0 + grain * 0.18 * detail)
            } else {
                let strength = if pal.snow { 0.035 } else { 0.10 };
                grass.scale(1.0 + (grain * strength + patch * 0.025) * detail)
            }
        }
    };
    let zone = |rel: f32| -> u8 {
        let a = rel.abs();
        if a <= ROAD_HALF - 0.25 { 0 } else if a <= ROAD_HALF { 1 } else if a <= ROAD_HALF + 0.9 { 2 } else { 3 }
    };

    let row = fb.row_mut(y);
    for (px, out) in row.iter_mut().enumerate() {
        let rel = (px as f32 + 0.5 - cx) * mpp;
        let (l, r) = (rel - 0.5 * mpp, rel + 0.5 * mpp);
        let thin = (rel.abs() - LANE).abs() < mpp + 0.12
            || start_line
            || grid_mark
            || (grid.is_some() && (rel + mpp).rem_euclid(GRID) < GRID_LINE + 2.0 * mpp);
        let px_out = if zone(l) != zone(r) || (l < 0.0) != (r < 0.0) || thin {
            // Supersample edges and thin markings for smooth, stable lines.
            let (mut rr, mut gg, mut bb) = (0u32, 0u32, 0u32);
            for k in 0..4 {
                let c = color_at(l + (k as f32 + 0.5) * 0.25 * mpp);
                rr += c.0 as u32;
                gg += c.1 as u32;
                bb += c.2 as u32;
            }
            Rgb((rr / 4) as u8, (gg / 4) as u8, (bb / 4) as u8)
        } else {
            color_at(rel)
        };
        *out = px_out;
        // Sunlit snow glitters: sparse cells catch the light near the car.
        if pal.snow && mpp < 0.3 && rel.abs() > ROAD_HALF + 1.2 {
            let cell = ((rel * 6.0).floor() as i32 as u32).wrapping_mul(0x9E37_79B1) ^ ((d * 6.0).floor() as i64 as u32).wrapping_mul(0x85EB_CA77);
            if hash(cell).is_multiple_of(37) {
                *out = px_out.lerp(Rgb(255, 255, 255), (1.0 - fog * 2.5).max(0.0));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_sprite(fb: &mut FrameBuf, s: &SpriteInst, cx: f32, base: f32, wpx: f32, hpx: f32, clip_y: f32, light: f32, fogc: Rgb, fog: f32) {
    let bmp = s.bmp;
    if wpx < 0.5 || hpx < 0.5 {
        return;
    }
    let (x0, y0) = (cx - wpx / 2.0, base - hpx);
    let ix0 = x0.floor().max(0.0) as i32;
    let ix1 = (x0 + wpx).ceil().min(fb.w as f32) as i32;
    let iy0 = y0.floor().max(0.0) as i32;
    let iy1 = base.min(clip_y).ceil().min(fb.h as f32) as i32;
    // Supersample when the bitmap is being shrunk.
    let ns = ((bmp.w as f32 / wpx).max(bmp.h as f32 / hpx).ceil() as usize).clamp(1, 3);
    let (bw, bh) = (bmp.w as f32, bmp.h as f32);
    for py in iy0..iy1 {
        let row_clip = (clip_y - py as f32).clamp(0.0, 1.0);
        for px in ix0..ix1 {
            let (mut r, mut g, mut b, mut hits) = (0u32, 0u32, 0u32, 0u32);
            for sy in 0..ns {
                let fy = py as f32 + (sy as f32 + 0.5) / ns as f32;
                let v = ((fy - y0) / hpx * bh).floor() as isize;
                if v < 0 || v >= bmp.h as isize {
                    continue;
                }
                let skew = if v < 6 { s.skew as isize } else { 0 };
                for sx in 0..ns {
                    let fx = px as f32 + (sx as f32 + 0.5) / ns as f32;
                    let mut u = ((fx - x0) / wpx * bw).floor() as isize - skew;
                    if u < 0 || u >= bmp.w as isize {
                        continue;
                    }
                    if s.flip {
                        u = bmp.w as isize - 1 - u;
                    }
                    if let Some(c) = bmp.get(u as usize, v as usize).resolve(s.body) {
                        r += c.0 as u32;
                        g += c.1 as u32;
                        b += c.2 as u32;
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let c = Rgb((r / hits) as u8, (g / hits) as u8, (b / hits) as u8).scale(light).lerp(fogc, fog);
                let alpha = hits as f32 / (ns * ns) as f32 * row_clip;
                fb.blend(px, py, c, alpha);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Backdrop
// ---------------------------------------------------------------------------

fn ridge(u: f32, seed: f32) -> f32 {
    let mut h = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for k in 0..5 {
        h += amp * (1.0 - ((u * freq + seed * (k as f32 + 1.3)).sin()).abs());
        amp *= 0.5;
        freq *= 2.13;
    }
    h / 0.97
}

fn rolling(u: f32, seed: f32) -> f32 {
    0.5 + 0.28 * (u + seed).sin() + 0.14 * (u * 2.7 + seed * 3.0).sin() + 0.06 * (u * 7.3 + seed).sin()
}

fn cloud_noise(horizontal: f32, vertical: f32) -> f32 {
    let column = horizontal.floor() as i32;
    let row = vertical.floor() as i32;
    let smooth = |value: f32| value * value * (3.0 - 2.0 * value);
    let across = smooth(horizontal - horizontal.floor());
    let down = smooth(vertical - vertical.floor());
    let sample = |column: i32, row: i32| {
        hashf((column as u32).wrapping_mul(73_856_093) ^ (row as u32).wrapping_mul(19_349_663))
    };
    let upper = sample(column, row) * (1.0 - across) + sample(column + 1, row) * across;
    let lower = sample(column, row + 1) * (1.0 - across) + sample(column + 1, row + 1) * across;
    upper * (1.0 - down) + lower * down
}

fn draw_clouds(frame: &mut FrameBuf, view: &View, cloud: Rgb, scroll: f32, time: f32) {
    for pixel_y in 0..(view.hz * 0.83) as usize {
        let altitude = pixel_y as f32 / view.hz;
        let vertical = altitude * 5.0 + 8.0;
        let envelope = (1.0 - ((altitude - 0.35) / 0.48).abs()).max(0.0);
        for pixel_x in 0..frame.w {
            let horizontal = (pixel_x as f32 / frame.w as f32 + scroll * 0.25 + time * 0.0012) * 7.0 + 13.0;
            let broad = cloud_noise(horizontal, vertical);
            let fine = cloud_noise(horizontal * 3.1 + 7.0, vertical * 3.1);
            let detail = cloud_noise(horizontal * 8.3, vertical * 8.3 + 3.0);
            let density = (broad * 0.62 + fine * 0.26 + detail * 0.12 - 0.47) * 5.5 * envelope;
            let opacity = density.clamp(0.0, 0.94);
            if opacity > 0.0 {
                let shade = cloud.scale(0.87 + fine * 0.12).lerp(Rgb(198, 215, 232), (1.0 - broad) * 0.12);
                frame.blend(pixel_x as i32, pixel_y as i32, shade, opacity);
            }
        }
    }
}

fn draw_halo(frame: &mut FrameBuf, center: (f32, f32), radius: f32, aspect: f32, color: Rgb, strength: f32) {
    let horizontal_radius = radius / aspect;
    let left = (center.0 - horizontal_radius * 3.0).floor().max(0.0) as usize;
    let right = (center.0 + horizontal_radius * 3.0).ceil().min(frame.w as f32).max(0.0) as usize;
    let top = (center.1 - radius * 3.0).floor().max(0.0) as usize;
    let bottom = (center.1 + radius * 3.0).ceil().min(frame.h as f32).max(0.0) as usize;
    for pixel_y in top..bottom {
        for pixel_x in left..right {
            let horizontal = (pixel_x as f32 + 0.5 - center.0) / horizontal_radius;
            let vertical = (pixel_y as f32 + 0.5 - center.1) / radius;
            let opacity = (-(horizontal * horizontal + vertical * vertical) * 0.5).exp() * strength;
            frame.blend(pixel_x as i32, pixel_y as i32, color, opacity);
        }
    }
}

fn draw_distant_ground(frame: &mut FrameBuf, view: &View, palette: &Palette) {
    let horizon = view.hz.ceil() as usize;
    for row in horizon.min(frame.h)..frame.h {
        let depth = ((row as f32 - view.hz) / (view.h as f32 - view.hz)).sqrt();
        let color = palette.grass[0].lerp(palette.fog, 0.82 * (1.0 - depth));
        frame.row_mut(row).fill(color);
    }
}

fn draw_backdrop(fb: &mut FrameBuf, v: &View, pal: &Palette, bg: f32, time: f32) {
    let (w, h) = (v.w, v.h);
    let wf = w as f32;
    let hz = v.hz;
    let hzi = hz.ceil() as usize;

    // Sky: three-stop gradient with ordered dithering between bands.
    for y in 0..hzi.min(h) {
        let t = y as f32 / hz;
        for x in 0..w {
            let tt = (t + bayer(x, y) * 0.03).clamp(0.0, 1.0);
            let c = if tt < 0.55 { pal.sky[0].lerp(pal.sky[1], tt / 0.55) } else { pal.sky[1].lerp(pal.sky[2], (tt - 0.55) / 0.45) };
            fb.set(x as i32, y as i32, c);
        }
    }

    if pal.stars {
        let n = (w * hzi / 110).min(220) as u32;
        for i in 0..n {
            let x = ((hashf(i * 3 + 1) - bg * 0.1) * wf).rem_euclid(wf);
            let y = hashf(i * 3 + 2) * hz * 0.8;
            let tw = ((time * (1.5 + hashf(i)) + i as f32).sin() * 0.5 + 0.5) * 0.7 + 0.3;
            let c = Rgb(255, 255, 230).scale(tw * (0.5 + hashf(i * 7) * 0.5));
            fb.set(x as i32, y as i32, c);
        }
    }

    match pal.backdrop {
        Backdrop::Synth => return draw_synth_backdrop(fb, v, pal, bg, time),
        Backdrop::Winter => return draw_winter_backdrop(fb, v, pal, bg, time),
        Backdrop::Midnight => return night_city::draw(fb, v, pal, bg),
        _ => {}
    }

    // Sun / moon with glow rings.
    if let Some(sun) = pal.sun {
        let sx = (wf * 0.72 - bg * wf * 0.15).rem_euclid(wf * 1.6) - wf * 0.3;
        let sy = hz * if pal.stars { 0.28 } else { 0.5 };
        let r = (h as f32 * 0.06).max(2.0);
        draw_halo(fb, (sx, sy), r * 1.8, v.aspect, sun, 0.24);
        fb.ellipse(sx, sy, r / v.aspect, r, sun, 1.0);
        if pal.stars {
            // Moon craters.
            for (ox, oy, rr) in [(-0.3, -0.2, 0.22), (0.35, 0.25, 0.16), (0.1, -0.45, 0.1)] {
                fb.ellipse(sx + ox * r / v.aspect, sy + oy * r, rr * r / v.aspect, rr * r, sun.scale(0.82), 1.0);
            }
        }
    }

    if let Some(cloud) = pal.clouds {
        draw_clouds(fb, v, cloud, bg, time);
    }

    // Far layer: jagged mountains or a city skyline.
    match pal.backdrop {
        Backdrop::Mountains { snow } => {
            let lit = pal.far_hills.lerp(Rgb(255, 255, 255), 0.18);
            let shade = pal.far_hills.scale(0.75);
            for x in 0..w {
                let u = (x as f32 / wf + bg * 0.35) * TAU;
                let hgt = ridge(u, 1.7);
                let top = hz - hz * 0.55 * hgt;
                let slope = ridge(u + 0.01, 1.7) - hgt;
                let base_c = if slope > 0.0 { lit } else { shade };
                for y in top.max(0.0) as usize..hzi.min(h) {
                    let depth = (y as f32 - top) / (hz - top).max(1.0);
                    let mut c = base_c.lerp(pal.sky[2], depth * 0.55);
                    if snow && hgt > 0.62 && (y as f32) < top + hz * 0.05 * (hgt - 0.5) * 4.0 {
                        c = if slope > 0.0 { Rgb(250, 240, 245) } else { Rgb(200, 180, 210) };
                    }
                    fb.set(x as i32, y as i32, c);
                }
            }
        }
        Backdrop::Synth | Backdrop::Winter | Backdrop::Midnight => unreachable!(),
        Backdrop::City => {
            let cols = 34.0;
            for x in 0..w {
                let u = x as f32 / wf + bg * 0.45;
                let bi = (u * cols).floor() as i64;
                let local = ((u * cols).fract() * wf / cols) as i32;
                let bh = 0.12 + hashf((bi as u32).wrapping_mul(7).wrapping_add(3)) * 0.32;
                let top = hz - hz * bh;
                let tint = pal.far_hills.scale(0.8 + hashf(bi as u32) * 0.4);
                for y in top.max(0.0) as usize..hzi.min(h) {
                    let ly = y as i32 - top as i32;
                    let window = local % 3 == 1 && ly % 3 == 1 && ly > 1;
                    let lit_w = hash((bi as u32).wrapping_mul(131) ^ ((local / 3) as u32).wrapping_mul(17) ^ (ly as u32 / 3 * 7919)).is_multiple_of(3);
                    let c = if window && lit_w { Rgb(255, 210, 110) } else { tint };
                    fb.set(x as i32, y as i32, c);
                }
            }
        }
    }

    // Near rolling hills with a tree line.
    let near_lit = pal.near_hills.lerp(Rgb(255, 255, 200), 0.08);
    for x in 0..w {
        let u = x as f32 / wf;
        let base = rolling((u * 1.5 + bg) * TAU, 4.0);
        let trees = ((u * 38.0 + bg * 25.0) * TAU).sin().abs().sqrt() * 0.03 * (0.5 + base);
        let top = hz - hz * (0.07 + 0.14 * base + trees);
        for y in top.max(0.0) as usize..hzi.min(h) {
            let t = (y as f32 - top) / (hz - top).max(1.0);
            let c = near_lit.lerp(pal.near_hills.scale(0.6), t + bayer(x, y) * 0.15);
            fb.set(x as i32, y as i32, c);
        }
    }

    // Distant ground below the horizon (seen past crests).
    draw_distant_ground(fb, v, pal);
}

/// Synthwave sunset: striped retro sun sinking behind neon wireframe
/// mountains (with a lone volcano) and a Tokyo skyline with pagodas.
fn draw_synth_backdrop(fb: &mut FrameBuf, v: &View, pal: &Palette, bg: f32, time: f32) {
    let (w, h) = (v.w, v.h);
    let (wf, hz) = (w as f32, v.hz);
    let hzi = (hz.ceil() as usize).min(h);
    let neon = pal.grid.unwrap_or(Rgb(255, 46, 196));

    // Stars fading out towards the warm horizon.
    let n = (w * hzi / 140).min(140) as u32;
    for i in 0..n {
        let x = ((hashf(i * 3 + 1) - bg * 0.1) * wf).rem_euclid(wf);
        let yt = hashf(i * 3 + 2).powf(1.6) * 0.55;
        let tw = ((time * (1.5 + hashf(i)) + i as f32).sin() * 0.5 + 0.5) * 0.6 + 0.4;
        fb.blend(x as i32, (yt * hz) as i32, Rgb(255, 230, 255), tw * (1.0 - yt / 0.55));
    }

    // Retro sun: yellow-to-magenta gradient, sliced by widening gaps.
    let sun = pal.sun.unwrap_or(Rgb(255, 214, 90));
    let r = h as f32 * 0.17;
    let rx = r / v.aspect;
    let sx = (wf * 0.5 - bg * wf * 0.15).rem_euclid(wf * 1.6) - wf * 0.3;
    let sy = hz * 0.6;
    for (k, a) in [(1.9, 0.08), (1.45, 0.14), (1.15, 0.22)] {
        fb.ellipse(sx, sy, rx * k, r * k, Rgb(255, 80, 150), a);
    }
    for y in (sy - r).max(0.0) as i32..(sy + r).min(hz) as i32 {
        let dy = (y as f32 + 0.5 - sy) / r;
        let u = (dy + 1.0) / 2.0;
        if dy > 0.0 {
            let band = (dy * 7.0).fract();
            if band < 0.15 + dy * 0.45 {
                continue;
            }
        }
        let half = (1.0 - dy * dy).max(0.0).sqrt() * rx;
        let c = sun.lerp(Rgb(255, 120, 60), (u * 1.6).min(1.0)).lerp(Rgb(255, 40, 150), ((u - 0.5) * 2.0).clamp(0.0, 1.0));
        for x in (sx - half).floor().max(0.0) as i32..(sx + half).ceil().min(wf) as i32 {
            let edge = (half - (x as f32 + 0.5 - sx).abs()).clamp(0.0, 1.0);
            fb.blend(x, y, c, edge);
        }
    }

    // Wireframe mountains with a snow-capped volcano.
    let fill = pal.far_hills;
    let height_at = |x: f32| -> f32 {
        let p = x / wf + bg * 0.35;
        let base = ridge(p * TAU, 3.1) * 0.42;
        let f = 1.0 - ((p.rem_euclid(1.0) - 0.68).abs() / 0.16);
        let volcano = if f > 0.0 { 0.78 * f.powf(1.25).min(0.93) } else { 0.0 };
        base.max(volcano)
    };
    let fuji = |x: f32| -> bool {
        let p = (x / wf + bg * 0.35).rem_euclid(1.0);
        (p - 0.68).abs() < 0.16 && ridge((x / wf + bg * 0.35) * TAU, 3.1) * 0.42 < 1.0 - ((p - 0.68).abs() / 0.16)
    };
    for x in 0..w {
        let xf = x as f32 + 0.5;
        let hgt = height_at(xf);
        let top = hz - hz * 0.62 * hgt;
        let slope = height_at(xf + 1.0) - hgt;
        let snowy = fuji(xf);
        for y in top.max(0.0) as usize..hzi {
            let yf = y as f32 + 0.5;
            let below = yf - top;
            let depth = below / (hz - top).max(1.0);
            let mut c = fill.lerp(Rgb(90, 20, 90), depth * 0.6);
            if slope > 0.0 {
                c = c.lerp(Rgb(150, 50, 120), 0.18 * (1.0 - depth));
            }
            // Horizontal contour lines.
            if ((hz - yf) / hz * 22.0).fract() < 0.1 {
                c = c.lerp(neon, 0.35 * (1.0 - depth * 0.5));
            }
            if snowy && below < hz * 0.62 * 0.2 * (1.0 + 0.3 * (xf * 0.9).sin()) && hgt > 0.5 {
                c = Rgb(255, 196, 230).lerp(Rgb(200, 120, 190), (slope < 0.0) as u8 as f32 * 0.6);
            }
            fb.set(x as i32, y as i32, c);
        }
        // Glowing ridge line.
        let ty = top.floor() as i32;
        fb.blend(x as i32, ty, neon.lerp(Rgb(255, 255, 255), 0.25), 1.0 - top.fract() * 0.5);
        fb.blend(x as i32, ty - 1, neon, 0.35);
    }

    // Tokyo skyline: towers with neon windows, pagodas, a radio mast.
    let cols = 28.0;
    let window_cols = [Rgb(255, 80, 200), Rgb(60, 230, 255), Rgb(255, 210, 110)];
    for x in 0..w {
        let u = x as f32 / wf + bg * 0.55;
        let bi = (u * cols).floor() as i64;
        let fx = (u * cols).fract();
        let id = bi as u32;
        let pagoda = hash(id.wrapping_mul(31).wrapping_add(5)).is_multiple_of(7);
        let gap = hash(id.wrapping_mul(13).wrapping_add(1)).is_multiple_of(4);
        let bh = if pagoda { 0.2 } else { 0.05 + hashf(id.wrapping_mul(7).wrapping_add(3)) * 0.17 };
        let mut top = hz - hz * bh;
        if pagoda {
            // Five tiers, each roof narrower, with a spire on top.
            let dc = (fx - 0.5).abs() * 2.0;
            let tier_h = bh / 5.5;
            let mut t = None;
            for k in 0..5 {
                let reach = 1.0 - k as f32 * 0.16;
                if dc <= reach {
                    t = Some(k);
                }
            }
            top = match t {
                Some(k) if dc < 0.06 => hz - hz * (tier_h * (k as f32 + 1.0) + bh * 0.25),
                Some(k) => hz - hz * tier_h * (k as f32 + 1.0) + if dc > 1.0 - k as f32 * 0.16 - 0.08 { -1.0 } else { 0.0 },
                None => hzi as f32,
            };
        } else if gap {
            top = hz - hz * 0.03;
        } else if hash(id.wrapping_mul(3).wrapping_add(9)).is_multiple_of(9) && (fx - 0.5).abs() < 0.04 {
            top -= hz * 0.08;
        }
        let tint = pal.near_hills.scale(0.85 + hashf(id) * 0.4);
        let px_w = (wf / cols) as i32;
        let lx = (fx * wf / cols) as i32;
        for y in top.max(0.0) as usize..hzi {
            let ly = y as i32 - top as i32;
            let mut c = tint;
            if !pagoda && !gap && lx % 3 == 1 && ly % 3 == 1 && ly > 1 && lx < px_w - 1 {
                let hsh = hash((id).wrapping_mul(131) ^ ((lx / 3) as u32 * 17) ^ (ly as u32 / 3 * 7919));
                if hsh.is_multiple_of(3) {
                    c = window_cols[(hsh / 3 % 3) as usize];
                }
            }
            if pagoda && ly == 0 {
                c = Rgb(255, 120, 150);
            }
            fb.set(x as i32, y as i32, c);
        }
    }

    // Horizon glow and the grid floor beyond any crests.
    for y in hzi.saturating_sub(3)..hzi {
        let a = 1.0 - (hzi - y) as f32 / 4.0;
        for x in 0..w {
            fb.blend(x as i32, y as i32, Rgb(255, 120, 200), a * 0.35);
        }
    }
    let ground = pal.grass[0].lerp(pal.fog, 0.7);
    for y in hzi..h {
        fb.row_mut(y).fill(ground);
    }
}

/// Sunny winter afternoon: a low golden sun, cirrus streaks, a snow-capped
/// volcano above hazy ranges, frosted forest, a pastel town with a pagoda
/// and a lattice tower, a bullet train gliding past, and diamond dust.
fn draw_winter_backdrop(fb: &mut FrameBuf, v: &View, pal: &Palette, bg: f32, time: f32) {
    let (w, h) = (v.w, v.h);
    let (wf, hz) = (w as f32, v.hz);
    let hzi = (hz.ceil() as usize).min(h);
    let haze = pal.sky[2];
    let aspect = v.aspect;

    // Low sun with a wide, soft halo and a faint horizontal flare.
    let sun = pal.sun.unwrap_or(Rgb(255, 238, 196));
    let sx = (wf * 0.8 - bg * wf * 0.15).rem_euclid(wf * 1.6) - wf * 0.3;
    let sy = hz * 0.36;
    let r = (h as f32 * 0.045).max(1.5);
    draw_halo(fb, (sx, sy), r * 3.2, aspect, Rgb(255, 222, 170), 0.38);
    fb.ellipse(sx, sy, r * 9.0 / aspect, r * 0.22, Rgb(255, 236, 200), 0.2);
    fb.ellipse(sx, sy, r / aspect, r, sun.lerp(Rgb(255, 255, 255), 0.6), 1.0);

    // Cirrus: long thin streaks, warmed where they pass near the sun.
    for k in 0..8u32 {
        let span = 1.7;
        let cu = (hashf(k * 7 + 1) * span - bg * 0.22 + time * 0.003).rem_euclid(span) - 0.35;
        let cy = hz * (0.08 + hashf(k * 7 + 2) * 0.38);
        let len = wf * (0.1 + hashf(k * 7 + 3) * 0.14);
        let warm = (1.0 - ((cu * wf - sx).abs() / (wf * 0.35))).clamp(0.0, 1.0);
        let c = Rgb(255, 255, 255).lerp(Rgb(255, 210, 186), warm);
        let th = (h as f32 * 0.006).max(0.6);
        for p in 0..5 {
            let t = p as f32 / 4.0 - 0.5;
            let px = cu * wf + t * len * 0.8;
            let py = cy + t * th * 3.0 + (hashf(k * 13 + p) - 0.5) * th * 2.0;
            fb.ellipse(px, py, len * (0.3 + hashf(k * 17 + p) * 0.2), th * (0.8 + hashf(k * 19 + p)), c, 0.28);
        }
    }

    // Hazy far ranges with snowy crests.
    for x in 0..w {
        let u = (x as f32 / wf + bg * 0.3) * TAU;
        let hgt = ridge(u * 1.3, 2.3);
        let top = hz - hz * 0.3 * hgt;
        let slope = ridge(u * 1.3 + 0.01, 2.3) - hgt;
        for y in top.max(0.0) as usize..hzi {
            let depth = (y as f32 - top) / (hz - top).max(1.0);
            let snowy = hgt > 0.5 && depth < 0.35;
            let base = if snowy {
                if slope < 0.0 { Rgb(250, 244, 240) } else { Rgb(188, 198, 232) }
            } else {
                pal.far_hills.scale(if slope < 0.0 { 1.08 } else { 0.92 })
            };
            fb.set(x as i32, y as i32, base.lerp(haze, 0.35 + depth * 0.35));
        }
    }

    // The volcano: concave flanks, a flat crater rim, snow streaming down
    // its gullies; the sun-facing side warm, the other in blue shadow.
    let p0 = 0.3;
    let vol_h = |x: f32| -> (f32, f32) {
        let p = (x / wf + bg * 0.3 - p0 + 0.5).rem_euclid(1.0) - 0.5;
        let q = 1.0 - p.abs() / 0.24;
        (if q > 0.0 { (0.86 * q.powf(1.7)).min(0.8) } else { 0.0 }, p)
    };
    for x in 0..w {
        let (hgt, p) = vol_h(x as f32 + 0.5);
        if hgt <= 0.0 {
            continue;
        }
        let top = hz - hz * 0.72 * hgt;
        let lit = p > 0.0;
        for y in top.max(0.0) as usize..hzi {
            let yh = (hz - y as f32 - 0.5) / (hz * 0.72);
            // Irregular tongues of snow reaching down the ravines.
            let gully = ((p * 230.0).sin() * 0.6 + (p * 97.0 + 1.0).sin() * 0.4).max(0.0).powi(2);
            let snowline = 0.5 + 0.03 * (p * 41.0).sin() - 0.17 * gully;
            let mut c = if yh > snowline {
                let streak = ((p * 150.0 + yh * 2.0).sin() > 0.8) as u8 as f32 * 0.12;
                if lit { Rgb(255, 246, 236) } else { Rgb(184, 196, 234) }.lerp(Rgb(150, 164, 210), streak)
            } else if lit {
                Rgb(118, 128, 176)
            } else {
                Rgb(86, 98, 148)
            };
            let fade = 0.12 + 0.55 * (1.0 - yh / 0.8).max(0.0).powi(2);
            c = c.lerp(haze, fade);
            fb.set(x as i32, y as i32, c);
        }
    }

    // Frosted cedar forest on the near hills.
    let forest = pal.near_hills;
    for x in 0..w {
        let u = x as f32 / wf + bg * 0.45;
        let base = rolling((u * 1.4) * TAU, 2.0);
        let teeth = 1.0 - ((u * 110.0).fract() - 0.5).abs() * 2.0;
        let top = hz - hz * (0.05 + 0.1 * base + 0.03 * teeth);
        for y in top.max(0.0) as usize..hzi {
            let t = (y as f32 - top) / (hz - top).max(1.0);
            let snow = 0.5 - t * 0.6;
            let c = if bayer(x, y) < snow {
                Rgb(226, 234, 248).lerp(haze, 0.15)
            } else {
                forest.lerp(Rgb(70, 104, 110), 0.3 * (1.0 - t)).lerp(haze, 0.25 * (1.0 - t))
            };
            fb.set(x as i32, y as i32, c);
        }
    }

    // A small pastel town along the horizon.
    let walls = [Rgb(240, 230, 214), Rgb(206, 212, 226), Rgb(156, 176, 206), Rgb(214, 140, 112), Rgb(236, 214, 170)];
    let cols = 46.0;
    let tower_at = 0.62;
    for x in 0..w {
        let u = x as f32 / wf + bg * 0.55;
        let bi = (u * cols).floor() as i64;
        let fx = (u * cols).fract();
        let id = bi as u32;
        let pu = u.rem_euclid(1.0);
        // Red-and-white lattice tower.
        let dt = (pu - tower_at).abs() * cols;
        if dt < 0.5 {
            let th = hz * 0.16;
            let top = hz - th;
            for y in top.max(0.0) as usize..hzi {
                let k = (hz - y as f32) / th;
                if dt < 0.5 * (1.0 - k * 0.85) {
                    let band = ((k * 9.0) as i32) % 2 == 0;
                    let c = if band { Rgb(232, 70, 56) } else { Rgb(246, 244, 240) };
                    fb.set(x as i32, y as i32, c.lerp(haze, 0.2));
                }
            }
            continue;
        }
        if hash(id.wrapping_mul(13).wrapping_add(7)).is_multiple_of(3) {
            continue;
        }
        let pagoda = hash(id.wrapping_mul(29).wrapping_add(3)).is_multiple_of(13);
        let bh = if pagoda { 0.12 } else { 0.03 + hashf(id.wrapping_mul(7).wrapping_add(1)) * 0.06 };
        let top = hz - hz * bh;
        let wall = walls[(hash(id) % walls.len() as u32) as usize];
        let px_w = (wf / cols).max(1.0);
        let lx = (fx * px_w) as i32;
        for y in top.max(0.0) as usize..hzi {
            let ly = y as i32 - top as i32;
            let c = if pagoda {
                let tier = (ly as f32 / (hz * bh / 4.0)) as i32;
                let reach = 0.25 + tier as f32 * 0.07;
                if (fx - 0.5).abs() > reach {
                    continue;
                }
                let roof = ((ly as f32) % (hz * bh / 4.0)) < 1.0;
                if roof { Rgb(244, 248, 255) } else { Rgb(92, 56, 50) }
            } else if ly == 0 {
                Rgb(250, 252, 255)
            } else if lx % 3 == 1 && ly % 3 == 2 && lx < px_w as i32 - 1 {
                if hash(id.wrapping_mul(97) ^ (lx as u32).wrapping_mul(31) ^ (ly as u32).wrapping_mul(7)).is_multiple_of(4) {
                    Rgb(255, 214, 140)
                } else {
                    wall.scale(0.62)
                }
            } else {
                wall
            };
            fb.set(x as i32, y as i32, c.lerp(haze, 0.18));
        }
    }

    // Elevated railway with a bullet train gliding across.
    let yv = hz - hz * 0.04;
    let rail = Rgb(170, 176, 196).lerp(haze, 0.2);
    for x in 0..w {
        fb.set(x as i32, yv as i32, rail);
        if ((x as f32 / wf + bg * 0.6) * 60.0).fract() < 0.06 {
            for y in yv as usize..hzi {
                fb.set(x as i32, y as i32, rail.scale(0.85));
            }
        }
    }
    let len = wf * 0.32;
    let tx = (time * wf * 0.07 - bg * wf * 0.6).rem_euclid(wf * 2.2) - wf * 0.5;
    let th = (hz * 0.04).max(2.0);
    let nose = len * 0.09;
    for x in tx.max(0.0) as i32..(tx + len).min(wf) as i32 {
        let along = x as f32 + 0.5 - tx;
        let cut = ((along - (len - nose)) / nose).max(0.0);
        let top = yv - th + cut * cut * th * 0.9;
        for y in top.max(0.0) as i32..yv as i32 {
            let k = (y as f32 + 0.5 - (yv - th)) / th;
            let car_gap = (along / (len / 8.0)).fract() < 0.015;
            let c = if car_gap {
                Rgb(150, 156, 176)
            } else if k > 0.62 {
                Rgb(32, 82, 180)
            } else if (0.25..0.45).contains(&k) && cut == 0.0 {
                Rgb(70, 84, 112)
            } else {
                Rgb(246, 248, 252)
            };
            fb.set(x, y, c);
        }
    }

    // Glow of haze along the horizon, then distant snowfields past crests.
    for y in hzi.saturating_sub(3)..hzi {
        let a = 1.0 - (hzi - y) as f32 / 4.0;
        for x in 0..w {
            fb.blend(x as i32, y as i32, haze, a * 0.3);
        }
    }
    let ground = pal.grass[0].lerp(pal.fog, 0.6);
    for y in hzi..h {
        fb.row_mut(y).fill(ground);
    }

    // Diamond dust: ice crystals twinkling as they drift in the sunlight.
    let n = (w * hzi / 260) as u32;
    for i in 0..n {
        let x = ((hashf(i * 5 + 1) - bg * 0.3 + time * 0.006) * wf).rem_euclid(wf);
        let y = ((hashf(i * 5 + 2) + time * 0.012 * (0.5 + hashf(i))) * hz).rem_euclid(hz);
        let tw = ((time * (2.0 + hashf(i * 5 + 3) * 3.0) + i as f32 * 1.7).sin() * 0.5 + 0.5).powi(3);
        if tw > 0.15 {
            fb.blend(x as i32, y as i32, Rgb(255, 255, 255), tw);
            if tw > 0.8 {
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    fb.blend(x as i32 + dx, y as i32 + dy, Rgb(255, 250, 235), (tw - 0.8) * 2.5);
                }
            }
        }
    }
}

/// The five red start lights.
pub fn draw_start_lights(fb: &mut FrameBuf, on: u32, aspect: f32) {
    let r = (fb.h as f32 / 24.0).max(2.0);
    let rx = r / aspect;
    let gap = rx * 2.8;
    let total = gap * 5.0;
    let x0 = fb.w as f32 / 2.0 - total / 2.0;
    let cy = fb.h as f32 * 0.16;
    fb.fill_rect((x0 - rx * 0.4) as i32, (cy - r * 1.7) as i32, (total + rx * 0.8) as i32, (r * 3.4) as i32, Rgb(15, 15, 18));
    for i in 0..5 {
        let lit = (i as u32) < on;
        let cx = x0 + gap * (i as f32 + 0.5);
        if lit {
            fb.ellipse(cx, cy, rx * 1.25, r * 1.25, Rgb(255, 40, 30), 0.35);
            fb.ellipse(cx, cy, rx, r, Rgb(255, 40, 30), 1.0);
            fb.ellipse(cx - rx * 0.3, cy - r * 0.3, rx * 0.3, r * 0.3, Rgb(255, 200, 180), 1.0);
        } else {
            fb.ellipse(cx, cy, rx, r, Rgb(60, 8, 8), 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::{draw_text_centered, Mode};
    use crate::race::Race;
    use crate::track::all_tracks;
    use std::rc::Rc;

    #[test]
    fn steering_yaw_points_the_nose_in_the_input_direction() {
        for steering in [-2, -1, 1, 2] {
            let yaw = car_yaw(0.0, 0.0, steering);
            let nose_offset = 3.94 * yaw.sin();
            assert_eq!(nose_offset.signum(), (steering as f32).signum());
            let road = 0.12;
            assert!((car_yaw(0.0, road, steering) - road.atan() - yaw).abs() < 0.00001);
        }
    }

    #[test]
    #[ignore = "writes steering screenshots and wheel-animation frames"]
    fn steering_and_wheel_showcase_frames() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/screenshots/handling");
        std::fs::create_dir_all(&directory).unwrap();
        let track = Rc::new(all_tracks().remove(0));
        for (name, steer) in [("left", -1.0), ("straight", 0.0), ("right", 1.0)] {
            let mut race = Race::new(track.clone(), 3, false, true);
            race.phase = crate::race::Phase::Racing;
            race.rivals.clear();
            race.d = 160.0;
            race.x = 0.0;
            race.update(1.0 / 60.0, crate::race::Controls { steer, ..Default::default() });
            let mut sprites = Vec::new();
            race.sprites(&mut sprites);
            let mut frame = FrameBuf::new();
            frame.resize(800, 448);
            render_world(&mut frame, &track, &Camera { d: race.cam_d(), x: race.x, bg: race.bg }, &mut sprites, race.time, 1.0);
            write_ppm(&directory.join(format!("{name}.ppm")), frame.w, frame.h, |pixel_x, pixel_y| frame.get(pixel_x, pixel_y));
        }
        for index in 0..36 {
            let mut frame = FrameBuf::new();
            frame.resize(640, 400);
            frame.fill_rect(0, 0, 640, 400, Rgb(70, 75, 80));
            motorsport::draw_car(&mut frame, &motorsport::ModelView {
                focal_x: 370.0, focal_y: 370.0, horizon: 65.0, camera_height: 2.8,
                lateral: 0.0, distance: 4.6, grade: 0.0, yaw: 0.45, clip: 400.0,
                ground: None, lift: 0.0, travel: index as f32 * 0.055,
                body: track.palette.car, light: 1.0, fog_color: track.palette.fog, fog: 0.0,
            });
            write_ppm(&directory.join(format!("wheel-{index:02}.ppm")), frame.w, frame.h, |pixel_x, pixel_y| frame.get(pixel_x, pixel_y));
        }
    }

    #[test]
    fn racing_showcase_frames() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/screenshots");
        std::fs::create_dir_all(&directory).unwrap();
        for (index, track) in all_tracks().into_iter().enumerate() {
            let mut race = Race::new(Rc::new(track), 3, true, true);
            for _ in 0..60 * (9 + index * 7) {
                race.update(1.0 / 60.0, Default::default());
            }
            let mut sprites = Vec::new();
            let camera = Camera { d: race.cam_d(), x: race.x, bg: race.bg };
            let mut frame = FrameBuf::new();
            frame.resize(800, 448);
            race.sprites(&mut sprites);
            render_world(&mut frame, &race.track, &camera, &mut sprites, race.time, 1.0);
            write_ppm(&directory.join(format!("race-{index}.ppm")), frame.w, frame.h, |pixel_x, pixel_y| frame.get(pixel_x, pixel_y));
        }
    }

    #[test]
    fn midnight_showcase_frames() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/screenshots/midnight");
        std::fs::create_dir_all(&directory).unwrap();
        let track = Rc::new(all_tracks().into_iter().find(|track| track.name == "Tokyo Midnight").unwrap());
        let mut race = Race::new(track.clone(), 3, false, true);
        race.x = 0.0;
        race.rivals.clear();
        let mut frame = FrameBuf::new();
        frame.resize(800, 448);
        for (name, distance, background) in [("boulevard", 80.0, 0.0), ("chicane", 530.0, 0.13), ("shrine", 1700.0, 0.3), ("skyline", 2680.0, -0.2)] {
            race.d = distance;
            let mut sprites = Vec::new();
            race.sprites(&mut sprites);
            render_world(&mut frame, &track, &Camera { d: race.cam_d(), x: 0.0, bg: background }, &mut sprites, 12.0, 1.0);
            assert!((0..frame.w).any(|horizontal| frame.get(horizontal, frame.h / 2) != Rgb::default()));
            write_ppm(&directory.join(format!("{name}.ppm")), frame.w, frame.h, |horizontal, vertical| frame.get(horizontal, vertical));
        }
    }

    fn write_ppm(path: &std::path::Path, w: usize, h: usize, px: impl Fn(usize, usize) -> Rgb) {
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                let c = px(x, y);
                out.extend_from_slice(&[c.0, c.1, c.2]);
            }
        }
        std::fs::write(path, out).unwrap();
    }

    /// Renders frames to `target/snapshots/*.ppm` for eyeballing: native
    /// pixels, plus a simulation of how the text modes look in a terminal.
    #[test]
    fn snapshot_frames() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        for (i, track) in all_tracks().into_iter().enumerate() {
            let mut race = Race::new(Rc::new(track), 3, true, true);
            for _ in 0..(60 * (9 + i * 7)) {
                race.update(1.0 / 60.0, Default::default());
            }
            let mut sprites = Vec::new();
            let cam = Camera { d: race.cam_d(), x: race.x, bg: race.bg };

            let mut fb = FrameBuf::new();
            fb.resize(384, 216);
            race.sprites(&mut sprites);
            render_world(&mut fb, &race.track, &cam, &mut sprites, 1.0, 1.0);
            if i == 0 {
                draw_start_lights(&mut fb, 3, 1.0);
            }
            if i == 1 {
                draw_text_centered(&mut fb, "Slopracer F1", 12, (4, 4), Rgb(255, 235, 80), Rgb(255, 90, 20), Some(Rgb(30, 10, 10)));
            }
            write_ppm(&dir.join(format!("pixels{i}.ppm")), fb.w, fb.h, |x, y| fb.get(x, y));

            if i == 4 {
                for (k, d) in [1450.0f32, 60.0, 700.0, 2500.0].into_iter().enumerate() {
                    race.d = d;
                    race.x = 0.0;
                    let cam = Camera { d: race.cam_d(), x: 0.0, bg: race.bg };
                    race.sprites(&mut sprites);
                    render_world(&mut fb, &race.track, &cam, &mut sprites, 3.0 + k as f32 * 4.0, 1.0);
                    write_ppm(&dir.join(format!("winter{k}.ppm")), fb.w, fb.h, |x, y| fb.get(x, y));
                }
                // A big frame with the bullet train in view.
                fb.resize(960, 540);
                race.d = 60.0;
                let cam = Camera { d: race.cam_d(), x: 0.0, bg: race.bg };
                race.sprites(&mut sprites);
                render_world(&mut fb, &race.track, &cam, &mut sprites, 10.0 + race.bg * 0.6 / 0.07, 1.0);
                write_ppm(&dir.join("winter_big.ppm"), fb.w, fb.h, |x, y| fb.get(x, y));
                fb.resize(384, 216);
            }
            if i == 3 {
                for (k, d) in [2150.0f32, 400.0, 1150.0].into_iter().enumerate() {
                    race.d = d;
                    race.x = 0.0;
                    let cam = Camera { d: race.cam_d(), x: 0.0, bg: race.bg };
                    race.sprites(&mut sprites);
                    render_world(&mut fb, &race.track, &cam, &mut sprites, 1.0, 1.0);
                    write_ppm(&dir.join(format!("synth{k}.ppm")), fb.w, fb.h, |x, y| fb.get(x, y));
                }
            }
            if i == 1 {
                for (k, d) in [1235.0f32, 1395.0, 1580.0].into_iter().enumerate() {
                    race.d = d;
                    race.x = 0.0;
                    let cam = Camera { d: race.cam_d(), x: 0.0, bg: race.bg };
                    race.sprites(&mut sprites);
                    render_world(&mut fb, &race.track, &cam, &mut sprites, 1.0, 1.0);
                    write_ppm(&dir.join(format!("hill{k}.ppm")), fb.w, fb.h, |x, y| fb.get(x, y));
                }
                fb.resize(384, 216);
            }

            // Terminal simulation at 140x42 cells of 8x16 px.
            let (cols, rows, cw, ch) = (140usize, 42usize, 8usize, 16usize);
            for mode in [Mode::HalfBlock, Mode::Quadrant] {
                let (w, h, aspect) = match mode {
                    Mode::HalfBlock => (cols, rows * 2, 1.0),
                    _ => (cols * 2, rows * 2, 0.5),
                };
                fb.resize(w, h);
                race.sprites(&mut sprites);
                render_world(&mut fb, &race.track, &cam, &mut sprites, 1.0, aspect);
                let name = format!("{}{i}.ppm", if mode == Mode::HalfBlock { "half" } else { "quad" });
                write_ppm(&dir.join(name), cols * cw, rows * ch, |x, y| {
                    let (c, r) = (x / cw, y / ch);
                    let (sx, sy) = ((x % cw) * 2 / cw, (y % ch) * 2 / ch);
                    if mode == Mode::HalfBlock {
                        return fb.get(c, r * 2 + sy);
                    }
                    let q = [fb.get(c * 2, r * 2), fb.get(c * 2 + 1, r * 2), fb.get(c * 2, r * 2 + 1), fb.get(c * 2 + 1, r * 2 + 1)];
                    let (glyph, fg, bg) = crate::gfx::quadrant_for_test(q);
                    let bit = sy * 2 + sx;
                    let idx = crate::gfx::QUADS_FOR_TEST.iter().position(|&g| g == glyph).unwrap();
                    if (idx >> bit) & 1 == 1 { fg } else { bg }
                });
            }
        }
    }
}
