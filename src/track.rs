//! Track layouts (curves and hills), colour palettes and roadside scenery.

use crate::gfx::Rgb;
use crate::minimap::TrackMap;
use crate::music::Tune;
use crate::sprites::{self, Bitmap, BILLBOARD_TEXT, NEON_TEXT, ROAD_SIGNS};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::f32::consts::PI;

/// Half the road width, in metres.
pub const ROAD_HALF: f32 = 5.0;

#[derive(Clone, Copy)]
pub struct Segment {
    pub len: f32,
    /// Lateral curvature (1/m). Positive bends right.
    pub curve: f32,
    /// Height change over the segment (m), eased with a cosine.
    pub hill: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Backdrop {
    Mountains { snow: bool },
    City,
    Midnight,
    /// Retro striped sun, neon wireframe mountains and a Tokyo skyline.
    Synth,
    /// Low winter sun, a snow-capped volcano and a pastel town with a
    /// bullet train gliding past.
    Winter,
}

#[derive(Clone, Copy)]
pub struct Palette {
    /// Sky gradient: top, middle, horizon.
    pub sky: [Rgb; 3],
    pub clouds: Option<Rgb>,
    pub far_hills: Rgb,
    pub near_hills: Rgb,
    pub backdrop: Backdrop,
    pub grass: [Rgb; 2],
    /// Neon grid lines drawn over the ground instead of grass stripes.
    pub grid: Option<Rgb>,
    pub road: [Rgb; 2],
    pub rumble: [Rgb; 2],
    pub lane: Rgb,
    pub fog: Rgb,
    /// Brightness multiplier for sprites (night tracks are darker).
    pub light: f32,
    pub stars: bool,
    pub sun: Option<Rgb>,
    /// The player's car colour.
    pub car: Rgb,
    /// Glittering snow on the ground.
    pub snow: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Prop {
    Tree,
    Pine,
    Bush,
    Rock,
    Chevron { left: bool },
    Tyres,
    Billboard { text: u8, color: Rgb },
    Lamp,
    Gantry,
    Grandstand,
    Palm,
    Sakura,
    Torii,
    NeonSign { text: u8, color: Rgb },
    NeonBoard { text: u8, color: Rgb },
    NeonChevron { left: bool },
    SnowCedar,
    Birch,
    Lantern,
    Vending { color: Rgb },
    SnowTorii,
    RoadSign { text: u8 },
    Snowman,
    SnowPole,
    CityBuilding { variant: u8, color: Rgb },
}

impl Prop {
    pub fn bitmap(self) -> &'static Bitmap {
        let s = sprites::get();
        match self {
            Prop::Tree => &s.tree,
            Prop::Pine => &s.pine,
            Prop::Bush => &s.bush,
            Prop::Rock => &s.rock,
            Prop::Chevron { .. } => &s.chevron,
            Prop::Tyres => &s.tyres,
            Prop::Billboard { text, .. } => &s.billboards[text as usize],
            Prop::Lamp => &s.lamp,
            Prop::Gantry => &s.gantry,
            Prop::Grandstand => &s.grandstand,
            Prop::Palm => &s.palm,
            Prop::Sakura => &s.sakura,
            Prop::Torii => &s.torii,
            Prop::NeonSign { text, .. } => &s.neon_signs[text as usize],
            Prop::NeonBoard { text, .. } => &s.neon_boards[text as usize],
            Prop::NeonChevron { .. } => &s.neon_chevron,
            Prop::SnowCedar => &s.snow_cedar,
            Prop::Birch => &s.birch,
            Prop::Lantern => &s.lantern,
            Prop::Vending { .. } => &s.vending,
            Prop::SnowTorii => &s.snow_torii,
            Prop::RoadSign { text } => &s.road_signs[text as usize],
            Prop::Snowman => &s.snowman,
            Prop::SnowPole => &s.snow_pole,
            Prop::CityBuilding { variant, .. } => &s.city_buildings[variant as usize],
        }
    }

    /// Width in metres.
    pub fn width(self) -> f32 {
        match self {
            Prop::Tree => 6.0,
            Prop::Pine => 4.5,
            Prop::Bush => 3.0,
            Prop::Rock => 3.5,
            Prop::Chevron { .. } => 2.2,
            Prop::Tyres => 2.6,
            Prop::Billboard { .. } => 9.0,
            Prop::Lamp => 1.6,
            Prop::Gantry => 18.0,
            Prop::Grandstand => 30.0,
            Prop::Palm => 7.5,
            Prop::Sakura => 6.0,
            Prop::Torii => 18.0,
            Prop::NeonSign { .. } => 2.4,
            Prop::NeonBoard { .. } => 9.0,
            Prop::NeonChevron { .. } => 2.2,
            Prop::SnowCedar => 5.0,
            Prop::Birch => 5.5,
            Prop::Lantern => 1.4,
            Prop::Vending { .. } => 1.2,
            Prop::SnowTorii => 18.0,
            Prop::RoadSign { .. } => 9.0,
            Prop::Snowman => 1.3,
            Prop::SnowPole => 0.8,
            Prop::CityBuilding { .. } => 14.0,
        }
    }

    /// Solid parts you can crash into: (lateral offset, half-width) in metres.
    pub fn solid(self) -> &'static [(f32, f32)] {
        match self {
            Prop::Tree | Prop::Pine | Prop::Sakura => &[(0.0, 0.7)],
            Prop::SnowCedar | Prop::Birch | Prop::Vending { .. } => &[(0.0, 0.6)],
            Prop::Lantern | Prop::Snowman => &[(0.0, 0.5)],
            Prop::SnowPole => &[(0.0, 0.2)],
            Prop::RoadSign { .. } => &[(0.0, 4.0)],
            Prop::Palm => &[(-0.9, 0.5)],
            Prop::Rock => &[(0.0, 1.4)],
            Prop::Chevron { .. } | Prop::NeonChevron { .. } => &[(0.0, 0.5)],
            Prop::Tyres => &[(0.0, 1.2)],
            Prop::Billboard { .. } | Prop::NeonBoard { .. } => &[(0.0, 4.0)],
            Prop::Lamp => &[(0.0, 0.3)],
            Prop::NeonSign { .. } => &[(0.0, 0.4)],
            // Pillars either side of the road.
            Prop::Torii | Prop::SnowTorii => &[(-7.9, 0.7), (7.9, 0.7)],
            Prop::Gantry => &[(-8.7, 0.4), (8.7, 0.4)],
            Prop::Bush | Prop::Grandstand => &[],
            Prop::CityBuilding { .. } => &[(0.0, 6.5)],
        }
    }

    pub fn flipped(self) -> bool {
        matches!(self, Prop::Chevron { left: true } | Prop::NeonChevron { left: true })
    }

    pub fn body(self) -> Rgb {
        match self {
            Prop::Billboard { color, .. }
            | Prop::NeonSign { color, .. }
            | Prop::NeonBoard { color, .. }
            | Prop::Vending { color }
            | Prop::CityBuilding { color, .. } => color,
            Prop::NeonChevron { .. } => Rgb(40, 230, 255),
            _ => Rgb(255, 255, 255),
        }
    }
}

pub struct Scenery {
    pub d: f32,
    pub x: f32,
    pub kind: Prop,
}

pub struct Track {
    pub name: &'static str,
    pub desc: &'static str,
    pub palette: Palette,
    segments: Vec<Segment>,
    starts: Vec<f32>,
    base_heights: Vec<f32>,
    pub length: f32,
    pub(crate) minimap: TrackMap,
    pub scenery: Vec<Scenery>,
    /// Soundtrack exclusive to this track, if any.
    pub music: Option<Tune>,
    /// One of the original tracks, hidden from the menu.
    pub classic: bool,
}

#[derive(Clone, Copy)]
enum Style {
    Parkland,
    Mountain,
    Night,
    Synth,
    Winter,
    Midnight,
}

impl Track {
    fn build(
        name: &'static str,
        desc: &'static str,
        palette: Palette,
        segs: &[(f32, f32, f32)],
        style: Style,
        seed: u64,
    ) -> Track {
        let segments: Vec<Segment> = segs.iter().map(|&(len, curve, hill)| Segment { len, curve, hill }).collect();
        debug_assert!(segments.iter().map(|s| s.hill).sum::<f32>().abs() < 0.01, "{name}: hills must loop");
        let (mut starts, mut base_heights) = (Vec::new(), Vec::new());
        let (mut acc, mut height) = (0.0, 0.0);
        for s in &segments {
            starts.push(acc);
            base_heights.push(height);
            acc += s.len;
            height += s.hill;
        }
        let music = match style {
            Style::Synth => Some(Tune::NeonSakura),
            Style::Winter => Some(Tune::Shirayuki),
            Style::Midnight => Some(Tune::TokyoMidnight),
            _ => None,
        };
        let classic = music.is_none();
        let mut t = Track { name, desc, palette, segments, starts, base_heights, length: acc, minimap: TrackMap::default(), scenery: Vec::new(), music, classic };
        t.minimap = if matches!(style, Style::Midnight) { TrackMap::from_closed_track(&t) } else { TrackMap::from_track(&t) };
        t.generate_scenery(style, seed);
        t
    }

    fn locate(&self, d: f32) -> (usize, f32) {
        let d = d.rem_euclid(self.length);
        let i = match self.starts.binary_search_by(|s| s.partial_cmp(&d).unwrap()) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        (i, d - self.starts[i])
    }

    /// Curvature at distance `d`, eased in and out at segment boundaries.
    pub fn curve_at(&self, d: f32) -> f32 {
        let (i, t) = self.locate(d);
        let seg = &self.segments[i];
        if seg.curve == 0.0 {
            return 0.0;
        }
        let ramp = (seg.len / 3.0).min(60.0);
        let k = (t / ramp).min((seg.len - t) / ramp).clamp(0.0, 1.0);
        seg.curve * k * k * (3.0 - 2.0 * k)
    }

    pub fn height_at(&self, d: f32) -> f32 {
        let (i, t) = self.locate(d);
        let seg = &self.segments[i];
        self.base_heights[i] + seg.hill * (1.0 - (PI * t / seg.len).cos()) * 0.5
    }

    /// dh/dd — positive when climbing.
    pub fn slope_at(&self, d: f32) -> f32 {
        let (i, t) = self.locate(d);
        let seg = &self.segments[i];
        seg.hill * PI / (2.0 * seg.len) * (PI * t / seg.len).sin()
    }

    fn generate_scenery(&mut self, style: Style, seed: u64) {
        if matches!(style, Style::Midnight) {
            self.generate_midnight_scenery(seed);
            return;
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let winter = matches!(style, Style::Winter);
        let ads = if winter {
            // Soft, sun-faded pastels.
            [Rgb(90, 160, 220), Rgb(240, 120, 110), Rgb(245, 180, 80), Rgb(110, 185, 150), Rgb(170, 130, 210)]
        } else {
            [Rgb(200, 20, 30), Rgb(20, 70, 200), Rgb(230, 130, 0), Rgb(20, 140, 60), Rgb(120, 30, 160)]
        };
        let mut out = vec![Scenery { d: 0.0, x: 0.0, kind: Prop::Gantry }];
        let neon = [Rgb(255, 50, 190), Rgb(40, 230, 255), Rgb(255, 200, 60), Rgb(180, 90, 255)];
        let synth = matches!(style, Style::Synth);
        let billboard = |rng: &mut StdRng| {
            if synth {
                Prop::NeonBoard { text: rng.gen_range(0..NEON_TEXT.len()) as u8, color: neon[rng.gen_range(0..neon.len())] }
            } else {
                Prop::Billboard {
                    text: rng.gen_range(0..BILLBOARD_TEXT.len()) as u8,
                    color: ads[rng.gen_range(0..ads.len())],
                }
            }
        };

        // Grandstands along the start/finish straight.
        for k in -2..=3 {
            let d = (k as f32 * 34.0 - 20.0).rem_euclid(self.length);
            out.push(Scenery { d, x: ROAD_HALF + 19.0, kind: Prop::Grandstand });
            if k % 2 == 0 {
                out.push(Scenery { d: d + 10.0, x: -(ROAD_HALF + 6.0), kind: billboard(&mut rng) });
            }
        }

        if let Style::Night = style {
            let mut d = 0.0;
            while d < self.length {
                for side in [-1.0, 1.0] {
                    out.push(Scenery { d, x: side * (ROAD_HALF + 2.5), kind: Prop::Lamp });
                }
                d += 40.0;
            }
        }

        // A tunnel of torii gates over the long downhill straight.
        let mut torii = Vec::new();
        if synth {
            let mut d = 2110.0;
            while d < 2290.0 {
                torii.push(d);
                out.push(Scenery { d, x: 0.0, kind: Prop::Torii });
                d += 12.0;
            }
        }

        // A shrine approach: snowy torii over the road, lanterns either side.
        if winter {
            for k in 0..3 {
                let d = 1440.0 + k as f32 * 40.0;
                torii.push(d);
                out.push(Scenery { d, x: 0.0, kind: Prop::SnowTorii });
            }
            let mut d = 1420.0;
            while d < 1570.0 {
                for side in [-1.0, 1.0] {
                    out.push(Scenery { d, x: side * (ROAD_HALF + 2.0), kind: Prop::Lantern });
                }
                d += 10.0;
            }
        }
        let shrine = |d: f32| winter && (1410.0..1580.0).contains(&d);

        let mut d = 120.0;
        let mut flip = false;
        let mut pole = false;
        while d < self.length - 120.0 {
            let c = self.curve_at(d);
            if c.abs() > 0.004 {
                // Warning chevrons and tyre walls on the outside of the bend.
                let outer = if c > 0.0 { -1.0 } else { 1.0 };
                let kind = match (flip, synth) {
                    (true, _) => Prop::Tyres,
                    (false, true) => Prop::NeonChevron { left: c < 0.0 },
                    (false, false) => Prop::Chevron { left: c < 0.0 },
                };
                flip = !flip;
                out.push(Scenery { d, x: outer * (ROAD_HALF + 2.2), kind });
                d += 14.0;
                continue;
            }
            let near_torii = torii.iter().any(|&t| (t - d).abs() < 8.0);
            if winter {
                pole = !pole;
                for side in [-1.0f32, 1.0] {
                    // Arrow poles marking the road edge under deep snow.
                    if pole && !shrine(d) {
                        out.push(Scenery { d, x: side * (ROAD_HALF + 1.6), kind: Prop::SnowPole });
                    }
                    if shrine(d) {
                        // Cedars standing behind the lanterns.
                        out.push(Scenery { d: d + 5.0, x: side * (ROAD_HALF + rng.gen_range(6.0..9.0)), kind: Prop::SnowCedar });
                        continue;
                    }
                    let r: f32 = rng.gen();
                    let (kind, x) = match r {
                        _ if r < 0.03 => (Prop::Vending { color: [Rgb(220, 40, 48), Rgb(40, 110, 210), Rgb(240, 240, 244)][rng.gen_range(0..3)] }, 4.2),
                        _ if r < 0.06 => (Prop::Snowman, rng.gen_range(4.0..7.0)),
                        _ if r < 0.09 => (Prop::RoadSign { text: rng.gen_range(0..ROAD_SIGNS.len()) as u8 }, 6.5),
                        _ if r < 0.12 => (Prop::Lantern, 4.0),
                        _ if r < 0.55 => (Prop::SnowCedar, rng.gen_range(4.5..9.0)),
                        _ if r < 0.75 => (Prop::Birch, rng.gen_range(4.5..9.0)),
                        _ => (Prop::SnowCedar, rng.gen_range(10.0..18.0)),
                    };
                    out.push(Scenery { d: d + 4.0, x: side * (ROAD_HALF + x), kind });
                    // A second row of forest further out.
                    if rng.gen_bool(0.6) {
                        let kind = if rng.gen_bool(0.75) { Prop::SnowCedar } else { Prop::Birch };
                        out.push(Scenery { d: d + 11.0, x: side * (ROAD_HALF + rng.gen_range(11.0..20.0)), kind });
                    }
                }
                d += 16.0;
                continue;
            }
            for side in [-1.0f32, 1.0] {
                if near_torii {
                    continue;
                }
                if synth {
                    // A palm-lined boulevard, with signs and blossoms further out.
                    let kind = if rng.gen_bool(0.16) {
                        Prop::NeonSign { text: rng.gen_range(0..NEON_TEXT.len()) as u8, color: neon[rng.gen_range(0..neon.len())] }
                    } else {
                        Prop::Palm
                    };
                    out.push(Scenery { d, x: side * (ROAD_HALF + rng.gen_range(3.4..4.2)), kind });
                    if rng.gen_bool(0.06) {
                        out.push(Scenery { d: d + 6.0, x: side * (ROAD_HALF + 8.0), kind: billboard(&mut rng) });
                    } else if rng.gen_bool(0.55) {
                        let kind = if rng.gen_bool(0.6) { Prop::Sakura } else { Prop::Palm };
                        out.push(Scenery { d: d + 7.0, x: side * (ROAD_HALF + rng.gen_range(8.0..16.0)), kind });
                    }
                } else if rng.gen_bool(0.07) {
                    out.push(Scenery { d, x: side * (ROAD_HALF + 6.0), kind: billboard(&mut rng) });
                } else if rng.gen_bool(0.7) {
                    let r: f32 = rng.gen();
                    let kind = match style {
                        Style::Parkland if r < 0.65 => Prop::Tree,
                        Style::Parkland => Prop::Bush,
                        Style::Mountain if r < 0.7 => Prop::Pine,
                        Style::Mountain if r < 0.85 => Prop::Rock,
                        Style::Mountain => Prop::Bush,
                        Style::Night if r < 0.5 => Prop::Tree,
                        Style::Night | Style::Synth | Style::Winter | Style::Midnight => Prop::Bush,
                    };
                    let x = side * (ROAD_HALF + rng.gen_range(3.5..14.0));
                    out.push(Scenery { d, x, kind });
                }
            }
            if synth {
                d += 16.0;
                continue;
            }
            d += rng.gen_range(10.0..24.0);
        }
        out.sort_by(|a, b| a.d.partial_cmp(&b.d).unwrap());
        self.scenery = out;
    }

    fn generate_midnight_scenery(&mut self, seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);
        let neon = [Rgb(255, 65, 176), Rgb(55, 222, 255), Rgb(164, 110, 255), Rgb(255, 180, 90)];
        let mut out = vec![Scenery { d: 0.0, x: 0.0, kind: Prop::Gantry }];
        let mut distance = 0.0;
        let mut block = 0usize;
        while distance < self.length {
            let shrine = (1620.0..1920.0).contains(&distance);
            for (side_index, side) in [-1.0, 1.0].into_iter().enumerate() {
                let color = neon[(block + side_index) % neon.len()];
                let variant = if shrine { 3 } else { rng.gen_range(0..3) };
                out.push(Scenery {
                    d: distance,
                    x: side * (ROAD_HALF + 14.0 + rng.gen_range(0.0..3.0)),
                    kind: Prop::CityBuilding { variant, color },
                });
                if block.is_multiple_of(2) {
                    out.push(Scenery { d: distance + 5.0, x: side * (ROAD_HALF + 2.8), kind: Prop::Lamp });
                    out.push(Scenery {
                        d: distance + 12.0,
                        x: side * (ROAD_HALF + 7.0),
                        kind: Prop::NeonSign { text: ((block / 2 + side_index) % NEON_TEXT.len()) as u8, color },
                    });
                }
                if shrine {
                    out.push(Scenery { d: distance + 16.0, x: side * (ROAD_HALF + 3.0), kind: Prop::Lantern });
                    if block.is_multiple_of(2) {
                        out.push(Scenery { d: distance + 20.0, x: side * (ROAD_HALF + 8.0), kind: Prop::Sakura });
                    }
                } else if block.is_multiple_of(5) {
                    out.push(Scenery { d: distance + 18.0, x: side * (ROAD_HALF + 4.8), kind: Prop::Vending { color } });
                }
            }
            let curve = self.curve_at(distance + 12.0);
            if curve.abs() > 0.003 {
                let outer = if curve > 0.0 { -1.0 } else { 1.0 };
                out.push(Scenery { d: distance + 12.0, x: outer * (ROAD_HALF + 1.7), kind: Prop::NeonChevron { left: curve < 0.0 } });
            }
            if block % 9 == 4 {
                out.push(Scenery {
                    d: distance + 8.0,
                    x: (if block.is_multiple_of(2) { 1.0 } else { -1.0 }) * (ROAD_HALF + 8.0),
                    kind: Prop::NeonBoard { text: (block % NEON_TEXT.len()) as u8, color: neon[block % neon.len()] },
                });
            }
            distance += 32.0;
            block += 1;
        }
        for distance in [1670.0, 1740.0, 1810.0] {
            out.push(Scenery { d: distance, x: 0.0, kind: Prop::Torii });
        }
        for object in &mut out {
            object.d = object.d.rem_euclid(self.length);
        }
        out.sort_by(|first, second| first.d.total_cmp(&second.d));
        self.scenery = out;
    }
}

const CAR_RED: Rgb = Rgb(225, 25, 35);

pub fn all_tracks() -> Vec<Track> {
    let day = Palette {
        sky: [Rgb(28, 84, 200), Rgb(76, 146, 236), Rgb(178, 220, 255)],
        clouds: Some(Rgb(255, 255, 255)),
        far_hills: Rgb(104, 140, 190),
        near_hills: Rgb(48, 118, 58),
        backdrop: Backdrop::Mountains { snow: false },
        grass: [Rgb(62, 170, 60), Rgb(50, 152, 52)],
        grid: None,
        road: [Rgb(112, 112, 118), Rgb(104, 104, 110)],
        rumble: [Rgb(220, 30, 30), Rgb(240, 240, 240)],
        lane: Rgb(238, 238, 238),
        fog: Rgb(178, 220, 255),
        light: 1.0,
        stars: false,
        sun: Some(Rgb(255, 248, 200)),
        car: CAR_RED,
        snow: false,
    };
    let sunset = Palette {
        sky: [Rgb(48, 22, 92), Rgb(196, 72, 96), Rgb(255, 170, 80)],
        clouds: Some(Rgb(255, 170, 150)),
        far_hills: Rgb(116, 62, 112),
        near_hills: Rgb(56, 38, 66),
        backdrop: Backdrop::Mountains { snow: true },
        grass: [Rgb(92, 132, 42), Rgb(78, 114, 36)],
        grid: None,
        road: [Rgb(106, 96, 102), Rgb(97, 88, 94)],
        rumble: [Rgb(232, 232, 232), Rgb(32, 32, 32)],
        lane: Rgb(242, 222, 164),
        fog: Rgb(236, 140, 96),
        light: 0.88,
        stars: false,
        sun: Some(Rgb(255, 214, 96)),
        car: CAR_RED,
        snow: false,
    };
    let night = Palette {
        sky: [Rgb(2, 4, 18), Rgb(12, 14, 46), Rgb(40, 36, 92)],
        clouds: None,
        far_hills: Rgb(22, 24, 56),
        near_hills: Rgb(10, 14, 28),
        backdrop: Backdrop::City,
        grass: [Rgb(20, 60, 40), Rgb(16, 50, 34)],
        grid: None,
        road: [Rgb(62, 62, 74), Rgb(56, 56, 68)],
        rumble: [Rgb(240, 200, 0), Rgb(30, 30, 40)],
        lane: Rgb(232, 232, 128),
        fog: Rgb(24, 24, 60),
        light: 0.72,
        stars: true,
        sun: Some(Rgb(232, 232, 214)),
        car: CAR_RED,
        snow: false,
    };
    let synthwave = Palette {
        sky: [Rgb(20, 6, 52), Rgb(128, 24, 124), Rgb(255, 118, 92)],
        clouds: None,
        far_hills: Rgb(40, 12, 66),
        near_hills: Rgb(26, 8, 44),
        backdrop: Backdrop::Synth,
        grass: [Rgb(26, 8, 46), Rgb(26, 8, 46)],
        grid: Some(Rgb(255, 46, 196)),
        road: [Rgb(42, 28, 66), Rgb(36, 24, 58)],
        rumble: [Rgb(255, 46, 180), Rgb(40, 220, 255)],
        lane: Rgb(130, 240, 255),
        fog: Rgb(150, 44, 128),
        light: 0.95,
        stars: true,
        sun: Some(Rgb(255, 214, 90)),
        car: Rgb(150, 64, 255),
        snow: false,
    };
    let winter = Palette {
        sky: [Rgb(52, 112, 204), Rgb(128, 182, 238), Rgb(255, 224, 194)],
        clouds: None,
        far_hills: Rgb(116, 140, 194),
        near_hills: Rgb(34, 70, 70),
        backdrop: Backdrop::Winter,
        grass: [Rgb(238, 242, 252), Rgb(226, 234, 248)],
        grid: None,
        road: [Rgb(84, 88, 106), Rgb(78, 82, 100)],
        rumble: [Rgb(222, 44, 52), Rgb(246, 246, 250)],
        lane: Rgb(250, 250, 252),
        fog: Rgb(218, 226, 244),
        light: 1.0,
        stars: false,
        sun: Some(Rgb(255, 238, 196)),
        car: Rgb(24, 178, 170),
        snow: true,
    };

    let midnight = Palette {
        sky: [Rgb(3, 5, 20), Rgb(12, 13, 40), Rgb(52, 28, 78)],
        clouds: None,
        far_hills: Rgb(22, 24, 48),
        near_hills: Rgb(12, 16, 30),
        backdrop: Backdrop::Midnight,
        grass: [Rgb(21, 23, 40), Rgb(24, 26, 44)],
        grid: None,
        road: [Rgb(34, 39, 58), Rgb(30, 34, 50)],
        rumble: [Rgb(235, 55, 155), Rgb(42, 200, 235)],
        lane: Rgb(174, 218, 238),
        fog: Rgb(35, 28, 66),
        light: 0.86,
        stars: true,
        sun: Some(Rgb(202, 225, 245)),
        car: Rgb(150, 64, 255),
        snow: false,
    };

    vec![
        Track::build(
            "Danube Ring",
            "Sunny parkland with fast, rolling bends",
            day,
            &[
                (300.0, 0.0, 0.0), (200.0, 0.004, 10.0), (150.0, 0.0, 0.0), (250.0, -0.006, -10.0),
                (200.0, 0.0, 12.0), (150.0, 0.009, 0.0), (100.0, 0.0, -12.0), (300.0, -0.003, 8.0),
                (250.0, 0.0, -8.0), (200.0, 0.007, 0.0), (150.0, -0.007, 15.0), (300.0, 0.0, -15.0),
                (250.0, 0.005, 0.0), (200.0, 0.0, 0.0),
            ],
            Style::Parkland,
            1,
        ),
        Track::build(
            "Carpathian Circuit",
            "Twisty mountain road with steep crests",
            sunset,
            &[
                (250.0, 0.0, 0.0), (150.0, 0.010, 15.0), (100.0, 0.0, 0.0), (150.0, -0.010, 10.0),
                (200.0, 0.0, -10.0), (300.0, 0.004, -15.0), (120.0, -0.012, 0.0), (150.0, 0.0, 20.0),
                (200.0, 0.008, -5.0), (200.0, -0.005, -15.0), (250.0, 0.0, 12.0), (150.0, 0.011, 0.0),
                (100.0, -0.008, -12.0), (300.0, 0.0, 18.0), (180.0, 0.006, -18.0), (200.0, 0.0, 0.0),
            ],
            Style::Mountain,
            2,
        ),
        Track::build(
            "Baltic Night GP",
            "Long straights and hairpins under the lights",
            night,
            &[
                (500.0, 0.0, 0.0), (150.0, 0.012, 0.0), (400.0, 0.0, 10.0), (200.0, -0.004, -10.0),
                (150.0, -0.011, 0.0), (450.0, 0.0, 6.0), (250.0, 0.006, -6.0), (200.0, 0.0, 0.0),
                (150.0, -0.012, 0.0), (350.0, 0.0, 8.0), (200.0, 0.003, -8.0), (300.0, 0.0, 0.0),
            ],
            Style::Night,
            3,
        ),
        Track::build(
            "Neon Sakura",
            "Synthwave sunset: palms, torii and neon",
            synthwave,
            &[
                (400.0, 0.0, 0.0), (250.0, 0.005, 8.0), (200.0, 0.0, -8.0), (300.0, -0.007, 0.0),
                (150.0, 0.0, 12.0), (250.0, 0.010, -6.0), (200.0, 0.0, -6.0), (350.0, -0.004, 10.0),
                (200.0, 0.0, -10.0), (180.0, 0.011, 0.0), (300.0, 0.0, 6.0), (220.0, -0.008, -6.0),
                (250.0, 0.003, 0.0), (250.0, 0.0, 0.0),
            ],
            Style::Synth,
            4,
        ),
        Track::build(
            "Shirayuki Pass",
            "Winter sun: snowy cedars, shrines and city pop",
            winter,
            &[
                (350.0, 0.0, 0.0), (220.0, 0.006, 10.0), (180.0, 0.0, 8.0), (260.0, -0.008, 0.0),
                (200.0, 0.0, -12.0), (160.0, 0.012, 0.0), (240.0, 0.0, 6.0), (300.0, -0.005, -6.0),
                (220.0, 0.0, 14.0), (180.0, 0.009, 0.0), (200.0, -0.010, -8.0), (280.0, 0.0, -6.0),
                (220.0, 0.004, 0.0), (260.0, 0.0, -6.0),
            ],
            Style::Winter,
            5,
        ),
        Track::build(
            "Tokyo Midnight",
            "22 turns through neon boulevards, hairpins and a winding infield",
            midnight,
            &[
                (80.5752, 0.0, 0.0), (194.6673, 0.0095, 0.0), (298.1802, 0.0, 0.0),
                (72.8642, 0.006, 0.0), (199.6787, 0.0, 0.0), (117.8097, 0.010, 0.0),
                (57.9045, 0.0, 0.0), (117.8097, 0.010, 0.0), (178.2886, 0.0, 4.0),
                (168.2996, 0.014, 4.0), (11.4849, 0.0, 0.0), (168.2996, 0.014, 0.0),
                (11.4849, 0.0, 0.0), (168.2996, -0.014, 0.0), (11.4849, 0.0, 0.0),
                (168.2996, -0.014, 0.0), (182.6074, 0.0, -4.0), (234.5329, -0.009, -4.0),
                (306.9111, 0.0, 5.0), (117.8097, 0.010, 0.0), (57.9045, 0.0, 0.0),
                (117.8097, 0.010, 0.0), (249.4477, 0.0, 0.0), (190.8997, 0.012, 0.0),
                (82.6440, 0.0, -5.0), (168.2996, 0.014, 0.0), (53.9849, 0.0, 0.0),
                (168.2996, -0.014, 0.0), (11.4849, 0.0, 0.0), (168.2996, -0.014, 0.0),
                (202.6477, 0.0, 0.0), (202.7997, 0.011, 0.0), (156.9514, 0.0, 0.0),
                (117.8097, 0.010, 0.0), (57.9045, 0.0, 0.0), (117.8097, 0.010, 0.0),
                (245.8641, 0.0, 0.0), (196.591, -0.0115, 0.0), (36.5604, 0.0, 0.0),
                (168.2996, 0.014, 0.0), (84.5582, 0.0, 0.0), (132.4707, 0.0105, 0.0),
                (72.6314, 0.0, 0.0), (132.4707, -0.0105, 0.0), (80.5752, 0.0, 0.0),
            ],
            Style::Midnight,
            6,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midnight_is_selectable_and_has_more_turns_than_every_other_track() {
        let tracks = all_tracks();
        let midnight = tracks.iter().find(|track| track.name == "Tokyo Midnight").unwrap();
        let turns = |track: &Track| track.segments.iter().filter(|segment| segment.curve != 0.0).count();
        assert!(!midnight.classic);
        assert_eq!(midnight.music, Some(Tune::TokyoMidnight));
        assert_eq!(turns(midnight), 22);
        for track in tracks.iter().filter(|track| track.name != midnight.name) {
            assert!(turns(midnight) > turns(track), "not enough turns compared with {}", track.name);
        }
        assert!(midnight.segments.iter().map(|segment| segment.hill).sum::<f32>().abs() < 0.001);
        assert_eq!(midnight.height_at(0.0), midnight.height_at(midnight.length));
    }

    #[test]
    fn midnight_layout_closes_with_varied_corner_angles_and_radii() {
        let track = all_tracks().into_iter().find(|track| track.name == "Tokyo Midnight").unwrap();
        let step = track.length / 16_384.0;
        let mut heading = 0.0f32;
        let mut horizontal = 0.0f32;
        let mut vertical = 0.0f32;
        for index in 0..16_384 {
            let turn = track.curve_at((index as f32 + 0.5) * step) * step;
            horizontal += (heading + turn * 0.5).sin() * step;
            vertical -= (heading + turn * 0.5).cos() * step;
            heading += turn;
        }
        assert!((heading - 2.0 * PI).abs() < 0.001, "finish heading does not match the start: {heading}");
        assert!(horizontal.hypot(vertical) < 0.5, "track endpoints are separated by {} metres", horizontal.hypot(vertical));
        let turns: Vec<_> = track.segments.iter().filter(|segment| segment.curve != 0.0).collect();
        let angle = |segment: &Segment| segment.curve.abs() * (segment.len - (segment.len / 3.0).min(60.0));
        assert!(turns.iter().any(|segment| angle(segment) < PI / 6.0));
        assert!(turns.iter().filter(|segment| angle(segment) > PI * 0.49).count() >= 10);
        assert!(turns.iter().any(|segment| segment.curve.abs() <= 0.006));
        assert!(turns.iter().any(|segment| segment.curve.abs() >= 0.014));
        assert!(turns.windows(2).filter(|pair| pair[0].curve * pair[1].curve < 0.0).count() >= 4);
    }

    #[test]
    fn midnight_scenery_stays_outside_the_racing_surface() {
        let track = all_tracks().pop().unwrap();
        assert!(track.scenery.iter().filter(|object| matches!(object.kind, Prop::CityBuilding { .. })).count() > 200);
        assert!(track.scenery.iter().any(|object| object.kind == Prop::Torii));
        assert!(track.scenery.iter().any(|object| object.kind == Prop::Lantern));
        for object in &track.scenery {
            assert!((0.0..track.length).contains(&object.d));
            for &(offset, half_width) in object.kind.solid() {
                assert!((object.x + offset).abs() - half_width > ROAD_HALF);
            }
        }
    }
}
