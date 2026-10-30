//! Pixel framebuffer rendered into the terminal with half-block characters,
//! plus a tiny 5x7 bitmap font for big in-game text.

use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn lerp(self, o: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let f = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(f(self.0, o.0), f(self.1, o.1), f(self.2, o.2))
    }

    pub fn scale(self, k: f32) -> Rgb {
        let f = |a: u8| (a as f32 * k).clamp(0.0, 255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
    }

    pub fn to_color(self, truecolor: bool) -> Color {
        if truecolor {
            return Color::Rgb(self.0, self.1, self.2);
        }
        // Nearest entry of the xterm 6x6x6 colour cube.
        let q = |v: u8| -> u8 {
            match v {
                0..=47 => 0,
                48..=114 => 1,
                _ => ((v as u16 - 35) / 40) as u8,
            }
        };
        Color::Indexed(16 + 36 * q(self.0) + 6 * q(self.1) + q(self.2))
    }
}

pub struct FrameBuf {
    pub w: usize,
    pub h: usize,
    px: Vec<Rgb>,
}

impl FrameBuf {
    pub fn new() -> Self {
        Self { w: 0, h: 0, px: Vec::new() }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.px = vec![Rgb::default(); w * h];
        }
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Rgb {
        self.px[y * self.w + x]
    }

    pub fn row_mut(&mut self, y: usize) -> &mut [Rgb] {
        &mut self.px[y * self.w..(y + 1) * self.w]
    }

    pub fn fill_rect(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: Rgb) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.set(x, y, c);
            }
        }
    }

    /// Darken the whole frame (used behind overlays).
    pub fn dim(&mut self, k: f32) {
        for p in &mut self.px {
            *p = p.scale(k);
        }
    }

    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let p = &mut self.px[y as usize * self.w + x as usize];
            *p = p.lerp(c, a);
        }
    }

    /// Filled ellipse; `a` < 1 blends with what is underneath.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb, a: f32) {
        let (x0, x1) = ((cx - rx).floor() as i32, (cx + rx).ceil() as i32);
        let (y0, y1) = ((cy - ry).floor() as i32, (cy + ry).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.blend(x, y, c, a);
                }
            }
        }
    }
}

/// How the framebuffer is mapped onto the terminal.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    /// Real pixels via the kitty graphics protocol.
    Pixels,
    /// 2x2 pixels per cell using quadrant block glyphs (two colours per cell).
    Quadrant,
    /// 1x2 pixels per cell using the upper half block.
    HalfBlock,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Pixels => "Pixels (kitty graphics)",
            Mode::Quadrant => "Quadrant blocks",
            Mode::HalfBlock => "Half blocks",
        }
    }
}

const QUADS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

fn avg(px: &[Rgb], mask: u8, want: bool) -> Rgb {
    let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
    for (i, p) in px.iter().enumerate() {
        if ((mask >> i) & 1 == 1) == want {
            r += p.0 as u32;
            g += p.1 as u32;
            b += p.2 as u32;
            n += 1;
        }
    }
    let n = n.max(1);
    Rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
}

fn dist2(a: Rgb, b: Rgb) -> i32 {
    let (dr, dg, db) = (a.0 as i32 - b.0 as i32, a.1 as i32 - b.1 as i32, a.2 as i32 - b.2 as i32);
    dr * dr + dg * dg + db * db
}

/// Pick the quadrant glyph and two colours that best approximate 4 pixels.
fn quadrant(px: [Rgb; 4]) -> (char, Rgb, Rgb) {
    let all = avg(&px, 0, false);
    let mut best = (px.iter().map(|&p| dist2(p, all)).sum::<i32>(), 0u8, all, all);
    if best.0 < 48 {
        return (' ', all, all);
    }
    for mask in 1u8..=7 {
        let fg = avg(&px, mask, true);
        let bg = avg(&px, mask, false);
        let err: i32 = px
            .iter()
            .enumerate()
            .map(|(i, &p)| dist2(p, if (mask >> i) & 1 == 1 { fg } else { bg }))
            .sum();
        if err < best.0 {
            best = (err, mask, fg, bg);
        }
    }
    (QUADS[best.1 as usize], best.2, best.3)
}

#[cfg(test)]
pub const QUADS_FOR_TEST: [char; 16] = QUADS;
#[cfg(test)]
pub fn quadrant_for_test(px: [Rgb; 4]) -> (char, Rgb, Rgb) {
    quadrant(px)
}

/// Widget that draws the framebuffer as terminal cells. In `Pixels` mode the
/// cells are cleared so the graphics-protocol image behind them shows through.
pub struct FrameView<'a> {
    pub fb: &'a FrameBuf,
    pub truecolor: bool,
    pub mode: Mode,
}

impl Widget for FrameView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let fb = self.fb;
        let tc = self.truecolor;
        for r in 0..area.height as usize {
            for c in 0..area.width as usize {
                let Some(cell) = buf.cell_mut((area.x + c as u16, area.y + r as u16)) else { continue };
                match self.mode {
                    Mode::Pixels => {
                        cell.set_char(' ').set_fg(Color::Reset).set_bg(Color::Reset);
                    }
                    Mode::HalfBlock if c < fb.w && r * 2 + 1 < fb.h => {
                        cell.set_char('▀')
                            .set_fg(fb.get(c, r * 2).to_color(tc))
                            .set_bg(fb.get(c, r * 2 + 1).to_color(tc));
                    }
                    Mode::Quadrant if c * 2 + 1 < fb.w && r * 2 + 1 < fb.h => {
                        let (x, y) = (c * 2, r * 2);
                        let px = [fb.get(x, y), fb.get(x + 1, y), fb.get(x, y + 1), fb.get(x + 1, y + 1)];
                        let (ch, fg, bg) = quadrant(px);
                        cell.set_char(ch).set_fg(fg.to_color(tc)).set_bg(bg.to_color(tc));
                    }
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 5x7 bitmap font
// ---------------------------------------------------------------------------

const FONT: &[(char, [&str; 7])] = &[
    ('A', [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', ["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"]),
    ('J', ["..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."]),
    ('K', ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', ["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', [".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"]),
    ('X', ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', ["####.", "....#", "....#", ".###.", "....#", "....#", "####."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."]),
    ('!', ["..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#.."]),
    (':', [".....", "..#..", "..#..", ".....", "..#..", "..#..", "....."]),
    ('.', [".....", ".....", ".....", ".....", ".....", ".....", "..#.."]),
    ('-', [".....", ".....", ".....", "#####", ".....", ".....", "....."]),
    ('/', ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."]),
];

pub fn glyph(ch: char) -> Option<&'static [&'static str; 7]> {
    let ch = ch.to_ascii_uppercase();
    FONT.iter().find(|(c, _)| *c == ch).map(|(_, g)| g)
}

pub fn text_width(s: &str, sx: i32) -> i32 {
    let n = s.chars().count() as i32;
    (n * 6 - 1).max(0) * sx
}

/// Draw text with a vertical colour gradient and an optional drop shadow.
/// `sx`/`sy` are the horizontal and vertical pixel scale of the font.
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    fb: &mut FrameBuf,
    s: &str,
    x: i32,
    y: i32,
    (sx, sy): (i32, i32),
    top: Rgb,
    bottom: Rgb,
    shadow: Option<Rgb>,
) {
    let passes: &[(i32, bool)] = if shadow.is_some() { &[(1, true), (0, false)] } else { &[(0, false)] };
    for &(off, is_shadow) in passes {
        for (i, ch) in s.chars().enumerate() {
            let Some(g) = glyph(ch) else { continue };
            let gx = x + i as i32 * 6 * sx;
            for (row, line) in g.iter().enumerate() {
                let color = match (is_shadow, shadow) {
                    (true, Some(sc)) => sc,
                    _ => top.lerp(bottom, row as f32 / 6.0),
                };
                for (col, b) in line.bytes().enumerate() {
                    if b == b'#' {
                        let px = gx + col as i32 * sx + off * sx;
                        let py = y + row as i32 * sy + off * sy;
                        fb.fill_rect(px, py, sx, sy, color);
                    }
                }
            }
        }
    }
}

pub fn draw_text_centered(
    fb: &mut FrameBuf,
    s: &str,
    y: i32,
    scale: (i32, i32),
    top: Rgb,
    bottom: Rgb,
    shadow: Option<Rgb>,
) {
    let x = (fb.w as i32 - text_width(s, scale.0)) / 2;
    draw_text(fb, s, x, y, scale, top, bottom, shadow);
}
