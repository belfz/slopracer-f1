use crate::gfx::{draw_text, FrameBuf, Rgb};
use crate::race::Race;
use crate::track::Track;
use std::f32::consts::TAU;

const SAMPLES: usize = 256;
const PANEL: Rgb = Rgb(10, 14, 29);
const PLAYER: Rgb = Rgb(255, 205, 48);
const ROAD: Rgb = Rgb(211, 223, 238);

#[derive(Clone, Copy, Default, Debug)]
struct Point {
    horizontal: f32,
    vertical: f32,
}

#[derive(Default)]
pub(crate) struct TrackMap {
    points: Vec<Point>,
    size: Point,
    length: f32,
}

impl TrackMap {
    pub(crate) fn from_track(track: &Track) -> Self {
        Self::from_curvature(track, false)
    }

    pub(crate) fn from_closed_track(track: &Track) -> Self {
        Self::from_curvature(track, true)
    }

    fn from_curvature(track: &Track, closed: bool) -> Self {
        let substeps = if closed { 16 } else { 1 };
        let step = track.length / (SAMPLES * substeps) as f32;
        let turns: Vec<_> = (0..SAMPLES * substeps).map(|index| track.curve_at((index as f32 + 0.5) * step) * step).collect();
        let right: f32 = turns.iter().map(|turn| turn.max(0.0)).sum();
        let left: f32 = turns.iter().map(|turn| (-turn).max(0.0)).sum();
        let reverse_turns = if right > 0.0 && left > 0.0 { TAU * 0.35 } else { 0.0 };
        let right_target = if right >= left { TAU + reverse_turns } else { reverse_turns };
        let left_target = if right >= left { reverse_turns } else { TAU + reverse_turns };
        let mut heading = 0.0f32;
        let mut point = Point::default();
        let mut points = vec![point];
        for (index, turn) in turns.into_iter().enumerate() {
            let turn = if closed {
                turn
            } else if right + left < 0.0001 {
                TAU / SAMPLES as f32
            } else if turn >= 0.0 {
                turn * right_target / right.max(0.0001)
            } else {
                turn * left_target / left.max(0.0001)
            };
            let direction = heading + turn * 0.5;
            point.horizontal += direction.sin() * step;
            point.vertical -= direction.cos() * step;
            heading += turn;
            if (index + 1).is_multiple_of(substeps) {
                points.push(point);
            }
        }
        let endpoint = point;
        for (index, point) in points.iter_mut().enumerate() {
            let progress = index as f32 / SAMPLES as f32;
            let closure = progress * progress * (3.0 - 2.0 * progress);
            point.horizontal -= endpoint.horizontal * closure;
            point.vertical -= endpoint.vertical * closure;
        }
        points[SAMPLES] = points[0];
        let minimum = Point {
            horizontal: points.iter().map(|point| point.horizontal).fold(f32::INFINITY, f32::min),
            vertical: points.iter().map(|point| point.vertical).fold(f32::INFINITY, f32::min),
        };
        let width = points.iter().map(|point| point.horizontal - minimum.horizontal).fold(0.0, f32::max);
        let height = points.iter().map(|point| point.vertical - minimum.vertical).fold(0.0, f32::max);
        let extent = width.max(height).max(0.001);
        for point in &mut points {
            point.horizontal = (point.horizontal - minimum.horizontal) / extent;
            point.vertical = (point.vertical - minimum.vertical) / extent;
        }
        Self { points, size: Point { horizontal: width / extent, vertical: height / extent }, length: track.length }
    }

    fn position(&self, distance: f32) -> Point {
        let position = distance.rem_euclid(self.length) / self.length * SAMPLES as f32;
        let index = (position as usize).min(SAMPLES - 1);
        let fraction = position - index as f32;
        let first = self.points[index];
        let second = self.points[index + 1];
        Point {
            horizontal: first.horizontal + (second.horizontal - first.horizontal) * fraction,
            vertical: first.vertical + (second.vertical - first.vertical) * fraction,
        }
    }
}

struct Layout {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    aspect: f32,
    labels: bool,
}

impl Layout {
    fn new(frame: &FrameBuf, aspect: f32) -> Option<Self> {
        if frame.w < 32 || frame.h < 20 || !aspect.is_finite() || aspect <= 0.0 {
            return None;
        }
        let width = (frame.h as f32 * 0.42).clamp(26.0, 164.0).min(frame.w as f32 * aspect * 0.30);
        let height = (width * 0.76).min(frame.h as f32 * 0.34);
        let margin = (frame.h as f32 * 0.022).clamp(2.0, 10.0);
        Some(Self {
            left: frame.w as f32 - (width + margin) / aspect,
            top: margin,
            width: width / aspect,
            height,
            aspect,
            labels: width >= 100.0 && height >= 70.0,
        })
    }

    fn project(&self, map: &TrackMap, point: Point) -> Point {
        let padding = if self.labels { 12.0 } else { 3.5 };
        let header = if self.labels { 17.0 } else { 0.0 };
        let footer = if self.labels { 15.0 } else { 0.0 };
        let available_width = (self.width * self.aspect - padding * 2.0).max(1.0);
        let available_height = (self.height - padding * 2.0 - header - footer).max(1.0);
        let scale = (available_width / map.size.horizontal.max(0.001)).min(available_height / map.size.vertical.max(0.001));
        Point {
            horizontal: self.left + (self.width * self.aspect - map.size.horizontal * scale) * 0.5 / self.aspect + point.horizontal * scale / self.aspect,
            vertical: self.top + padding + header + (available_height - map.size.vertical * scale) * 0.5 + point.vertical * scale,
        }
    }
}

fn stroke(frame: &mut FrameBuf, first: Point, second: Point, radius: f32, color: Rgb, aspect: f32) {
    let horizontal = (second.horizontal - first.horizontal) * aspect;
    let vertical = second.vertical - first.vertical;
    let length_squared = (horizontal * horizontal + vertical * vertical).max(0.0001);
    let extent = radius + 0.6;
    let left = (first.horizontal.min(second.horizontal) - extent / aspect).floor().max(0.0) as usize;
    let right = (first.horizontal.max(second.horizontal) + extent / aspect).ceil().min(frame.w as f32) as usize;
    let top = (first.vertical.min(second.vertical) - extent).floor().max(0.0) as usize;
    let bottom = (first.vertical.max(second.vertical) + extent).ceil().min(frame.h as f32) as usize;
    for pixel_y in top..bottom {
        for pixel_x in left..right {
            let offset_x = (pixel_x as f32 + 0.5 - first.horizontal) * aspect;
            let offset_y = pixel_y as f32 + 0.5 - first.vertical;
            let along = ((offset_x * horizontal + offset_y * vertical) / length_squared).clamp(0.0, 1.0);
            let distance = ((offset_x - horizontal * along).powi(2) + (offset_y - vertical * along).powi(2)).sqrt();
            frame.blend(pixel_x as i32, pixel_y as i32, color, (radius + 0.5 - distance).clamp(0.0, 1.0));
        }
    }
}

pub(crate) fn draw(frame: &mut FrameBuf, race: &Race, aspect: f32) {
    let Some(layout) = Layout::new(frame, aspect) else { return };
    let map = &race.track.minimap;
    for pixel_y in layout.top as i32..(layout.top + layout.height).ceil() as i32 {
        for pixel_x in layout.left as i32..(layout.left + layout.width).ceil() as i32 {
            let horizontal = ((pixel_x as f32 + 0.5 - layout.left - layout.width * 0.5) * aspect).abs();
            let vertical = (pixel_y as f32 + 0.5 - layout.top - layout.height * 0.5).abs();
            let corner = if layout.labels { 6.0 } else { 2.0 };
            let distance = ((horizontal - layout.width * aspect * 0.5 + corner).max(0.0).powi(2)
                + (vertical - layout.height * 0.5 + corner).max(0.0).powi(2)).sqrt() - corner;
            if distance <= 0.0 {
                frame.blend(pixel_x, pixel_y, PANEL, 0.90);
                if distance > -0.9 {
                    frame.blend(pixel_x, pixel_y, Rgb(86, 106, 138), 0.55);
                }
            }
        }
    }
    let points: Vec<_> = map.points.iter().map(|&point| layout.project(map, point)).collect();
    let radius = if layout.labels { 1.3 } else { 0.65 };
    for (width, color) in [(radius + 1.7, Rgb(4, 7, 17)), (radius, ROAD)] {
        for segment in points.windows(2) {
            stroke(frame, segment[0], segment[1], width, color, aspect);
        }
    }
    let start = points[0];
    let next = layout.project(map, map.position(map.length * 0.01));
    let direction_x = (next.horizontal - start.horizontal) * aspect;
    let direction_y = next.vertical - start.vertical;
    let length = (direction_x * direction_x + direction_y * direction_y).sqrt().max(0.001);
    let normal = Point { horizontal: -direction_y / length / aspect, vertical: direction_x / length };
    let flag_width = if layout.labels { 4.0 } else { 2.0 };
    for block in 0..4 {
        let offset = (block as f32 / 2.0 - 1.0) * flag_width;
        let first = Point { horizontal: start.horizontal + normal.horizontal * offset, vertical: start.vertical + normal.vertical * offset };
        let second = Point { horizontal: first.horizontal + normal.horizontal * flag_width * 0.5, vertical: first.vertical + normal.vertical * flag_width * 0.5 };
        stroke(frame, first, second, radius, if block % 2 == 0 { Rgb(255, 255, 255) } else { PANEL }, aspect);
    }
    let dot = if layout.labels { 1.8 } else { 1.0 };
    for rival in &race.rivals {
        let point = layout.project(map, map.position(rival.d));
        frame.ellipse(point.horizontal, point.vertical, (dot + 0.9) / aspect, dot + 0.9, PANEL, 1.0);
        frame.ellipse(point.horizontal, point.vertical, dot / aspect, dot, Rgb(123, 172, 203), 1.0);
    }
    let player = layout.project(map, map.position(race.d));
    let marker = if layout.labels { 3.1 } else { 1.6 };
    frame.ellipse(player.horizontal, player.vertical, (marker + 3.0) / aspect, marker + 3.0, PLAYER, 0.14);
    frame.ellipse(player.horizontal, player.vertical, (marker + 1.2) / aspect, marker + 1.2, PANEL, 1.0);
    frame.ellipse(player.horizontal, player.vertical, marker / aspect, marker, PLAYER, 1.0);
    if layout.labels {
        frame.ellipse(player.horizontal - 0.5 / aspect, player.vertical - 0.5, 1.0 / aspect, 1.0, Rgb(255, 249, 220), 1.0);
        let font = ((1.0 / aspect).round().max(1.0) as i32, 1);
        let left = (layout.left + 10.0 / aspect) as i32;
        draw_text(frame, "TRACK MAP", left, (layout.top + 8.0) as i32, font, Rgb(166, 182, 205), Rgb(166, 182, 205), None);
        let footer = layout.top + layout.height - 11.0;
        frame.ellipse(layout.left + 12.0 / aspect, footer + 3.0, 2.0 / aspect, 2.0, PLAYER, 1.0);
        draw_text(frame, "YOU", (layout.left + 19.0 / aspect) as i32, footer as i32, font, PLAYER, PLAYER, None);
        draw_text(frame, "S/F", (layout.left + layout.width - 28.0 / aspect) as i32, footer as i32, font, ROAD, ROAD, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::all_tracks;
    use std::rc::Rc;

    fn separation(first: Point, second: Point) -> f32 {
        (first.horizontal - second.horizontal).hypot(first.vertical - second.vertical)
    }

    #[test]
    fn every_track_has_a_finite_closed_distinct_map() {
        let tracks = all_tracks();
        for (index, track) in tracks.iter().enumerate() {
            let map = &track.minimap;
            assert_eq!(map.points.len(), SAMPLES + 1);
            assert!(separation(map.points[0], map.points[SAMPLES]) < 0.00001);
            assert!(map.size.horizontal > 0.1 && map.size.vertical > 0.1);
            for point in &map.points {
                assert!(point.horizontal.is_finite() && point.vertical.is_finite());
                assert!((0.0..=1.00001).contains(&point.horizontal));
                assert!((0.0..=1.00001).contains(&point.vertical));
            }
            for other in &tracks[..index] {
                assert!(map.points.iter().zip(&other.minimap.points).any(|(&first, &second)| separation(first, second) > 0.1));
            }
        }
    }

    #[test]
    fn midnight_map_has_deep_infield_sections_without_crossings() {
        let track = all_tracks().into_iter().find(|track| track.name == "Tokyo Midnight").unwrap();
        let map = &track.minimap;
        let perimeter: f32 = map.points.windows(2).map(|pair| separation(pair[0], pair[1])).sum();
        let area = map.points.windows(2).map(|pair| pair[0].horizontal * pair[1].vertical - pair[1].horizontal * pair[0].vertical).sum::<f32>().abs() * 0.5;
        let circularity = 2.0 * TAU * area / (perimeter * perimeter);
        assert!(circularity < 0.55, "Tokyo must not return to a circular outline: {circularity}");
        let cross = |start: Point, end: Point, point: Point| {
            (end.horizontal - start.horizontal) * (point.vertical - start.vertical)
                - (end.vertical - start.vertical) * (point.horizontal - start.horizontal)
        };
        for first in 0..SAMPLES {
            for second in first + 2..SAMPLES {
                if first == 0 && second == SAMPLES - 1 { continue; }
                let first_start = map.points[first];
                let first_end = map.points[first + 1];
                let second_start = map.points[second];
                let second_end = map.points[second + 1];
                let intersects = cross(first_start, first_end, second_start) * cross(first_start, first_end, second_end) < 0.0
                    && cross(second_start, second_end, first_start) * cross(second_start, second_end, first_end) < 0.0;
                assert!(!intersects, "Tokyo map crosses itself at samples {first} and {second}");
            }
        }
    }

    #[test]
    fn player_position_follows_distance_and_wraps_at_the_finish_line() {
        for track in all_tracks() {
            let map = &track.minimap;
            for index in 0..SAMPLES {
                let distance = index as f32 / SAMPLES as f32 * track.length;
                assert!(separation(map.position(distance), map.points[index]) < 0.00001);
                let midpoint = map.position(distance + track.length / SAMPLES as f32 * 0.5);
                let expected = Point {
                    horizontal: (map.points[index].horizontal + map.points[index + 1].horizontal) * 0.5,
                    vertical: (map.points[index].vertical + map.points[index + 1].vertical) * 0.5,
                };
                assert!(separation(midpoint, expected) < 0.00001);
            }
            for fraction in [0.0, 0.13, 0.41, 0.77] {
                let distance = fraction * track.length;
                assert!(separation(map.position(distance), map.position(distance + track.length)) < 0.00001);
                assert!(separation(map.position(distance), map.position(distance - track.length)) < 0.00001);
            }
            assert!(separation(map.position(-0.1), map.position(0.1)) < 0.005);
            assert!(separation(map.position(track.length * 0.25), map.position(track.length * 0.75)) > 0.1);
        }
    }

    #[test]
    fn rivals_have_a_distinct_marker_and_player_stays_visible_in_a_cluster() {
        for track in all_tracks() {
            let mut race = Race::new(Rc::new(track), 3, true, false);
            race.d = 0.0;
            race.rivals.truncate(1);
            race.rivals[0].d = race.track.length * 0.5;
            let mut frame = FrameBuf::new();
            frame.resize(800, 448);
            let layout = Layout::new(&frame, 1.0).unwrap();
            draw(&mut frame, &race, 1.0);
            let rival = layout.project(&race.track.minimap, race.track.minimap.position(race.rivals[0].d));
            assert_eq!(frame.get(rival.horizontal as usize, rival.vertical as usize), Rgb(123, 172, 203));
            race.rivals[0].d = race.d;
            draw(&mut frame, &race, 1.0);
            let player = layout.project(&race.track.minimap, race.track.minimap.position(race.d));
            let color = frame.get(player.horizontal as usize, player.vertical as usize);
            assert!(color.0 > 200 && color.1 > 150 && color.2 < 230);
        }
    }

    #[test]
    fn map_and_player_marker_fit_all_graphics_modes() {
        for track in all_tracks() {
            let mut race = Race::new(Rc::new(track), 3, true, false);
            for (width, height, aspect) in [(800, 448, 1.0), (160, 84, 0.5), (80, 84, 1.0), (40, 26, 1.0)] {
                let mut frame = FrameBuf::new();
                frame.resize(width, height);
                let layout = Layout::new(&frame, aspect).unwrap();
                for invalid_aspect in [0.0, -1.0, f32::INFINITY, f32::NAN] {
                    assert!(Layout::new(&frame, invalid_aspect).is_none());
                }
                assert!(layout.left >= 0.0 && layout.top >= 0.0);
                assert!(layout.left + layout.width <= width as f32);
                assert!(layout.top + layout.height <= height as f32);
                for progress in [0.0, 0.25, 0.5, 0.75] {
                    race.d = race.track.length * progress;
                    draw(&mut frame, &race, aspect);
                    let marker = layout.project(&race.track.minimap, race.track.minimap.position(race.d));
                    let color = frame.get(marker.horizontal as usize, marker.vertical as usize);
                    assert!(color.0 > 200 && color.1 > 150 && color.2 < 230, "player marker lost at {width}x{height}");
                }
            }
        }
        let mut tiny = FrameBuf::new();
        tiny.resize(1, 1);
        assert!(Layout::new(&tiny, 1.0).is_none());
    }
}
