//! Browser front end: the same `App` and ratatui UI, painted onto a canvas.
//! ratatui renders into an in-memory buffer; its cells are drawn with the
//! browser's monospace font (block and box glyphs as exact shapes, so they
//! tile without seams). As with kitty graphics in the terminal, the game's
//! framebuffer goes underneath at full resolution and cells with a default
//! background stay transparent over it.

use crate::app::App;
use crate::gfx::{FrameBuf, Mode, QUADS};
use crate::input::{Key, KeyEvent, KeyKind};
use crate::render::SpriteInst;
use crate::ui;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent, Window};
use web_time::Instant;

const FONT: &str = "Menlo, 'SF Mono', Monaco, 'DejaVu Sans Mono', Consolas, monospace";
/// The UI wants about this many cells; the font shrinks to fit the window.
const MIN_COLS: f64 = 124.0;
const MIN_ROWS: f64 = 38.0;
/// Cell height relative to the font size.
const LINE: f64 = 1.25;
const DEFAULT_FG: &str = "#e6e6e6";

enum Input {
    Key(KeyEvent),
    Blur,
    Resize,
}

/// Paints a ratatui buffer (and the framebuffer image) onto the canvas.
struct Painter {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    /// Offscreen canvas holding the framebuffer at native resolution.
    img: HtmlCanvasElement,
    img_ctx: CanvasRenderingContext2d,
    rgba: Vec<u8>,
    /// Cell size and grid origin, in device pixels.
    cell: (f64, f64),
    origin: (f64, f64),
    font_px: f64,
    fill: String,
    font: String,
}

struct Web {
    window: Window,
    app: App,
    term: Terminal<TestBackend>,
    fb: FrameBuf,
    sprites: Vec<SpriteInst>,
    painter: Painter,
    last: Option<f64>,
}

fn context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    Ok(canvas.get_context("2d")?.ok_or("no 2d context")?.dyn_into()?)
}

/// Starts the game on the canvas with id `canvas_id`. Call it from a user
/// gesture (click or key press) so the browser lets the audio play.
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas: HtmlCanvasElement = document.get_element_by_id(canvas_id).ok_or("no canvas")?.dyn_into()?;
    let img: HtmlCanvasElement = document.create_element("canvas")?.dyn_into()?;
    let painter = Painter {
        ctx: context(&canvas)?,
        img_ctx: context(&img)?,
        canvas,
        img,
        rgba: Vec::new(),
        cell: (1.0, 1.0),
        origin: (0.0, 0.0),
        font_px: 16.0,
        fill: String::new(),
        font: String::new(),
    };
    // Browsers report key releases, so the keyboard is always "enhanced".
    let app = App::new(true, true, false, Mode::Pixels, true, true);
    let term = Terminal::new(TestBackend::new(1, 1)).map_err(|e| e.to_string())?;
    let mut web = Web { window: window.clone(), app, term, fb: FrameBuf::new(), sprites: Vec::new(), painter, last: None };
    web.layout()?;

    let inputs: Rc<RefCell<Vec<Input>>> = Rc::default();
    listen_keys(&window, &inputs)?;
    listen(&window, "blur", &inputs, || Input::Blur)?;
    listen(&window, "resize", &inputs, || Input::Resize)?;

    // The animation loop: each frame schedules the next.
    let web = Rc::new(RefCell::new(web));
    let tick: Rc<RefCell<Option<FrameCallback>>> = Rc::default();
    let next = tick.clone();
    *tick.borrow_mut() = Some(Closure::new(move |t: f64| {
        if let Err(e) = web.borrow_mut().frame(t, &inputs) {
            web_sys::console::error_1(&e);
        }
        if let Some(cb) = next.borrow().as_ref() {
            let _ = web_sys::window().map(|w| w.request_animation_frame(cb.as_ref().unchecked_ref()));
        }
    }));
    window.request_animation_frame(tick.borrow().as_ref().ok_or("no tick")?.as_ref().unchecked_ref())?;
    Ok(())
}

type FrameCallback = Closure<dyn FnMut(f64)>;

fn listen(window: &Window, event: &str, inputs: &Rc<RefCell<Vec<Input>>>, input: fn() -> Input) -> Result<(), JsValue> {
    let q = inputs.clone();
    let cb = Closure::<dyn FnMut()>::new(move || q.borrow_mut().push(input()));
    window.add_event_listener_with_callback(event, cb.as_ref().unchecked_ref())?;
    cb.forget();
    Ok(())
}

fn listen_keys(window: &Window, inputs: &Rc<RefCell<Vec<Input>>>) -> Result<(), JsValue> {
    for (event, release) in [("keydown", false), ("keyup", true)] {
        let q = inputs.clone();
        let cb = Closure::<dyn FnMut(KeyboardEvent)>::new(move |e: KeyboardEvent| {
            // Leave browser shortcuts (reload, tabs, zoom) alone.
            if e.meta_key() || e.ctrl_key() || e.alt_key() {
                return;
            }
            let key = web_key(&e.key());
            if matches!(key, Key::Up | Key::Down | Key::Left | Key::Right | Key::Tab | Key::Char(' ')) {
                e.prevent_default();
            }
            let kind = match (release, e.repeat()) {
                (true, _) => KeyKind::Release,
                (false, true) => KeyKind::Repeat,
                (false, false) => KeyKind::Press,
            };
            q.borrow_mut().push(Input::Key(KeyEvent { key, kind, ctrl: false }));
        });
        window.add_event_listener_with_callback(event, cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    Ok(())
}

fn web_key(key: &str) -> Key {
    match key {
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "Enter" => Key::Enter,
        "Escape" => Key::Esc,
        "Tab" => Key::Tab,
        _ => {
            let mut chars = key.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Key::Char(c),
                _ => Key::Other,
            }
        }
    }
}

impl Web {
    /// Size the canvas to the window and pick a font that fits the UI.
    fn layout(&mut self) -> Result<(), JsValue> {
        let w = &self.window;
        let dpr = w.device_pixel_ratio();
        let css_w = w.inner_width()?.as_f64().unwrap_or(1280.0);
        let css_h = w.inner_height()?.as_f64().unwrap_or(720.0);
        let p = &mut self.painter;
        p.canvas.set_width((css_w * dpr).round() as u32);
        p.canvas.set_height((css_h * dpr).round() as u32);
        let font_css = (css_w / (MIN_COLS * 0.6)).min(css_h / (MIN_ROWS * LINE)).clamp(8.0, 20.0);
        p.font_px = (font_css * dpr).round().max(6.0);
        // Resizing the canvas reset its state.
        p.font.clear();
        p.fill.clear();
        p.ctx.set_font(&format!("{}px {FONT}", p.font_px));
        let cw = p.ctx.measure_text("M")?.width().round().max(1.0);
        let ch = (p.font_px * LINE).round();
        let (pw, ph) = (p.canvas.width() as f64, p.canvas.height() as f64);
        let cols = (pw / cw).floor().clamp(1.0, u16::MAX as f64) as u16;
        let rows = (ph / ch).floor().clamp(1.0, u16::MAX as f64) as u16;
        p.cell = (cw, ch);
        p.origin = (((pw - cols as f64 * cw) / 2.0).floor(), ((ph - rows as f64 * ch) / 2.0).floor());
        self.term = Terminal::new(TestBackend::new(cols, rows)).map_err(|e| e.to_string())?;
        self.app.cell_px = (cw as f32, ch as f32);
        Ok(())
    }

    fn frame(&mut self, t: f64, inputs: &RefCell<Vec<Input>>) -> Result<(), JsValue> {
        let dt = self.last.map_or(0.0, |l| ((t - l) / 1000.0) as f32).clamp(0.0, 0.05);
        self.last = Some(t);
        let pending: Vec<Input> = inputs.borrow_mut().drain(..).collect();
        for input in pending {
            match input {
                Input::Key(k) => self.app.on_key(k, Instant::now()),
                Input::Blur => self.app.focus_lost(),
                Input::Resize => self.layout()?,
            }
        }
        // A browser tab can't quit; closing it does.
        self.app.quit = false;
        self.app.poll_pad();
        self.app.update(dt, Instant::now());

        let mut image = None;
        let (app, fb, sprites) = (&self.app, &mut self.fb, &mut self.sprites);
        self.term.draw(|f| image = ui::draw(f, app, fb, sprites)).map_err(|e| e.to_string())?;
        self.painter.paint(self.term.backend().buffer(), image.map(|(r, _)| r), &self.fb)
    }
}

impl Painter {
    fn set_fill(&mut self, css: &str) {
        if self.fill != css {
            self.ctx.set_fill_style_str(css);
            self.ctx.set_stroke_style_str(css);
            self.fill.clear();
            self.fill.push_str(css);
        }
    }

    fn set_font(&mut self, m: Modifier) {
        let bold = if m.contains(Modifier::BOLD) { "bold " } else { "" };
        let italic = if m.contains(Modifier::ITALIC) { "italic " } else { "" };
        let font = format!("{italic}{bold}{}px {FONT}", self.font_px);
        if self.font != font {
            self.ctx.set_font(&font);
            self.font = font;
        }
    }

    fn cell_xy(&self, x: u16, y: u16) -> (f64, f64) {
        (self.origin.0 + x as f64 * self.cell.0, self.origin.1 + y as f64 * self.cell.1)
    }

    fn paint(&mut self, buf: &Buffer, image: Option<Rect>, fb: &FrameBuf) -> Result<(), JsValue> {
        self.set_fill("#000");
        self.ctx.fill_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);
        if let Some(r) = image {
            self.blit(fb, r)?;
        }
        let area = buf.area;
        let (cw, ch) = self.cell;
        // Backgrounds first, merged into runs of equal colour.
        for y in area.top()..area.bottom() {
            let mut x = area.left();
            while x < area.right() {
                let bg = colors(&buf[(x, y)]).1;
                let start = x;
                while x < area.right() && colors(&buf[(x, y)]).1 == bg {
                    x += 1;
                }
                if let Some(css) = css(bg) {
                    let (px, py) = self.cell_xy(start, y);
                    self.set_fill(&css);
                    self.ctx.fill_rect(px, py, (x - start) as f64 * cw, ch);
                }
            }
        }
        self.ctx.set_text_baseline("middle");
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = &buf[(x, y)];
                let sym = cell.symbol();
                if cell.skip || sym.is_empty() || sym == " " {
                    continue;
                }
                let fg = css(colors(cell).0).unwrap_or_else(|| DEFAULT_FG.into());
                self.set_fill(&fg);
                let (px, py) = self.cell_xy(x, y);
                let c = sym.chars().next().unwrap_or(' ');
                if let Some(mask) = QUADS.iter().position(|&q| q == c) {
                    self.quadrants(px, py, mask);
                } else if let Some(arms) = box_arms(c) {
                    self.box_glyph(px, py, arms);
                } else {
                    self.set_font(cell.modifier);
                    self.ctx.fill_text(sym, px, py + ch / 2.0)?;
                }
            }
        }
        Ok(())
    }

    /// Draw the framebuffer, upscaled without smoothing, over `rect` (cells).
    fn blit(&mut self, fb: &FrameBuf, rect: Rect) -> Result<(), JsValue> {
        if fb.w == 0 || fb.h == 0 {
            return Ok(());
        }
        if self.img.width() != fb.w as u32 || self.img.height() != fb.h as u32 {
            self.img.set_width(fb.w as u32);
            self.img.set_height(fb.h as u32);
        }
        self.rgba.clear();
        for y in 0..fb.h {
            for x in 0..fb.w {
                let p = fb.get(x, y);
                self.rgba.extend_from_slice(&[p.0, p.1, p.2, 255]);
            }
        }
        let data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&self.rgba), fb.w as u32, fb.h as u32)?;
        self.img_ctx.put_image_data(&data, 0.0, 0.0)?;
        let (px, py) = self.cell_xy(rect.x, rect.y);
        let (cw, ch) = self.cell;
        self.ctx.set_image_smoothing_enabled(false);
        self.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
            &self.img,
            px,
            py,
            rect.width as f64 * cw,
            rect.height as f64 * ch,
        )
    }

    /// Quadrant / half / full block glyphs as exact rectangles.
    fn quadrants(&self, x: f64, y: f64, mask: usize) {
        let (cw, ch) = self.cell;
        let (hx, hy) = ((cw / 2.0).round(), (ch / 2.0).round());
        let quads = [(x, y, hx, hy), (x + hx, y, cw - hx, hy), (x, y + hy, hx, ch - hy), (x + hx, y + hy, cw - hx, ch - hy)];
        for (i, &(qx, qy, qw, qh)) in quads.iter().enumerate() {
            if mask >> i & 1 == 1 {
                self.ctx.fill_rect(qx, qy, qw, qh);
            }
        }
    }

    /// Light box-drawing lines from the cell centre to its edges, so panel
    /// borders join up regardless of the font's line height.
    fn box_glyph(&self, x: f64, y: f64, a: Arms) {
        let (cw, ch) = self.cell;
        let lw = (cw / 9.0).round().max(1.0);
        let (cx, cy) = (x + ((cw - lw) / 2.0).floor(), y + ((ch - lw) / 2.0).floor());
        if a.round {
            // A quarter arc joining the two arms.
            let (mx, my) = (cx + lw / 2.0, cy + lw / 2.0);
            let ex = if a.right { x + cw } else { x };
            let ey = if a.down { y + ch } else { y };
            let ctx = &self.ctx;
            ctx.set_line_width(lw);
            ctx.begin_path();
            ctx.move_to(ex, my);
            let _ = ctx.arc_to(mx, my, mx, ey, (cw / 2.0).min(ch / 2.0));
            ctx.line_to(mx, ey);
            ctx.stroke();
            return;
        }
        if a.left || a.right {
            let x0 = if a.left { x } else { cx };
            let x1 = if a.right { x + cw } else { cx + lw };
            self.ctx.fill_rect(x0, cy, x1 - x0, lw);
        }
        if a.up || a.down {
            let y0 = if a.up { y } else { cy };
            let y1 = if a.down { y + ch } else { cy + lw };
            self.ctx.fill_rect(cx, y0, lw, y1 - y0);
        }
    }
}

#[derive(Clone, Copy)]
struct Arms {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    round: bool,
}

fn box_arms(c: char) -> Option<Arms> {
    let (up, down, left, right, round) = match c {
        '─' => (false, false, true, true, false),
        '│' => (true, true, false, false, false),
        '┌' => (false, true, false, true, false),
        '┐' => (false, true, true, false, false),
        '└' => (true, false, false, true, false),
        '┘' => (true, false, true, false, false),
        '╭' => (false, true, false, true, true),
        '╮' => (false, true, true, false, true),
        '╰' => (true, false, false, true, true),
        '╯' => (true, false, true, false, true),
        '├' => (true, true, false, true, false),
        '┤' => (true, true, true, false, false),
        '┬' => (false, true, true, true, false),
        '┴' => (true, false, true, true, false),
        '┼' => (true, true, true, true, false),
        _ => return None,
    };
    Some(Arms { up, down, left, right, round })
}

/// Foreground and background, with `REVERSED` applied.
fn colors(cell: &ratatui::buffer::Cell) -> (Color, Color) {
    if cell.modifier.contains(Modifier::REVERSED) {
        (cell.bg, cell.fg)
    } else {
        (cell.fg, cell.bg)
    }
}

/// CSS colour for a ratatui colour; `None` for the terminal default.
fn css(c: Color) -> Option<String> {
    let (r, g, b) = match c {
        Color::Reset => return None,
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => indexed(i),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::Gray => (191, 191, 191),
        Color::DarkGray => (102, 102, 102),
        Color::LightRed => (241, 76, 76),
        Color::LightGreen => (35, 209, 139),
        Color::LightYellow => (245, 245, 67),
        Color::LightBlue => (59, 142, 234),
        Color::LightMagenta => (214, 112, 214),
        Color::LightCyan => (41, 184, 219),
        Color::White => (255, 255, 255),
    };
    Some(format!("rgb({r},{g},{b})"))
}

/// The xterm 256-colour palette.
fn indexed(i: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (191, 191, 191),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    match i {
        0..=15 => BASE[i as usize],
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (level(i / 36), level((i / 6) % 6), level(i % 6))
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    }
}
