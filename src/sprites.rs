//! Sprite bitmaps. Cars are hand-drawn pixel art (recoloured per team via
//! "body" pixels); scenery is generated procedurally at start-up with
//! 16-bit style dithered shading.

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
    b.text("PEGASUS GP", (w as i32 - 59) / 2, 5, 1, Rgb(255, 214, 40), Rgb(0, 0, 20));
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
        'ペ' => [
            ".......##.", "......#..#", ".......##.", "...#......", "..#.#.....",
            ".#...#....", "#.....#...", ".......#..", "........#.", ".........#",
        ],
        'ガ' => [
            "...#..#.#.", "...#...#.#", "########..", "...#....#.", "...#....#.",
            "..#.....#.", "..#.....#.", ".#......#.", "#.......#.", "......##..",
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
pub const NEON_TEXT: [(&str, &str); 4] = [("ペガサス", "PEGASUS"), ("ネオン", "NEON"), ("ターボ", "TURBO"), ("サクラ", "SAKURA")];

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
    let (w, h) = (22, 76);
    let mut b = Bitmap::new(w, h);
    let n = text.chars().count() as i32;
    let ph = n * 12 + 6;
    pole(&mut b, 10, ph, h as i32);
    neon_panel(&mut b, 0, 0, w as i32, ph);
    kana_line(&mut b, text, 6, 4, true);
    b
}

/// Wide sign: katakana on top, the Latin reading below.
fn make_neon_board(kana_text: &str, latin: &str) -> Bitmap {
    let (w, h) = (66, 44);
    let mut b = Bitmap::new(w, h);
    for x in [12, 52] {
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

/// Vermilion torii gate spanning the road.
fn make_torii() -> Bitmap {
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
    b
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
}

pub const BILLBOARD_TEXT: [&str; 5] = ["PEGASUS", "TURBO", "VOLT OIL", "F1", "ZORKA"];

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
        torii: make_torii(),
        neon_signs: NEON_TEXT.iter().map(|(k, _)| make_neon_sign(k)).collect(),
        neon_boards: NEON_TEXT.iter().map(|(k, l)| make_neon_board(k, l)).collect(),
        neon_chevron: make_neon_chevron(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let k = 4;
        let w: usize = sheet.iter().map(|(b, _)| b.w * k + 8).sum();
        let h = sheet.iter().map(|(b, _)| b.h * k).max().unwrap();
        let mut px = vec![Rgb(60, 30, 80); w * h];
        let mut x0 = 0;
        for (b, body) in sheet {
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
        std::fs::write(dir.join("sprites.ppm"), out).unwrap();
    }
}
