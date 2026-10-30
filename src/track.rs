//! Track layouts (curves and hills), colour palettes and roadside scenery.

use crate::gfx::Rgb;
use crate::music::Tune;
use crate::sprites::{self, Bitmap, BILLBOARD_TEXT, NEON_TEXT};
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
    /// Retro striped sun, neon wireframe mountains and a Tokyo skyline.
    Synth,
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
        }
    }

    /// Solid parts you can crash into: (lateral offset, half-width) in metres.
    pub fn solid(self) -> &'static [(f32, f32)] {
        match self {
            Prop::Tree | Prop::Pine | Prop::Sakura => &[(0.0, 0.7)],
            Prop::Palm => &[(-0.9, 0.5)],
            Prop::Rock => &[(0.0, 1.4)],
            Prop::Chevron { .. } | Prop::NeonChevron { .. } => &[(0.0, 0.5)],
            Prop::Tyres => &[(0.0, 1.2)],
            Prop::Billboard { .. } | Prop::NeonBoard { .. } => &[(0.0, 4.0)],
            Prop::Lamp => &[(0.0, 0.3)],
            Prop::NeonSign { .. } => &[(0.0, 0.4)],
            // Pillars either side of the road.
            Prop::Torii => &[(-7.9, 0.7), (7.9, 0.7)],
            Prop::Gantry => &[(-8.7, 0.4), (8.7, 0.4)],
            Prop::Bush | Prop::Grandstand => &[],
        }
    }

    pub fn flipped(self) -> bool {
        matches!(self, Prop::Chevron { left: true } | Prop::NeonChevron { left: true })
    }

    pub fn body(self) -> Rgb {
        match self {
            Prop::Billboard { color, .. } | Prop::NeonSign { color, .. } | Prop::NeonBoard { color, .. } => color,
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
    pub scenery: Vec<Scenery>,
    /// Soundtrack exclusive to this track, if any.
    pub music: Option<Tune>,
}

#[derive(Clone, Copy)]
enum Style {
    Parkland,
    Mountain,
    Night,
    Synth,
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
        let music = matches!(style, Style::Synth).then_some(Tune::NeonSakura);
        let mut t = Track { name, desc, palette, segments, starts, base_heights, length: acc, scenery: Vec::new(), music };
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
        let mut rng = StdRng::seed_from_u64(seed);
        let ads = [Rgb(200, 20, 30), Rgb(20, 70, 200), Rgb(230, 130, 0), Rgb(20, 140, 60), Rgb(120, 30, 160)];
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

        let mut d = 120.0;
        let mut flip = false;
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
                        Style::Night | Style::Synth => Prop::Bush,
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
    ]
}
