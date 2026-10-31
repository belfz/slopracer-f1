//! Frame output through the kitty terminal graphics protocol (kitty, Ghostty,
//! WezTerm). Each frame is zlib-compressed RGB, double-buffered between two
//! image ids so the swap is flicker-free, and placed below cell backgrounds
//! so ratatui panels drawn on top stay visible.

use crate::gfx::FrameBuf;
use base64::Engine;
use flate2::{write::ZlibEncoder, Compression};
use ratatui::layout::Rect;
use std::io::{self, Write};

/// Below cells with non-default backgrounds (kitty spec: < INT32_MIN / 2).
const Z_UNDER: i64 = -1_073_741_825;
const CHUNK: usize = 4096;

pub struct Kitty {
    live: Option<u32>,
    raw: Vec<u8>,
    out: Vec<u8>,
}

impl Kitty {
    pub fn new() -> Kitty {
        Kitty { live: None, raw: Vec::new(), out: Vec::new() }
    }

    /// Terminals known to implement the kitty graphics protocol.
    pub fn detect() -> bool {
        let env = |k: &str| std::env::var(k).unwrap_or_default();
        env("TERM") == "xterm-kitty"
            || env("TERM") == "xterm-ghostty"
            || !env("KITTY_WINDOW_ID").is_empty()
            || !env("GHOSTTY_RESOURCES_DIR").is_empty()
            || matches!(env("TERM_PROGRAM").as_str(), "ghostty" | "WezTerm")
    }

    /// Send `fb`, upscaled by an integer `scale`, to cover `rect` (in cells).
    pub fn draw(&mut self, w: &mut impl Write, fb: &FrameBuf, rect: Rect, scale: usize) -> io::Result<()> {
        if fb.w == 0 || fb.h == 0 || rect.width == 0 || rect.height == 0 {
            return Ok(());
        }
        let scale = scale.max(1);
        let (iw, ih) = (fb.w * scale, fb.h * scale);
        self.raw.clear();
        self.raw.reserve(iw * ih * 3);
        for y in 0..fb.h {
            let start = self.raw.len();
            for x in 0..fb.w {
                let p = fb.get(x, y);
                for _ in 0..scale {
                    self.raw.extend_from_slice(&[p.0, p.1, p.2]);
                }
            }
            for _ in 1..scale {
                self.raw.extend_from_within(start..start + iw * 3);
            }
        }
        let mut enc = ZlibEncoder::new(Vec::with_capacity(self.raw.len() / 8), Compression::fast());
        enc.write_all(&self.raw)?;
        let compressed = enc.finish()?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(compressed);

        let id = if self.live == Some(1) { 2 } else { 1 };
        self.out.clear();
        write!(self.out, "\x1b7\x1b[{};{}H", rect.y + 1, rect.x + 1)?;
        let chunks: Vec<&[u8]> = b64.as_bytes().chunks(CHUNK).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let more = (i + 1 < chunks.len()) as u8;
            if i == 0 {
                write!(
                    self.out,
                    "\x1b_Ga=T,f=24,o=z,s={iw},v={ih},i={id},p=1,c={},r={},C=1,q=2,z={Z_UNDER},m={more};",
                    rect.width, rect.height
                )?;
            } else {
                write!(self.out, "\x1b_Gm={more};")?;
            }
            self.out.extend_from_slice(chunk);
            self.out.extend_from_slice(b"\x1b\\");
        }
        if let Some(old) = self.live {
            write!(self.out, "\x1b_Ga=d,d=I,i={old},q=2\x1b\\")?;
        }
        self.out.extend_from_slice(b"\x1b8");
        self.live = Some(id);
        w.write_all(&self.out)?;
        w.flush()
    }

    /// A handle that assumes images may be on screen (for cleanup on exit).
    pub fn new_live() -> Kitty {
        Kitty { live: Some(1), raw: Vec::new(), out: Vec::new() }
    }

    pub fn clear(&mut self, w: &mut impl Write) -> io::Result<()> {
        if self.live.take().is_some() {
            w.write_all(b"\x1b_Ga=d,d=A,q=2\x1b\\")?;
            w.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::Race;
    use crate::render::{render_world, Camera};
    use crate::track::all_tracks;
    use std::rc::Rc;
    use std::time::Instant;

    #[test]
    fn frame_budget() {
        let mut race = Race::new(Rc::new(all_tracks().remove(0)), 3, true, true);
        for _ in 0..600 {
            race.update(1.0 / 60.0, Default::default());
        }
        let mut fb = FrameBuf::new();
        let mut sprites = Vec::new();
        let mut kitty = Kitty::new();
        let mut sink = Vec::new();
        for (w, h, scale) in [(384, 216, 2), (400, 224, 1), (400, 160, 1), (800, 448, 1)] {
            fb.resize(w, h);
            let t = Instant::now();
            let n = 60;
            for _ in 0..n {
                race.update(1.0 / 60.0, Default::default());
                race.sprites(&mut sprites);
                let cam = Camera { d: race.cam_d(), x: race.x, bg: race.bg };
                render_world(&mut fb, &race.track, &cam, &mut sprites, 1.0, 1.0);
                sink.clear();
                kitty.draw(&mut sink, &fb, Rect::new(0, 0, 140, 40), scale).unwrap();
            }
            println!("{w}x{h} x{scale}: {:.2} ms/frame, {} KB/frame", t.elapsed().as_secs_f64() * 1000.0 / n as f64, sink.len() / 1024);
        }
    }
}
