//! Procedural scenery bitmaps and team-coloured car fallbacks for distant
//! traffic. Nearby cars and sponsor boards use the scene's mesh renderer.

use crate::gfx::{glyph, Rgb};
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum Px {
    Clear,
    Rgb(Rgb),
    /// Team colour with a brightness factor (> 1 blends towards white).
    Body(f32),
}

impl Px {
    #[inline]
    pub fn resolve(self, body: Rgb) -> Option<Rgb> {
        match self {
            Px::Clear => None,
            Px::Rgb(c) => Some(c),
            Px::Body(f) if f <= 1.0 => Some(body.scale(f)),
            Px::Body(f) => Some(body.lerp(Rgb(255, 255, 255), f - 1.0)),
        }
    }
}

pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    px: Vec<Px>,
}

impl Bitmap {
    fn new(w: usize, h: usize) -> Bitmap {
        Bitmap { w, h, px: vec![Px::Clear; w * h] }
    }

    fn from_art(rows: &[&str]) -> Bitmap {
        let w = rows[0].len();
        assert!(rows.iter().all(|r| r.len() == w), "ragged sprite art");
        let mut b = Bitmap::new(w, rows.len());
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.bytes().enumerate() {
                b.px[y * w + x] = art_px(ch);
            }
        }
        b
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Px {
        self.px[y * self.w + x]
    }

    fn set(&mut self, x: i32, y: i32, p: Px) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = p;
        }
    }

    fn rgb(&mut self, x: i32, y: i32, c: Rgb) {
        self.set(x, y, Px::Rgb(c));
    }

    fn is_set(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && !matches!(self.px[y as usize * self.w + x as usize], Px::Clear)
    }

    fn text(&mut self, s: &str, x: i32, y: i32, scale: i32, c: Rgb, shadow: Rgb) {
        for (pass, col) in [(1, shadow), (0, c)] {
            for (i, ch) in s.chars().enumerate() {
                let Some(g) = glyph(ch) else { continue };
                for (row, line) in g.iter().enumerate() {
                    for (cx, b) in line.bytes().enumerate() {
                        if b != b'#' {
                            continue;
                        }
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = x + (i as i32 * 6 + cx as i32) * scale + sx + pass;
                                let py = y + row as i32 * scale + sy + pass;
                                self.rgb(px, py, col);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn art_px(ch: u8) -> Px {
    let c = |r, g, b| Px::Rgb(Rgb(r, g, b));
    match ch {
        b'.' => Px::Clear,
        b'H' => Px::Body(1.35),
        b'B' => Px::Body(1.0),
        b'b' => Px::Body(0.68),
        b'd' => Px::Body(0.42),
        b'K' => c(30, 30, 36),
        b'k' => c(70, 70, 80),
        b'q' => c(14, 14, 18),
        b'W' => c(245, 245, 245),
        b'w' => c(185, 185, 195),
        b'G' => c(130, 130, 142),
        b'g' => c(78, 78, 90),
        b'D' => c(18, 18, 22),
        b'C' => c(44, 44, 52),
        b'c' => c(84, 84, 96),
        b'Y' => c(255, 214, 40),
        b'y' => c(205, 150, 0),
        b'V' => c(30, 34, 70),
        b'R' => c(255, 50, 40),
        b'r' => c(150, 20, 20),
        _ => c(255, 0, 255),
    }
}

// ---------------------------------------------------------------------------
// Cars
// ---------------------------------------------------------------------------

const CAR_REAR: &[&str] = &[
    "..DDDDDDDDDDDDDDDDDDDDDDDDDDDD..",
    "..DHHHHHHHHHHHHHHHHHHHHHHHHHHD..",
    "..DBBWWWWBBBBBBBBBBBBBBWWWWBBD..",
    "..DbbbbbbbbbbbbbbbbbbbbbbbbbbD..",
    "..DddddddddddddddddddddddddddD..",
    "..D........gCCyYYyCCg........D..",
    "qKKKKKk....gCCVVVVCCg....kKKKKKq",
    "qKkKKKk.....bBCYYCBb.....kKKKkKq",
    "qKKKKKk....bBHHBBHHBb....kKKKKKq",
    "qKkKKKk...bBBHBWWBHBBb...kKKKkKq",
    "qKKKKKkggbBBBBBWWBBBBBbggkKKKKKq",
    "qKkKKKk..bBBBBBBBBBBBBb..kKKKkKq",
    "qKKKKKk..dbBBBBBBBBBBbd..kKKKKKq",
    "qKkKKKk...dbbBBBBBBbbd...kKKKkKq",
    "qKKKKKk...ddCCCRRCCCdd...kKKKKKq",
    "qKkKKKk...GCCCCrrCCCCG...kKKKkKq",
    "qKKKKKk..CCcCCcCCcCCcCC..kKKKKKq",
    "qKkKKKk..DCDCDCDDCDCDCD..kKKKkKq",
    ".qKKKKq..................qKKKKq.",
    "..qqqq....................qqqq..",
];

const CAR_FRONT: &[&str] = &[
    "................................",
    "..............gCCg..............",
    ".............gCYYCg.............",
    "............gCyYYyCg............",
    "qKKKKk......gCVVVVCg......kKKKKq",
    "qKkKKk....gbBBBBBBBBbg....kKKkKq",
    "qKKKKkgg.bBHBBBBBBBBHBb.ggkKKKKq",
    "qKkKKk...bBBBBBWWBBBBBb...kKKkKq",
    "qKKKKk....dbBBBWWBBBbd....kKKKKq",
    "qKkKKk.....dbBBWWBBbd.....kKKkKq",
    "qKKKKk......dBBWWBBd......kKKKKq",
    ".qKKq.......dBBBBBBd.......qKKq.",
    "............dbBBBBbd............",
    ".............dBWWBd.............",
    "..DDDDDDDDDDDdBBBBdDDDDDDDDDDD..",
    "..DHHHHHHHHHHHHHHHHHHHHHHHHHHD..",
    "..DBBBBBBBBBBBBBBBBBBBBBBBBBBD..",
    "..DbbbbbbbbbbbbbbbbbbbbbbbbbbD..",
    "..DDDDDDDDDDDDDDDDDDDDDDDDDDDD..",
    "................................",
];

const CAR_SIDE: &[&str] = &[
    "DDDDD...........................................",
    "DHHHD..............gCCg.........................",
    "DBWBD.............gCCCCg........................",
    "DbbbD...........gBBBBBBBg....gYYg...............",
    ".gDg..........gBBBBBBBBBBBgCyYYVg...............",
    "..g.......gBBBBBBBBWWWWBBBBBBBBBBBBBg...........",
    "...gbBBBBBBBBBBBBBBWWWWBBBBBBBBBBBBBBBBBBBBBBg..",
    "...bBBqKKKKKqBBBBBBBBBBBBBBBBBBBqKKKKKqBBBBBBbg.",
    "...dbqKKkkkKKqbbbbbbbbbbbbbbbbbqKKkkkKKqbbbbbbbb",
    "....qKkKKKKKkKqddddddddddddddddqKkKKKKKkKqDDDDDD",
    "....qKkKggKKkKq................qKkKggKKkKq.HHHHH",
    "....qKkKggKKkKq................qKkKggKKkKq.BBBBB",
    ".....qKKkkkKKq..................qKKkkkKKq.......",
    "......qqqqqqq....................qqqqqqq........",
];

// ---------------------------------------------------------------------------
// Procedural scenery
// ---------------------------------------------------------------------------

const BAYER: [f32; 16] = [0., 8., 2., 10., 12., 4., 14., 6., 3., 11., 1., 9., 15., 7., 13., 5.];

fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

fn noise(x: i32, y: i32, seed: u32) -> f32 {
    (hash(x, y, seed) % 1000) as f32 / 1000.0
}

/// Pick a palette tone for brightness `l` with ordered dithering.
fn tone(pal: &[Rgb], l: f32, x: i32, y: i32) -> Rgb {
    let b = BAYER[((y & 3) * 4 + (x & 3)) as usize] / 16.0 - 0.5;
    let i = (l.clamp(0.0, 1.0) * (pal.len() - 1) as f32 + b * 0.9).round();
    pal[i.clamp(0.0, (pal.len() - 1) as f32) as usize]
}

/// Shaded "blob" made of overlapping spheres, lit from the upper left.
fn blobs(b: &mut Bitmap, spheres: &[(f32, f32, f32)], pal: &[Rgb], seed: u32, rough: f32) {
    for y in 0..b.h as i32 {
        for x in 0..b.w as i32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut best: Option<f32> = None;
            for &(cx, cy, r) in spheres {
                let r = r * (1.0 - rough * noise(x, y, seed));
                let (nx, ny) = ((fx - cx) / r, (fy - cy) / r);
                let d2 = nx * nx + ny * ny;
                if d2 < 1.0 {
                    let nz = (1.0 - d2).sqrt();
                    let l = (-0.45 * nx - 0.55 * ny + 0.7 * nz).max(0.0);
                    best = Some(best.map_or(l, |o: f32| o.max(l)));
                }
            }
            if let Some(l) = best {
                let l = l * 0.85 + noise(x, y, seed + 1) * 0.3 - 0.1;
                b.rgb(x, y, tone(pal, l, x, y));
            }
        }
    }
}

fn trunk(b: &mut Bitmap, x0: i32, x1: i32, y0: i32, y1: i32) {
    let bark = [Rgb(60, 36, 18), Rgb(96, 62, 30), Rgb(132, 90, 48)];
    for y in y0..y1 {
        for x in x0..x1 {
            let l = 1.0 - (x - x0) as f32 / (x1 - x0) as f32;
            b.rgb(x, y, tone(&bark, l * 0.9 + noise(x, y, 9) * 0.2, x, y));
        }
    }
}

fn make_tree() -> Bitmap {
    let mut b = Bitmap::new(48, 64);
    trunk(&mut b, 20, 28, 40, 64);
    let leaves = [Rgb(16, 58, 24), Rgb(28, 90, 34), Rgb(42, 124, 44), Rgb(74, 160, 58), Rgb(136, 198, 86)];
    let spheres = [
        (24.0, 24.0, 17.0),
        (12.0, 30.0, 11.0),
        (36.0, 30.0, 11.0),
        (24.0, 10.0, 10.0),
        (15.0, 16.0, 9.0),
        (33.0, 16.0, 9.0),
        (24.0, 38.0, 11.0),
    ];
    blobs(&mut b, &spheres, &leaves, 3, 0.12);
    b
}

fn make_pine() -> Bitmap {
    let (w, h) = (36, 76);
    let mut b = Bitmap::new(w, h);
    trunk(&mut b, 15, 21, 62, h as i32);
    let pal = [Rgb(10, 40, 26), Rgb(18, 64, 38), Rgb(30, 92, 50), Rgb(56, 124, 64), Rgb(110, 160, 96)];
    let cx = w as f32 / 2.0;
    for tier in 0..6 {
        let top = 2.0 + tier as f32 * 10.0;
        let bottom = top + 18.0;
        let half = 5.0 + tier as f32 * 2.6;
        for y in top as i32..bottom as i32 {
            let t = (y as f32 - top) / (bottom - top);
            let hw = half * t + 0.5;
            for x in 0..w as i32 {
                let dx = x as f32 + 0.5 - cx;
                let jag = noise(x, y, 4) * 2.0 * t;
                if dx.abs() < hw - jag {
                    let l = 0.65 - dx / hw * 0.4 - t * 0.35 + noise(x, y, 5) * 0.2;
                    b.rgb(x, y, tone(&pal, l, x, y));
                }
            }
        }
    }
    b
}

fn make_bush() -> Bitmap {
    let mut b = Bitmap::new(32, 16);
    let pal = [Rgb(20, 66, 26), Rgb(36, 102, 36), Rgb(58, 140, 48), Rgb(108, 180, 70)];
    blobs(&mut b, &[(9.0, 10.0, 7.5), (17.0, 7.5, 8.0), (25.0, 10.5, 7.0), (16.0, 12.0, 8.0)], &pal, 6, 0.1);
    // A few flowers.
    for i in 0..9 {
        let (x, y) = ((hash(i, 0, 7) % 28 + 2) as i32, (hash(i, 1, 7) % 8 + 4) as i32);
        if b.is_set(x, y) {
            b.rgb(x, y, if i % 2 == 0 { Rgb(250, 230, 90) } else { Rgb(240, 120, 160) });
        }
    }
    b
}

fn make_rock() -> Bitmap {
    let mut b = Bitmap::new(32, 20);
    let pal = [Rgb(54, 50, 56), Rgb(88, 84, 90), Rgb(124, 120, 124), Rgb(168, 164, 160), Rgb(206, 202, 196)];
    blobs(&mut b, &[(12.0, 13.0, 9.0), (21.0, 12.0, 10.0), (16.0, 8.0, 7.0)], &pal, 8, 0.05);
    for y in 17..20 {
        for x in 0..32 {
            if !b.is_set(x, y) && b.is_set(x, 16) {
                b.rgb(x, y, Rgb(54, 50, 56));
            }
        }
    }
    b
}

fn make_chevron() -> Bitmap {
    let (w, h) = (26, 30);
    let mut b = Bitmap::new(w, h);
    for y in 22..h as i32 {
        for x in 11..15 {
            b.rgb(x, y, if x == 11 { Rgb(170, 170, 180) } else { Rgb(100, 100, 112) });
        }
    }
    for y in 0..22 {
        for x in 0..w as i32 {
            let border = x == 0 || y == 0 || x == w as i32 - 1 || y == 21;
            let c = if border {
                Rgb(20, 20, 20)
            } else {
                // Two thick ">" arrows.
                let dy = (y as f32 - 10.5).abs();
                let on = [4.0, 13.0].iter().any(|&ax| {
                    let d = x as f32 - (ax + (10.0 - dy) * 0.55);
                    (0.0..4.0).contains(&d)
                });
                if on {
                    Rgb(20, 20, 20)
                } else if y < 3 {
                    Rgb(255, 228, 90)
                } else {
                    Rgb(250, 196, 0)
                }
            };
            b.rgb(x, y, c);
        }
    }
    b
}

fn make_tyres() -> Bitmap {
    let (w, h) = (36, 18);
    let mut b = Bitmap::new(w, h);
    let rubber = [Rgb(14, 14, 18), Rgb(36, 36, 42), Rgb(70, 70, 80)];
    for row in 0..2 {
        for col in 0..4 {
            let (cx, cy) = (4.5 + col as f32 * 9.0 + row as f32 * 0.0, 4.5 + row as f32 * 9.0);
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let (dx, dy) = ((x as f32 + 0.5 - cx) / 4.5, (y as f32 + 0.5 - cy) / 4.5);
                    let d = dx * dx + dy * dy;
                    if d < 1.0 {
                        let band = (0.35..0.6).contains(&d) && row == 0;
                        let c = if band {
                            if col % 2 == 0 { Rgb(240, 240, 240) } else { Rgb(220, 30, 30) }
                        } else {
                            tone(&rubber, 0.6 - dy * 0.5, x, y)
                        };
                        b.rgb(x, y, c);
                    }
                }
            }
        }
    }
    b
}

fn make_billboard(text: &str) -> Bitmap {
    let (w, h) = (64, 40);
    let mut b = Bitmap::new(w, h);
    for y in 26..h as i32 {
        for x in [8, 9, 10, 53, 54, 55] {
            b.rgb(x, y, if x % 2 == 0 { Rgb(150, 150, 160) } else { Rgb(90, 90, 100) });
        }
    }
    for y in 0..27 {
        for x in 0..w as i32 {
            let frame = x < 2 || y < 2 || x >= w as i32 - 2 || y >= 25;
            if frame {
                b.rgb(x, y, if y < 2 { Rgb(230, 230, 236) } else { Rgb(150, 150, 160) });
            } else {
                let f = 1.25 - y as f32 / 25.0 * 0.5 + (BAYER[((y & 3) * 4 + (x & 3)) as usize] / 16.0 - 0.5) * 0.12;
                b.set(x, y, Px::Body(f));
            }
        }
    }
    let n = text.chars().count() as i32;
    let scale = if n * 12 - 2 <= 56 { 2 } else { 1 };
    let tw = (n * 6 - 1) * scale;
    let th = 7 * scale;
    b.text(text, (w as i32 - tw) / 2, (25 - th) / 2 + 1, scale, Rgb(255, 255, 255), Rgb(20, 20, 30));
    b
}

fn make_lamp() -> Bitmap {
    let (w, h) = (16, 80);
    let mut b = Bitmap::new(w, h);
    for y in 6..h as i32 {
        for x in 7..10 {
            b.rgb(x, y, [Rgb(190, 190, 200), Rgb(130, 130, 142), Rgb(80, 80, 92)][(x - 7) as usize]);
        }
    }
    for x in 2..14 {
        b.rgb(x, 4, Rgb(120, 120, 132));
        b.rgb(x, 5, Rgb(80, 80, 92));
    }
    for x in 1..15 {
        let glow = if (4..12).contains(&x) { Rgb(255, 250, 210) } else { Rgb(255, 214, 90) };
        b.rgb(x, 6, glow);
        b.rgb(x, 7, Rgb(255, 200, 60));
    }
    for x in 4..12 {
        b.rgb(x, 3, Rgb(60, 60, 70));
    }
    for y in h as i32 - 3..h as i32 {
        for x in 5..12 {
            b.rgb(x, y, Rgb(90, 90, 100));
        }
    }
    b
}

fn make_gantry() -> Bitmap {
    let (w, h) = (128, 52);
    let mut b = Bitmap::new(w, h);
    // Truss pillars.
    for x0 in [0, w as i32 - 8] {
        for y in 14..h as i32 {
            for x in x0..x0 + 8 {
                let lx = x - x0;
                let diag = (lx - (y % 8)).abs() < 1 || (lx - (7 - y % 8)).abs() < 1;
                if lx == 0 || lx == 7 || diag {
                    b.rgb(x, y, if lx < 4 { Rgb(200, 200, 210) } else { Rgb(120, 120, 132) });
                }
            }
        }
    }
    // Banner with checkered borders.
    for y in 0..16 {
        for x in 0..w as i32 {
            let c = if !(3..13).contains(&y) {
                if ((x / 3) + (y / 3)) % 2 == 0 { Rgb(245, 245, 245) } else { Rgb(16, 16, 16) }
            } else {
                Rgb(20, 40, 120).lerp(Rgb(10, 20, 70), (y - 3) as f32 / 10.0)
            };
            b.rgb(x, y, c);
        }
    }
    let label = "SLOPRACER GP";
    b.text(label, (w as i32 - crate::gfx::text_width(label, 1)) / 2, 5, 1, Rgb(255, 214, 40), Rgb(0, 0, 20));
    b
}

fn make_grandstand() -> Bitmap {
    let (w, h) = (128, 50);
    let mut b = Bitmap::new(w, h);
    let crowd = [
        Rgb(230, 60, 50), Rgb(250, 220, 60), Rgb(60, 110, 230), Rgb(240, 240, 240),
        Rgb(60, 180, 90), Rgb(240, 190, 150), Rgb(200, 150, 110), Rgb(30, 30, 40),
    ];
    // Roof.
    for y in 0..6 {
        for x in 0..w as i32 {
            let stripe = ((x + y) / 6) % 2 == 0;
            b.rgb(x, y, if y == 5 { Rgb(60, 60, 70) } else if stripe { Rgb(235, 235, 240) } else { Rgb(210, 30, 40) });
        }
    }
    for x in (4..w as i32).step_by(30) {
        for y in 6..12 {
            b.rgb(x, y, Rgb(120, 120, 130));
            b.rgb(x + 1, y, Rgb(80, 80, 90));
        }
    }
    // Stepped tiers full of spectators.
    for y in 10..44 {
        let tier = (y - 10) / 4;
        for x in 0..w as i32 {
            let c = if (y - 10) % 4 == 3 {
                Rgb(150, 150, 160)
            } else if hash(x, y, 11).is_multiple_of(5) {
                Rgb(110, 110, 120).scale(0.8 + tier as f32 * 0.03)
            } else {
                crowd[(hash(x / 2, y + tier, 12) % crowd.len() as u32) as usize]
            };
            b.rgb(x, y, c);
        }
    }
    // Front wall with sponsor panels.
    for y in 44..h as i32 {
        for x in 0..w as i32 {
            let panel = (x / 32) % 2 == 0;
            b.rgb(x, y, if panel { Rgb(20, 70, 200) } else { Rgb(240, 240, 240) });
        }
    }
    b
}

// ---------------------------------------------------------------------------
// Synthwave scenery
// ---------------------------------------------------------------------------

/// Katakana for neon signs, 10x10. `vertical` picks the rotated long-vowel
/// mark used in top-to-bottom writing.
fn kana(ch: char, vertical: bool) -> Option<[&'static str; 10]> {
    Some(match ch {
        'ロ' => [
            "#########.", "#.......#.", "#.......#.", "#.......#.", "#.......#.",
            "#.......#.", "#.......#.", "#.......#.", "#########.", "..........",
        ],
        'ッ' => [
            "..........", "..........", "..........", "..........", "..#..#..#.",
            "...#..#.#.", "........#.", ".......#..", "......#...", "....##....",
        ],
        'プ' => [
            ".......##.", "......#..#", ".######.#.", "......###.", "......#...",
            "......#...", ".....#....", "....#.....", "..##......", "##........",
        ],
        'レ' => [
            ".#........", ".#........", ".#........", ".#.......#", ".#......#.",
            ".#.....#..", ".#....#...", ".#...#....", ".#.##.....", ".##.......",
        ],
        'サ' => [
            "..#....#..", "..#....#..", "##########", "..#....#..", "..#....#..",
            "..#....#..", ".......#..", "......#...", ".....#....", "...##.....",
        ],
        'ス' => [
            ".#######..", ".......#..", "......#...", ".....#....", "....##....",
            "...#..#...", "..#....#..", ".#......#.", "#........#", "..........",
        ],
        'ネ' => [
            "....#.....", ".....#....", ".#######..", "......#...", ".....#....",
            "...###....", "..#.#.#...", ".#..#..#..", "#...#...#.", "....#.....",
        ],
        'オ' => [
            "......#...", "......#...", "##########", ".....##...", "....#.#...",
            "...#..#...", "..#...#...", ".#....#...", "#.....#...", "....###...",
        ],
        'ン' => [
            "##........", "..#.......", "...#.....#", ".........#", "........#.",
            ".......#..", "......#...", "....##....", "..##......", "##........",
        ],
        'タ' => [
            "...#......", "..#######.", ".#......#.", "#.##....#.", "....##.#..",
            "......##..", "......#...", ".....#....", "...##.....", ".##.......",
        ],
        'ボ' => [
            "....#.#.#.", "....#..#.#", "#########.", "....#.....", "....#.....",
            "..#.#.#...", ".#..#..#..", "#...#...#.", "....#.....", "..###.....",
        ],
        'ク' => [
            "...#......", "..########", ".#......#.", "#.......#.", ".......#..",
            "......#...", ".....#....", "....#.....", "..##......", "##........",
        ],
        'ラ' => [
            ".#######..", "..........", "##########", ".........#", "........#.",
            ".......#..", "......#...", "....##....", "..##......", "##........",
        ],
        'ー' if vertical => [
            "..........", "....##....", "....##....", "....##....", "....##....",
            "....##....", "....##....", "....##....", "....##....", "..........",
        ],
        'ー' => [
            "..........", "..........", "..........", "..........", "##########",
            "##########", "..........", "..........", "..........", "..........",
        ],
        _ => return None,
    })
}

/// Neon sign texts: katakana and their meaning in Latin letters.
pub const NEON_TEXT: [(&str, &str); 4] = [("スロップレーサー", "SLOPRACER"), ("ネオン", "NEON"), ("ターボ", "TURBO"), ("サクラ", "SAKURA")];

/// Paint a glowing tube: white-hot core, coloured edge and a soft halo.
fn neon_strokes(b: &mut Bitmap, on: &dyn Fn(i32, i32) -> bool, x0: i32, y0: i32, w: i32, h: i32) {
    for y in y0 - 1..y0 + h + 1 {
        for x in x0 - 1..x0 + w + 1 {
            if on(x, y) {
                let edge = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| !on(x + dx, y + dy));
                b.set(x, y, Px::Body(if edge { 1.15 } else { 1.75 }));
            } else if (-1..=1).any(|dy| (-1..=1).any(|dx| on(x + dx, y + dy))) {
                b.set(x, y, Px::Body(0.45));
            }
        }
    }
}

fn kana_line(b: &mut Bitmap, text: &str, x0: i32, y0: i32, vertical: bool) {
    let glyphs: Vec<[&str; 10]> = text.chars().filter_map(|c| kana(c, vertical)).collect();
    let on = |x: i32, y: i32| -> bool {
        let (along, across) = if vertical { (y - y0, x - x0) } else { (x - x0, y - y0) };
        if along < 0 || !(0..10).contains(&across) {
            return false;
        }
        let (i, k) = ((along / 12) as usize, along % 12);
        if i >= glyphs.len() || k >= 10 {
            return false;
        }
        let (gx, gy) = if vertical { (across, k) } else { (k, across) };
        glyphs[i][gy as usize].as_bytes()[gx as usize] == b'#'
    };
    let n = glyphs.len() as i32;
    let (w, h) = if vertical { (10, n * 12) } else { (n * 12, 10) };
    neon_strokes(b, &on, x0, y0, w, h);
}

fn neon_panel(b: &mut Bitmap, x0: i32, y0: i32, w: i32, h: i32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let border = x == x0 || y == y0 || x == x0 + w - 1 || y == y0 + h - 1;
            let inner = x == x0 + 1 || y == y0 + 1 || x == x0 + w - 2 || y == y0 + h - 2;
            if border {
                b.set(x, y, Px::Body(0.55));
            } else if inner {
                b.set(x, y, Px::Body(1.5));
            } else {
                b.rgb(x, y, Rgb(22, 10, 34).lerp(Rgb(40, 14, 52), (y - y0) as f32 / h as f32));
            }
        }
    }
}

fn pole(b: &mut Bitmap, x0: i32, y0: i32, y1: i32) {
    for y in y0..y1 {
        b.rgb(x0, y, Rgb(110, 70, 120));
        b.rgb(x0 + 1, y, Rgb(54, 30, 66));
    }
}

/// Tall vertical sign with katakana written top to bottom.
fn make_neon_sign(text: &str) -> Bitmap {
    let n = text.chars().count() as i32;
    let ph = n * 12 + 6;
    let (w, h) = (22, (ph + 22).max(76) as usize);
    let mut b = Bitmap::new(w, h);
    pole(&mut b, 10, ph, h as i32);
    neon_panel(&mut b, 0, 0, w as i32, ph);
    kana_line(&mut b, text, 6, 4, true);
    b
}

/// Wide sign: katakana on top, the Latin reading below.
fn make_neon_board(kana_text: &str, latin: &str) -> Bitmap {
    let w = 66.max(kana_text.chars().count() * 12 + 12).max(latin.chars().count() * 6 + 12);
    let h = 44;
    let mut b = Bitmap::new(w, h);
    for x in [w as i32 / 2 - 20, w as i32 / 2 + 20] {
        pole(&mut b, x, 30, h as i32);
    }
    neon_panel(&mut b, 0, 0, w as i32, 31);
    let kw = kana_text.chars().count() as i32 * 12 - 2;
    kana_line(&mut b, kana_text, (w as i32 - kw) / 2, 4, false);
    let lw = latin.chars().count() as i32 * 6 - 1;
    let lx = (w as i32 - lw) / 2;
    let on = |x: i32, y: i32| -> bool {
        let (cx, ry) = (x - lx, y - 19);
        if cx < 0 || !(0..7).contains(&ry) || cx % 6 == 5 {
            return false;
        }
        let Some(ch) = latin.chars().nth((cx / 6) as usize) else { return false };
        glyph(ch).is_some_and(|g| g[ry as usize].as_bytes()[(cx % 6) as usize] == b'#')
    };
    neon_strokes(&mut b, &on, lx, 19, lw, 7);
    b
}

fn make_neon_chevron() -> Bitmap {
    let (w, h) = (26, 30);
    let mut b = Bitmap::new(w, h);
    pole(&mut b, 12, 22, h as i32);
    neon_panel(&mut b, 0, 0, w as i32, 22);
    let on = |x: i32, y: i32| -> bool {
        let dy = (y as f32 - 10.5).abs();
        (3..19).contains(&y) && [5.0, 13.0].iter().any(|&ax| (0.0..2.5).contains(&(x as f32 - (ax + (8.0 - dy) * 0.7))))
    };
    neon_strokes(&mut b, &on, 2, 2, 22, 18);
    b
}

impl Bitmap {
    /// Thick line with round-ish caps.
    fn stroke(&mut self, (x0, y0): (f32, f32), (x1, y1): (f32, f32), r: f32, c: Rgb) {
        let n = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0).ceil().max(1.0) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if (dx * dx + dy * dy) as f32 <= r * r + 0.3 {
                        self.rgb((x + dx as f32).round() as i32, (y + dy as f32).round() as i32, c);
                    }
                }
            }
        }
    }
}

/// Backlit palm: dark violet silhouette with a pink sunset rim light.
fn make_palm() -> Bitmap {
    let (w, h) = (64, 100);
    let mut b = Bitmap::new(w, h);
    let trunk = [Rgb(30, 12, 40), Rgb(52, 22, 60), Rgb(86, 34, 82), Rgb(170, 64, 120), Rgb(255, 140, 120)];
    let crown = (32.0f32, 28.0f32);
    for y in 28..h as i32 {
        let t = (99.0 - y as f32) / 71.0;
        let cx = 24.0 + 8.0 * t * t;
        let hw = 3.8 - 1.4 * t;
        for x in 0..w as i32 {
            let dx = (x as f32 + 0.5 - cx) / hw;
            if dx.abs() <= 1.0 {
                // Bark rings, and the rim light on the sun side.
                let ring = if (y + (dx * 2.0) as i32) % 4 == 0 { -0.22 } else { 0.0 };
                let l = 0.28 + dx.max(0.0).powi(3) * 0.8 + ring + noise(x, y, 21) * 0.12;
                b.rgb(x, y, tone(&trunk, l, x, y));
            }
        }
    }
    let leaf = [Rgb(16, 6, 30), Rgb(34, 14, 54), Rgb(64, 24, 82), Rgb(132, 42, 124), Rgb(255, 96, 170)];
    // (angle from +x in degrees, screen y down; length; droop).
    let fronds = [
        (-168.0, 30.0, 22.0), (-138.0, 26.0, 20.0), (-108.0, 18.0, 12.0), (-72.0, 18.0, 12.0),
        (-42.0, 26.0, 20.0), (-12.0, 30.0, 22.0), (150.0, 18.0, 14.0), (30.0, 18.0, 14.0),
    ];
    for (k, &(ang, len, droop)) in fronds.iter().enumerate() {
        let a = (ang as f32).to_radians();
        let (dx, dy) = (a.cos(), a.sin());
        let at = |s: f32| (crown.0 + dx * s * len, crown.1 + dy * s * len + droop * s * s);
        let steps = (len * 1.2) as i32;
        for i in 0..steps {
            let s = i as f32 / steps as f32;
            let (x, y) = at(s);
            let (x2, y2) = at(s + 1.0 / steps as f32);
            let (tx, ty) = (x2 - x, y2 - y);
            let tl = (tx * tx + ty * ty).sqrt().max(1e-3);
            let (ux, uy) = (tx / tl, ty / tl);
            // Leaflets sweep back and hang down on both sides of the rib.
            let ll = 4.8 * (std::f32::consts::PI * (0.12 + 0.88 * s)).sin().powf(0.7);
            for side in [-1.0f32, 1.0] {
                let (nx, ny) = (-uy * side, ux * side);
                let (ex, ey) = (x + (nx * 0.7 + ux * 0.5) * ll, y + (ny * 0.7 + uy * 0.5).abs() * ll + ll * 0.35);
                let upper = ny < 0.0;
                let lit = 0.2 + if upper { 0.25 } else { 0.0 } + (1.0 - s) * 0.2 + noise(i, k as i32 + side as i32, 22) * 0.12;
                b.stroke((x, y), (ex, ey), 0.35, tone(&leaf, lit, ex as i32, ey as i32));
            }
            b.stroke((x, y), (x2, y2), 0.6, tone(&leaf, 0.8 - s * 0.35, x as i32, y as i32));
        }
    }
    // Coconuts.
    for (cx, cy) in [(30.0, 31.0), (34.0, 32.0), (32.0, 33.5)] {
        b.stroke((cx, cy), (cx, cy), 1.4, Rgb(60, 26, 50));
        b.rgb(cx as i32, cy as i32 - 1, Rgb(150, 60, 110));
    }
    b
}

fn make_sakura() -> Bitmap {
    let mut b = Bitmap::new(52, 56);
    trunk(&mut b, 23, 29, 34, 56);
    // A few dark branches showing between blossoms.
    for (x0, y0, x1, y1) in [(26.0, 36.0, 12.0, 22.0), (26.0, 36.0, 40.0, 20.0), (26.0, 34.0, 26.0, 14.0)] {
        b.stroke((x0, y0), (x1, y1), 1.2, Rgb(66, 34, 40));
    }
    let blossom = [Rgb(118, 38, 88), Rgb(190, 72, 132), Rgb(236, 124, 172), Rgb(255, 172, 206), Rgb(255, 224, 236)];
    let clusters = [
        (26.0, 20.0, 13.0), (12.0, 24.0, 10.0), (40.0, 23.0, 10.0), (19.0, 11.0, 9.0),
        (34.0, 10.0, 9.0), (5.0, 30.0, 5.5), (47.0, 30.0, 5.5), (26.0, 30.0, 8.0),
    ];
    blobs(&mut b, &clusters, &blossom, 23, 0.22);
    // Petals drifting below.
    for i in 0..14 {
        let (x, y) = ((hash(i, 0, 24) % 50 + 1) as i32, (hash(i, 1, 24) % 18 + 36) as i32);
        if !b.is_set(x, y) {
            b.rgb(x, y, if i % 3 == 0 { Rgb(255, 224, 236) } else { Rgb(236, 124, 172) });
        }
    }
    b
}

/// Vermilion torii gate spanning the road, optionally under fresh snow.
fn make_torii(snow: bool) -> Bitmap {
    let (w, h) = (128, 84);
    let mut b = Bitmap::new(w, h);
    let red = [Rgb(110, 16, 22), Rgb(176, 32, 28), Rgb(228, 64, 36), Rgb(255, 124, 70)];
    let black = [Rgb(12, 8, 16), Rgb(34, 24, 40), Rgb(70, 52, 74)];
    let wi = w as i32;
    // Pillars, slightly wider at the foot, with black bases.
    for (pc, _) in [(15.0f32, 0), (113.0, 1)] {
        for y in 18..h as i32 {
            let hw = 4.0 + (y as f32 - 18.0) / 66.0 * 1.2;
            for x in 0..wi {
                let dx = (x as f32 + 0.5 - pc) / hw;
                if dx.abs() <= 1.0 {
                    let c = if y >= 76 {
                        tone(&black, 0.5 - dx * 0.3, x, y)
                    } else {
                        tone(&red, 0.45 + dx.abs().powi(4) * 0.5 * dx.signum() + noise(x, y, 31) * 0.1, x, y)
                    };
                    b.rgb(x, y, c);
                }
            }
        }
    }
    // Nuki (lower tie beam), passing through the pillars.
    for y in 34..40 {
        for x in 3..wi - 3 {
            b.rgb(x, y, tone(&red, if y == 34 { 0.95 } else { 0.55 - (y - 34) as f32 * 0.06 }, x, y));
        }
    }
    // Central strut with a dark plaque.
    for y in 20..34 {
        for x in 56..72 {
            let border = x == 56 || x == 71 || y == 20 || y == 33;
            b.rgb(x, y, if border { Rgb(230, 182, 70) } else { Rgb(26, 14, 30) });
        }
    }
    for x in 60..68 {
        for y in [24, 27, 30] {
            b.rgb(x, y, Rgb(230, 182, 70));
        }
    }
    // Shimaki (red) under the black kasagi, whose ends sweep upward.
    for x in 0..wi {
        let e = ((x as f32 + 0.5 - 64.0).abs() / 64.0).powi(3);
        let top = 7.0 - 6.0 * e;
        for y in 0..h as i32 {
            let yf = y as f32;
            if yf >= top && yf < top + 7.0 {
                b.rgb(x, y, tone(&black, if yf < top + 1.5 { 0.95 } else { 0.35 }, x, y));
            } else if (4..wi - 4).contains(&x) && yf >= top + 7.0 && yf < top + 12.0 {
                b.rgb(x, y, tone(&red, 0.6 - (yf - top - 7.0) * 0.07, x, y));
            }
        }
    }
    if snow {
        let wi = w as i32;
        for x in 0..wi {
            let e = ((x as f32 + 0.5 - 64.0).abs() / 64.0).powi(3);
            let top = (7.0 - 6.0 * e).floor() as i32;
            // A pillow of snow along the roof beam, thinning at the tips.
            let depth = (2.6 * (1.0 - e * 1.6) + noise(x, 0, 41) * 0.8).max(0.0) as i32;
            for k in 1..=depth {
                let y = top - k + 1;
                b.rgb(x, y.max(0), snow_tone(0.95 - k as f32 * 0.12 + (x as f32 / wi as f32) * 0.1, x, y));
            }
            if (4..wi - 4).contains(&x) && noise(x, 1, 42) < 0.85 {
                b.rgb(x, 33, snow_tone(0.85, x, 33));
            }
        }
        // Drifts around the pillar feet.
        for (pc, half) in [(15.0f32, 9.0f32), (113.0, 9.0)] {
            for x in 0..wi {
                let dx = (x as f32 + 0.5 - pc) / half;
                if dx.abs() < 1.0 {
                    let rise = ((1.0 - dx * dx) * 4.0) as i32;
                    for y in h as i32 - 1 - rise..h as i32 {
                        b.rgb(x, y, snow_tone(0.9 - dx * 0.25, x, y));
                    }
                }
            }
        }
    }
    b
}

// ---------------------------------------------------------------------------
// Winter scenery
// ---------------------------------------------------------------------------

const SNOW: [Rgb; 5] = [Rgb(120, 140, 190), Rgb(168, 186, 226), Rgb(208, 220, 244), Rgb(240, 244, 252), Rgb(255, 255, 255)];

fn snow_tone(l: f32, x: i32, y: i32) -> Rgb {
    tone(&SNOW, l, x, y)
}

/// Japanese cedar with snow resting on every tier of branches.
fn make_snow_cedar() -> Bitmap {
    let (w, h) = (40, 84);
    let mut b = Bitmap::new(w, h);
    trunk(&mut b, 18, 23, 70, h as i32);
    let pal = [Rgb(8, 28, 28), Rgb(14, 46, 40), Rgb(24, 68, 54), Rgb(44, 96, 74), Rgb(86, 132, 104)];
    let cx = w as f32 / 2.0;
    for tier in 0..7 {
        let top = 2.0 + tier as f32 * 9.6;
        let bottom = top + 16.0;
        let half = 4.0 + tier as f32 * 2.3;
        for y in top as i32..bottom as i32 {
            let t = (y as f32 - top) / (bottom - top);
            let hw = half * t + 0.5;
            for x in 0..w as i32 {
                let dx = x as f32 + 0.5 - cx;
                let jag = noise(x, y, 43) * 1.6 * t;
                if dx.abs() >= hw - jag {
                    continue;
                }
                // Vertical distance below the tier's sloping upper edge.
                let below = (y as f32 - top) - (dx.abs() - 0.5).max(0.0) / half * (bottom - top);
                let c = if below < 2.2 + noise(x, y, 44) * 1.8 && t > 0.15 {
                    snow_tone(0.9 - dx / hw * 0.35 - (below / 4.0) * 0.3, x, y)
                } else {
                    tone(&pal, 0.6 - dx / hw * 0.35 - t * 0.35 + noise(x, y, 45) * 0.2, x, y)
                };
                b.rgb(x, y, c);
            }
        }
    }
    b
}

/// Bare white birch: a pale trunk with dark marks, twigs dusted with snow.
fn make_birch() -> Bitmap {
    let (w, h) = (40, 80);
    let mut b = Bitmap::new(w, h);
    let mut twigs: Vec<(i32, i32)> = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn grow(b: &mut Bitmap, twigs: &mut Vec<(i32, i32)>, at: (f32, f32), ang: f32, len: f32, r: f32, depth: u32, seed: i32) {
        let end = (at.0 + ang.cos() * len, at.1 + ang.sin() * len);
        let c = if depth == 0 { Rgb(150, 128, 120) } else { Rgb(84, 70, 72) };
        b.stroke(at, end, r, c);
        let n = (len * 1.5) as i32;
        for i in 0..=n {
            let t = i as f32 / n.max(1) as f32;
            twigs.push(((at.0 + (end.0 - at.0) * t).round() as i32, (at.1 + (end.1 - at.1) * t - r).round() as i32));
        }
        if depth > 0 {
            for (k, spread) in [-0.45f32, 0.4].into_iter().enumerate() {
                let jitter = (noise(seed, k as i32, 46) - 0.5) * 0.4;
                grow(b, twigs, end, ang + spread + jitter, len * 0.68, (r * 0.7).max(0.35), depth - 1, seed * 3 + k as i32 + 1);
            }
        }
    }
    let up = -std::f32::consts::FRAC_PI_2;
    for (k, &(y, side, len)) in [(46.0, -1.0, 13.0), (38.0, 1.0, 12.0), (30.0, -1.0, 11.0), (22.0, 1.0, 10.0), (15.0, -1.0, 8.0)].iter().enumerate() {
        let ang = up + side * (0.75 + noise(k as i32, 0, 47) * 0.3);
        grow(&mut b, &mut twigs, (20.0, y), ang, len, 0.8, 3, k as i32 + 1);
    }
    grow(&mut b, &mut twigs, (20.0, 12.0), up, 9.0, 0.8, 2, 9);
    // Snow along the upper side of the twigs.
    for (x, y) in twigs {
        if !b.is_set(x, y) && noise(x, y, 48) < 0.6 {
            b.rgb(x, y, snow_tone(0.85 + noise(x, y, 49) * 0.15, x, y));
        }
    }
    // Trunk last, tapering, with lenticel marks.
    for y in 10..h as i32 {
        let hw = 1.2 + (y as f32 - 10.0) / 70.0 * 1.6;
        for x in 0..w as i32 {
            let dx = (x as f32 + 0.5 - 20.0) / hw;
            if dx.abs() <= 1.0 {
                let mark = noise(y / 2, (x - 18) / 2, 50) < 0.16 && y % 2 == 0;
                let c = if mark {
                    Rgb(40, 36, 40)
                } else {
                    [Rgb(250, 248, 240), Rgb(222, 222, 226), Rgb(176, 182, 198)][((dx + 1.0) * 1.49) as usize]
                };
                b.rgb(x, y, c);
            }
        }
    }
    b
}

/// Stone lantern (toro) with a warm light and a cap of snow.
fn make_lantern() -> Bitmap {
    let (w, h) = (20, 36);
    let mut b = Bitmap::new(w, h);
    let stone = [Rgb(64, 66, 80), Rgb(104, 106, 118), Rgb(146, 146, 156), Rgb(188, 188, 194)];
    let slab = |b: &mut Bitmap, x0: i32, x1: i32, y0: i32, y1: i32| {
        for y in y0..y1 {
            for x in x0..x1 {
                let l = 0.85 - (x - x0) as f32 / (x1 - x0) as f32 * 0.6 + noise(x, y, 51) * 0.15;
                b.rgb(x, y, tone(&stone, l, x, y));
            }
        }
    };
    slab(&mut b, 3, 17, 32, 36);
    slab(&mut b, 8, 12, 21, 32);
    slab(&mut b, 4, 16, 18, 21);
    slab(&mut b, 5, 15, 11, 18);
    // Roof: wide at the eaves, with upturned corners.
    for y in 6..11 {
        let half = 3 + (y - 6) * 2;
        slab(&mut b, 10 - half, 10 + half, y, y + 1);
    }
    b.rgb(0, 9, stone[1]);
    b.rgb(19, 9, stone[0]);
    // Glowing paper window.
    for y in 12..17 {
        for x in 7..13 {
            let c = (x as f32 - 9.5).abs() + (y as f32 - 14.0).abs();
            b.rgb(x, y, Rgb(255, 244, 200).lerp(Rgb(255, 170, 70), (c / 4.5).min(1.0)));
        }
    }
    // Snow: a mound on the roof, a ball on the finial, dustings on ledges.
    for x in 1..19 {
        let dx = (x as f32 - 9.5) / 9.0;
        let top = 6.0 - (1.0 - dx * dx) * 3.0;
        for y in top as i32..7 {
            b.rgb(x, y, snow_tone(0.95 - dx * 0.3, x, y));
        }
    }
    for (x, y) in [(9, 1), (10, 1), (9, 2), (10, 2), (8, 2), (11, 2)] {
        b.rgb(x, y, snow_tone(if x < 10 { 1.0 } else { 0.75 }, x, y));
    }
    for x in 4..16 {
        b.rgb(x, 18, snow_tone(0.9, x, 18));
    }
    for x in 3..17 {
        b.rgb(x, 32, snow_tone(0.9, x, 32));
    }
    b
}

/// A glowing drinks machine, team-coloured per instance, snow on its roof.
fn make_vending() -> Bitmap {
    let (w, h) = (24, 40);
    let mut b = Bitmap::new(w, h);
    for y in 4..h as i32 - 1 {
        for x in 1..23 {
            let f = if x == 1 { 1.3 } else if x >= 21 { 0.7 } else { 1.0 };
            b.set(x, y, Px::Body(f));
        }
    }
    for x in 3..21 {
        for y in 5..9 {
            b.rgb(x, y, if y == 6 || y == 7 { Rgb(255, 252, 236) } else { Rgb(236, 230, 214) });
        }
    }
    // Lit window with three shelves of cans.
    let cans = [Rgb(230, 40, 40), Rgb(40, 110, 220), Rgb(250, 200, 40), Rgb(40, 170, 90), Rgb(245, 245, 245), Rgb(250, 130, 30)];
    for y in 10..25 {
        for x in 3..21 {
            b.rgb(x, y, Rgb(200, 214, 226));
        }
    }
    for (row, y0) in [11, 16, 21].into_iter().enumerate() {
        for k in 0..6 {
            let x0 = 3 + k * 3;
            let c = cans[(k as usize + row * 2) % cans.len()];
            for y in y0..y0 + 3 {
                b.rgb(x0, y, c);
                b.rgb(x0 + 1, y, c.scale(0.75));
            }
            b.rgb(x0, y0 + 3, Rgb(60, 220, 120));
        }
    }
    for y in 27..33 {
        for x in 16..20 {
            b.rgb(x, y, if y == 29 && x == 18 { Rgb(30, 30, 34) } else { Rgb(170, 172, 180) });
        }
    }
    for y in 33..37 {
        for x in 4..15 {
            b.rgb(x, y, Rgb(22, 22, 26));
        }
    }
    for x in 1..23 {
        b.rgb(x, h as i32 - 1, Rgb(30, 30, 34));
    }
    for x in 0..24 {
        let dx = (x as f32 - 11.5) / 12.0;
        let top = 4.0 - (1.0 - dx * dx) * 3.0 - noise(x, 0, 52) * 0.8;
        for y in top.max(0.0) as i32..5 {
            b.rgb(x, y, snow_tone(0.95 - dx * 0.3, x, y));
        }
    }
    b
}

/// Japanese expressway signs: katakana and their romaji, distance in km.
pub const ROAD_SIGNS: [(&str, &str); 4] = [("サクラ", "SAKURA 12"), ("スロップレーサー", "SLOPRACER 5"), ("ネオン", "NEON 24"), ("ターボ", "TURBO 31")];

/// Green expressway sign with white lettering and snow along its top.
fn make_road_sign(kana_text: &str, latin: &str) -> Bitmap {
    let w = 66.max(kana_text.chars().count() * 12 + 12).max(latin.chars().count() * 6 + 12);
    let h = 48;
    let mut b = Bitmap::new(w, h);
    for x in [w as i32 / 5, w as i32 * 4 / 5] {
        for y in 32..h as i32 {
            b.rgb(x, y, Rgb(196, 200, 210));
            b.rgb(x + 1, y, Rgb(120, 124, 138));
        }
    }
    let white = Rgb(250, 252, 250);
    for y in 2..33 {
        for x in 0..w as i32 {
            let border = x == 2 || x == w as i32 - 3 || y == 4 || y == 30;
            let c = if border && (2..w as i32 - 2).contains(&x) && (4..31).contains(&y) {
                white
            } else {
                Rgb(18, 122, 78).lerp(Rgb(10, 92, 60), (y - 2) as f32 / 31.0)
            };
            b.rgb(x, y, c);
        }
    }
    let glyphs: Vec<[&str; 10]> = kana_text.chars().filter_map(|c| kana(c, false)).collect();
    let kx = (w as i32 - (glyphs.len() as i32 * 12 - 2)) / 2;
    for (i, g) in glyphs.iter().enumerate() {
        for (gy, row) in g.iter().enumerate() {
            for (gx, ch) in row.bytes().enumerate() {
                if ch == b'#' {
                    b.rgb(kx + i as i32 * 12 + gx as i32, 7 + gy as i32, white);
                }
            }
        }
    }
    let lw = latin.chars().count() as i32 * 6 - 1;
    b.text(latin, (w as i32 - lw) / 2, 21, 1, white, Rgb(8, 70, 44));
    for x in 0..w as i32 {
        let depth = 1 + (noise(x / 3, 0, 53) * 2.5) as i32;
        for y in 2 - depth + 1..3 {
            b.rgb(x, y.max(0), snow_tone(0.95 - x as f32 / w as f32 * 0.2, x, y));
        }
    }
    b
}

/// A two-ball yukidaruma wearing a red bucket.
fn make_snowman() -> Bitmap {
    let (w, h) = (20, 30);
    let mut b = Bitmap::new(w, h);
    blobs(&mut b, &[(10.0, 21.0, 8.0), (10.0, 10.0, 5.8)], &SNOW[1..], 54, 0.03);
    for (x, y) in [(8, 10), (12, 10)] {
        b.rgb(x, y, Rgb(30, 30, 36));
    }
    for (x, y) in [(7, 12), (13, 12)] {
        b.rgb(x, y, Rgb(250, 160, 170));
    }
    b.rgb(10, 13, Rgb(120, 60, 50));
    for y in 1..6 {
        let half = 3 + (y - 1) / 2;
        for x in 10 - half..10 + half {
            let l = 0.5 + (10 - x) as f32 / 8.0;
            b.rgb(x + (5 - y) / 3, y, Rgb(200, 34, 40).lerp(Rgb(255, 110, 100), l.clamp(0.0, 1.0) * 0.5));
        }
    }
    b
}

/// Roadside snow marker: a red-and-white pole with a downward arrow board.
fn make_snow_pole() -> Bitmap {
    let (w, h) = (12, 64);
    let mut b = Bitmap::new(w, h);
    for y in 12..h as i32 {
        let red = (y / 5) % 2 == 0;
        let (lit, dark) = if red { (Rgb(236, 50, 50), Rgb(150, 24, 30)) } else { (Rgb(250, 250, 250), Rgb(190, 196, 210)) };
        b.rgb(5, y, lit);
        b.rgb(6, y, dark);
    }
    for y in 0..12 {
        let half = (11 - y) as f32 / 11.0 * 6.0;
        for x in 0..w as i32 {
            let dx = (x as f32 + 0.5 - 6.0).abs();
            if dx <= half {
                let edge = dx > half - 1.3 || y == 0;
                b.rgb(x, y, if edge { Rgb(250, 250, 250) } else { Rgb(228, 40, 44) });
            }
        }
    }
    b
}

fn make_city_building(variant: usize) -> Bitmap {
    let mut building = Bitmap::new(96, 192);
    let top = [12, 40, 64, 108][variant];
    let facade = [Rgb(28, 34, 55), Rgb(38, 29, 54), Rgb(24, 40, 58), Rgb(38, 31, 48)][variant];
    for vertical in top..192 {
        for horizontal in 8..90 {
            let side = horizontal >= 72;
            let seam = (horizontal - 8) % 16 == 0 || (vertical - top) % 18 == 0;
            let shade = if side { 0.48 } else if seam { 0.68 } else { 1.0 };
            building.rgb(horizontal, vertical, facade.scale(shade));
        }
    }
    for vertical in top + 9..162 {
        for horizontal in 14..87 {
            let column = (horizontal - 14) / 11;
            let floor = (vertical - top - 9) / 18;
            let window = (horizontal - 14) % 11 < 5 && (vertical - top - 9) % 18 < 9;
            if window && (column * 13 + floor * 7 + variant as i32 * 11) % 5 != 0 {
                let light = if (column + floor) % 3 == 0 { Rgb(244, 192, 124) } else { Rgb(95, 162, 193) };
                building.rgb(horizontal, vertical, light.scale(if horizontal >= 72 { 0.45 } else { 0.72 }));
            }
        }
    }
    for horizontal in 6..92 {
        building.set(horizontal, top, Px::Body(0.65));
        building.rgb(horizontal, top + 2, Rgb(65, 72, 95));
    }
    for vertical in top + 4..164 {
        building.set(9, vertical, Px::Body(0.55));
        building.set(71, vertical, Px::Body(0.75));
    }
    for vertical in top - 7..top {
        for horizontal in 42..61 {
            building.rgb(horizontal, vertical, Rgb(39, 46, 65));
        }
    }
    let sign_top = top + 12;
    neon_panel(&mut building, 72, sign_top, 20, 44);
    kana_line(&mut building, NEON_TEXT[1 + variant % 3].0, 77, sign_top + 5, true);
    neon_panel(&mut building, 9, 161, 64, 15);
    kana_line(&mut building, NEON_TEXT[1 + variant % 3].0, 18, 164, false);
    for vertical in 178..192 {
        for horizontal in 13..69 {
            let frame = (horizontal - 13) % 14 < 2;
            let light = if variant == 3 { Rgb(240, 151, 83) } else { Rgb(60, 126, 150) };
            building.rgb(horizontal, vertical, if frame { Rgb(38, 46, 64) } else { light.scale(0.65) });
        }
    }
    if variant == 3 {
        for vertical in 153..161 {
            let inset = (160 - vertical) / 2;
            for horizontal in 3 + inset..91 - inset {
                building.rgb(horizontal, vertical, if vertical % 3 == 0 { Rgb(150, 69, 90) } else { Rgb(64, 34, 55) });
            }
        }
    }
    building
}

pub struct Sprites {
    pub car_rear: Bitmap,
    pub car_front: Bitmap,
    pub car_side: Bitmap,
    pub tree: Bitmap,
    pub pine: Bitmap,
    pub bush: Bitmap,
    pub rock: Bitmap,
    pub chevron: Bitmap,
    pub tyres: Bitmap,
    pub billboards: Vec<Bitmap>,
    pub lamp: Bitmap,
    pub gantry: Bitmap,
    pub grandstand: Bitmap,
    pub palm: Bitmap,
    pub sakura: Bitmap,
    pub torii: Bitmap,
    pub neon_signs: Vec<Bitmap>,
    pub neon_boards: Vec<Bitmap>,
    pub neon_chevron: Bitmap,
    pub snow_cedar: Bitmap,
    pub birch: Bitmap,
    pub lantern: Bitmap,
    pub vending: Bitmap,
    pub snow_torii: Bitmap,
    pub road_signs: Vec<Bitmap>,
    pub snowman: Bitmap,
    pub snow_pole: Bitmap,
    pub city_buildings: Vec<Bitmap>,
}

pub const BILLBOARD_TEXT: [&str; 5] = ["SLOPRACER", "TURBO", "VOLT OIL", "F1", "ZORKA"];

pub fn get() -> &'static Sprites {
    static S: OnceLock<Sprites> = OnceLock::new();
    S.get_or_init(|| Sprites {
        car_rear: Bitmap::from_art(CAR_REAR),
        car_front: Bitmap::from_art(CAR_FRONT),
        car_side: Bitmap::from_art(CAR_SIDE),
        tree: make_tree(),
        pine: make_pine(),
        bush: make_bush(),
        rock: make_rock(),
        chevron: make_chevron(),
        tyres: make_tyres(),
        billboards: BILLBOARD_TEXT.iter().map(|t| make_billboard(t)).collect(),
        lamp: make_lamp(),
        gantry: make_gantry(),
        grandstand: make_grandstand(),
        palm: make_palm(),
        sakura: make_sakura(),
        torii: make_torii(false),
        neon_signs: NEON_TEXT.iter().map(|(k, _)| make_neon_sign(k)).collect(),
        neon_boards: NEON_TEXT.iter().map(|(k, l)| make_neon_board(k, l)).collect(),
        neon_chevron: make_neon_chevron(),
        snow_cedar: make_snow_cedar(),
        birch: make_birch(),
        lantern: make_lantern(),
        vending: make_vending(),
        snow_torii: make_torii(true),
        road_signs: ROAD_SIGNS.iter().map(|(k, l)| make_road_sign(k, l)).collect(),
        snowman: make_snowman(),
        snow_pole: make_snow_pole(),
        city_buildings: (0..4).map(make_city_building).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slopracer_signs_have_all_glyphs_and_fit_their_panels() {
        assert_eq!(BILLBOARD_TEXT[0], "SLOPRACER");
        for (japanese, latin) in NEON_TEXT.iter().chain(ROAD_SIGNS.iter()) {
            for character in japanese.chars() {
                let rows = kana(character, false).expect("missing horizontal sign glyph");
                assert!(rows.iter().all(|row| row.len() == 10));
                assert!(kana(character, true).is_some());
            }
            let board = make_neon_board(japanese, latin);
            let road_sign = make_road_sign(japanese, latin);
            let sign = make_neon_sign(japanese);
            let japanese_width = japanese.chars().count() * 12 - 2;
            let latin_width = crate::gfx::text_width(latin, 1) as usize;
            assert!(board.w >= japanese_width + 12 && board.w >= latin_width + 12);
            assert!(road_sign.w >= japanese_width + 12 && road_sign.w >= latin_width + 12);
            assert!(sign.h >= japanese.chars().count() * 12 + 6);
        }
    }

    /// Writes the synthwave sprites, enlarged 4x, to `target/snapshots/sprites.ppm`.
    #[test]
    fn sprite_sheet() {
        let s = get();
        let sheet: Vec<(&Bitmap, Rgb)> = vec![
            (&s.palm, Rgb(255, 255, 255)),
            (&s.sakura, Rgb(255, 255, 255)),
            (&s.neon_signs[0], Rgb(255, 50, 190)),
            (&s.neon_signs[2], Rgb(40, 230, 255)),
            (&s.neon_boards[0], Rgb(255, 200, 60)),
            (&s.neon_boards[3], Rgb(180, 90, 255)),
            (&s.neon_chevron, Rgb(40, 230, 255)),
            (&s.torii, Rgb(255, 255, 255)),
        ];
        write_sheet(&sheet, "sprites.ppm", Rgb(60, 30, 80));
        let winter: Vec<(&Bitmap, Rgb)> = vec![
            (&s.snow_cedar, Rgb(255, 255, 255)),
            (&s.birch, Rgb(255, 255, 255)),
            (&s.lantern, Rgb(255, 255, 255)),
            (&s.vending, Rgb(220, 40, 48)),
            (&s.vending, Rgb(40, 110, 210)),
            (&s.road_signs[1], Rgb(255, 255, 255)),
            (&s.snowman, Rgb(255, 255, 255)),
            (&s.snow_pole, Rgb(255, 255, 255)),
            (&s.snow_torii, Rgb(255, 255, 255)),
        ];
        write_sheet(&winter, "sprites_winter.ppm", Rgb(120, 170, 230));
    }

    fn write_sheet(sheet: &[(&Bitmap, Rgb)], name: &str, bg: Rgb) {
        let k = 4;
        let w: usize = sheet.iter().map(|(b, _)| b.w * k + 8).sum();
        let h = sheet.iter().map(|(b, _)| b.h * k).max().unwrap();
        let mut px = vec![bg; w * h];
        let mut x0 = 0;
        for &(b, body) in sheet {
            for y in 0..b.h * k {
                for x in 0..b.w * k {
                    if let Some(c) = b.get(x / k, y / k).resolve(body) {
                        px[y * w + x0 + x] = c;
                    }
                }
            }
            x0 += b.w * k + 8;
        }
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        px.iter().for_each(|c| out.extend_from_slice(&[c.0, c.1, c.2]));
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/snapshots");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), out).unwrap();
    }
}
