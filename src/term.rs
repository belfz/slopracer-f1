//! Terminal front end: crossterm input, ratatui output, and kitty graphics
//! for the Pixels mode.

use crate::app::App;
use crate::input::{Key, KeyEvent, KeyKind};
use crate::{gfx, kitty, ui};
use ratatui::crossterm::{
    cursor,
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, stdout};
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_millis(16);

fn detect_truecolor() -> bool {
    let colorterm = std::env::var("COLORTERM").unwrap_or_default().to_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return true;
    }
    let prog = std::env::var("TERM_PROGRAM").unwrap_or_default();
    matches!(prog.as_str(), "iTerm.app" | "WezTerm" | "ghostty" | "vscode" | "Hyper")
}

fn restore_terminal(enhanced: bool, pixels: bool) {
    let mut out = stdout();
    if pixels {
        let _ = kitty::Kitty::new_live().clear(&mut out);
    }
    if enhanced {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(out, LeaveAlternateScreen, cursor::Show);
    let _ = terminal::disable_raw_mode();
}

pub fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("slopracer-f1 [--mute] [--no-intro] [--truecolor | --256] [--pixels | --quadrant | --halfblock]");
        println!("  Arrows/WASD steer, accelerate, brake · Space shift gear · P/Esc pause · M mute · V graphics");
        return Ok(());
    }
    let muted = args.iter().any(|a| a == "--mute");
    let has = |f: &str| args.iter().any(|a| a == f);
    let pixels_available = has("--pixels") || kitty::Kitty::detect();
    let mode = if has("--halfblock") {
        gfx::Mode::HalfBlock
    } else if has("--quadrant") || !pixels_available {
        gfx::Mode::Quadrant
    } else {
        gfx::Mode::Pixels
    };
    let truecolor = if args.iter().any(|a| a == "--256") {
        false
    } else {
        args.iter().any(|a| a == "--truecolor") || detect_truecolor()
    };

    terminal::enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, cursor::Hide)?;
    let enhanced = matches!(terminal::supports_keyboard_enhancement(), Ok(true));
    if enhanced {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal(enhanced, pixels_available);
        default_hook(info);
    }));

    let result = run(enhanced, truecolor, muted, mode, pixels_available, !has("--no-intro"));
    restore_terminal(enhanced, pixels_available);
    result
}

fn key_event(k: event::KeyEvent) -> KeyEvent {
    let key = match k.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        _ => Key::Other,
    };
    let kind = match k.kind {
        KeyEventKind::Press => KeyKind::Press,
        KeyEventKind::Repeat => KeyKind::Repeat,
        KeyEventKind::Release => KeyKind::Release,
    };
    KeyEvent { key, kind, ctrl: k.modifiers.contains(KeyModifiers::CONTROL) }
}

fn cell_size() -> Option<(f32, f32)> {
    let ws = terminal::window_size().ok()?;
    (ws.width > 0 && ws.height > 0 && ws.columns > 0 && ws.rows > 0)
        .then(|| (ws.width as f32 / ws.columns as f32, ws.height as f32 / ws.rows as f32))
}

fn run(enhanced: bool, truecolor: bool, muted: bool, mode: gfx::Mode, pixels_available: bool, intro: bool) -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    let mut app = App::new(enhanced, truecolor, muted, mode, pixels_available, intro);
    if let Some(c) = cell_size() {
        app.cell_px = c;
    }
    let mut kitty = kitty::Kitty::new();
    let mut fb = gfx::FrameBuf::new();
    let mut sprites = Vec::new();
    let mut last = Instant::now();

    while !app.quit {
        let frame_start = Instant::now();
        let dt = (frame_start - last).as_secs_f32().min(0.05);
        last = frame_start;

        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(k) => app.on_key(key_event(k), Instant::now()),
                Event::Resize(..) => {
                    if let Some(c) = cell_size() {
                        app.cell_px = c;
                    }
                    kitty.clear(&mut stdout())?;
                }
                _ => {}
            }
        }
        app.poll_pad();
        app.update(dt, Instant::now());
        let mut image = None;
        terminal.draw(|f| image = ui::draw(f, &app, &mut fb, &mut sprites))?;
        match image {
            Some((rect, scale)) => kitty.draw(&mut stdout(), &fb, rect, scale)?,
            None => kitty.clear(&mut stdout())?,
        }

        let elapsed = frame_start.elapsed();
        if elapsed < FRAME {
            // Sleep until the next frame, waking early for input.
            event::poll(FRAME - elapsed)?;
        }
    }
    kitty.clear(&mut stdout())
}
