//! Screen composition: menu, race HUD and overlays.

use crate::app::{App, MenuItem, Screen};
use crate::boot::{self, BiosInfo, Tone};
use crate::gfx::{draw_text_centered, FrameBuf, FrameView, Mode, Rgb};
use crate::race::{Gear, Race, GRID_SIZE};
use crate::render::{draw_start_lights, render_world, Camera, SpriteInst};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

const PANEL_BG: Color = Color::Rgb(12, 12, 32);
const ACCENT: Color = Color::Rgb(255, 200, 40);

pub fn fmt_time(t: f32) -> String {
    let cs = (t * 100.0).round() as u32;
    format!("{}:{:02}.{:02}", cs / 6000, (cs / 100) % 60, cs % 100)
}

fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn centered(area: Rect, w: u16, h: u16, y_frac: f32) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (((area.height - h) as f32) * y_frac) as u16;
    Rect::new(x, y, w, h)
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Line::from(format!(" {title} ")).bold().fg(ACCENT))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(PANEL_BG).fg(Color::White))
}

/// Framebuffer geometry for the current output mode.
struct Surface {
    w: usize,
    h: usize,
    /// Pixel width / pixel height.
    aspect: f32,
    /// Integer upscale applied before sending pixels to the terminal.
    scale: usize,
}

fn surface(app: &App, area: Rect) -> Surface {
    let (cw, ch) = app.cell_px;
    let (cols, rows) = (area.width as usize, area.height as usize);
    match app.mode {
        Mode::HalfBlock => Surface { w: cols, h: rows * 2, aspect: 2.0 * cw / ch, scale: 1 },
        Mode::Quadrant => Surface { w: cols * 2, h: rows * 2, aspect: cw / ch, scale: 1 },
        Mode::Pixels => {
            let (pw, ph) = (cols as f32 * cw, rows as f32 * ch);
            let ih = ph.clamp(16.0, 224.0);
            let iw = (ih * pw / ph).round().max(16.0);
            let scale = ((ph / ih).floor() as usize).clamp(1, 2);
            Surface { w: iw as usize, h: ih as usize, aspect: 1.0, scale }
        }
    }
}

/// Font scale for big text, keeping glyphs square on non-square pixels.
fn text_scale(fb: &FrameBuf, aspect: f32, rows_per_screen: f32) -> (i32, i32) {
    let sy = ((fb.h as f32 / rows_per_screen).round() as i32).max(1);
    let sx = ((sy as f32 / aspect).round() as i32).max(1);
    (sx, sy)
}

fn draw_scene(fb: &mut FrameBuf, race: &Race, sprites: &mut Vec<SpriteInst>, surf: &Surface, t: f32) {
    fb.resize(surf.w, surf.h);
    race.sprites(sprites);
    let cam = Camera { d: race.cam_d(), x: race.x, bg: race.bg };
    render_world(fb, &race.track, &cam, sprites, t, surf.aspect);
}

/// Draws the UI. Returns where (and at what scale) the framebuffer must be
/// sent as an image when running in `Mode::Pixels`.
pub fn draw(frame: &mut Frame, app: &App, fb: &mut FrameBuf, sprites: &mut Vec<SpriteInst>) -> Option<(Rect, usize)> {
    let area = frame.area();
    if area.width < 40 || area.height < 16 {
        let msg = Paragraph::new("Please enlarge the terminal (min 40x16)").alignment(Alignment::Center);
        frame.render_widget(msg, centered(area, area.width, 1, 0.5));
        return None;
    }
    match app.screen {
        Screen::Boot => {
            draw_boot(frame, app, area);
            None
        }
        Screen::Menu => draw_menu(frame, app, fb, sprites, area),
        Screen::Race => app.race.as_ref().and_then(|race| draw_race(frame, app, race, fb, sprites, area)),
    }
}

fn draw_boot(frame: &mut Frame, app: &App, area: Rect) {
    let surf = surface(app, area);
    draw_bios(frame, app, &surf, area, app.boot.t);
}

/// The self-test screen, in real terminal text with a phosphor palette.
fn draw_bios(frame: &mut Frame, app: &App, surf: &Surface, area: Rect, t: f32) {
    let info = BiosInfo {
        cols: area.width,
        rows: area.height,
        res: (surf.w, surf.h),
        mode: match app.mode {
            Mode::Pixels => "PIXELS",
            Mode::Quadrant => "QUADRANT",
            Mode::HalfBlock => "HALF-BLOCK",
        },
        truecolor: app.truecolor,
        enhanced: app.keys.enhanced,
        audio: app.audio.available(),
        pad: app.pad.name(),
    };
    let tone = |t: Tone| match t {
        Tone::Header => Style::default().fg(Color::Rgb(255, 196, 70)).bold(),
        Tone::Dim => Style::default().fg(Color::Rgb(196, 200, 214)),
        Tone::Label => Style::default().fg(Color::Rgb(120, 200, 255)).bold(),
        Tone::Ok => Style::default().fg(Color::Rgb(90, 255, 140)).bold(),
        Tone::Warn => Style::default().fg(Color::Rgb(255, 170, 60)).bold(),
    };
    let blink = ((t * 3.0) as i32) % 2 == 0;
    let mut lines: Vec<Line> = vec![Line::raw("")];
    let bios = boot::bios_lines(t, &info);
    let last = bios.len().saturating_sub(1);
    for (i, l) in bios.into_iter().enumerate() {
        let mut spans: Vec<Span> = vec![Span::raw("  ")];
        spans.extend(l.runs.into_iter().map(|(s, t)| Span::styled(s, tone(t))));
        if (l.typing || i == last) && blink {
            spans.push(Span::styled("█", tone(Tone::Dim)));
        }
        lines.push(Line::from(spans));
    }
    let bg = Style::default().bg(Color::Rgb(4, 4, 12));
    frame.render_widget(Paragraph::new(lines).style(bg), area);

    let hint = Paragraph::new(Line::styled("Press any key to skip", Style::default().fg(Color::Rgb(90, 90, 120))))
        .alignment(Alignment::Center)
        .style(bg);
    frame.render_widget(hint, Rect::new(area.x, area.y + area.height - 2, area.width, 1));
}

fn image_target(app: &App, area: Rect, surf: &Surface) -> Option<(Rect, usize)> {
    (app.mode == Mode::Pixels).then_some((area, surf.scale))
}

fn draw_menu(frame: &mut Frame, app: &App, fb: &mut FrameBuf, sprites: &mut Vec<SpriteInst>, area: Rect) -> Option<(Rect, usize)> {
    let surf = surface(app, area);
    draw_scene(fb, &app.demo, sprites, &surf, app.clock);
    let mut scale = text_scale(fb, surf.aspect, 24.0);
    while scale.0 > 1 && crate::gfx::text_width("PEGASUS F1", scale.0) > fb.w as i32 * 9 / 10 {
        scale = (scale.0 - 1, (scale.1 - 1).max(1));
    }
    draw_text_centered(fb, "PEGASUS F1", scale.1 * 2, scale, Rgb(255, 235, 80), Rgb(255, 90, 20), Some(Rgb(30, 10, 10)));
    frame.render_widget(FrameView { fb, truecolor: app.truecolor, mode: app.mode }, area);

    let track = &app.tracks[app.track_idx];
    let item = |m: MenuItem, label: &str, value: String| -> Line<'static> {
        let sel = app.menu_item == m;
        let marker = if sel { "▸ " } else { "  " };
        let style = if sel { Style::default().fg(ACCENT).add_modifier(Modifier::BOLD) } else { Style::default() };
        Line::from(vec![
            Span::styled(format!("{marker}{label:<9}"), style),
            Span::styled(format!("◀ {value} ▶"), style),
        ])
    };
    let start_sel = app.menu_item == MenuItem::Start;
    let start = Line::from(Span::styled(
        if start_sel { "»  START RACE  «" } else { "   START RACE   " },
        if start_sel {
            Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(ACCENT)
        },
    ))
    .alignment(Alignment::Center);
    let pad = app.pad.name();
    let input = if let Some(name) = &pad {
        Span::styled(format!("Input: gamepad · {name}"), Style::default().fg(Color::Green))
    } else if app.keys.enhanced {
        Span::styled("Input: full key tracking", Style::default().fg(Color::Green))
    } else {
        Span::styled("Input: basic (auto-throttle, hold ↓ to brake)", Style::default().fg(Color::Yellow))
    };
    let classics = match app.locked_tracks() {
        _ if app.settings.classic_tracks => "Unlocked".to_string(),
        1 => "Locked (1 track)".to_string(),
        n => format!("Locked ({n} tracks)"),
    };
    let lines = vec![
        Line::default(),
        item(MenuItem::Track, "Track", track.name.to_string()),
        Line::from(Span::styled(format!("             {}", track.desc), Style::default().fg(Color::Gray).italic())),
        item(MenuItem::Gearbox, "Gearbox", if app.auto_gear { "Auto".into() } else { "Manual (Space)".into() }),
        item(MenuItem::Laps, "Laps", app.laps.to_string()),
        Line::from(Span::styled("  ── Settings ──", Style::default().fg(Color::DarkGray))),
        item(MenuItem::Graphics, "Graphics", app.mode.label().to_string()),
        item(MenuItem::Classics, "Classics", classics),
        Line::default(),
        start,
        Line::default(),
        Line::from(input).alignment(Alignment::Center),
        Line::from(Span::styled(
            if pad.is_some() {
                "↑↓ select  ←→ change  ✕ start  V graphics  M mute  Q quit"
            } else {
                "↑↓ select  ←→ change  Enter start  V graphics  M mute  Q quit"
            },
            Style::default().fg(Color::DarkGray),
        ))
        .alignment(Alignment::Center),
    ];
    let rect = centered(area, 64, lines.len() as u16 + 2, 0.7);
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(panel("RACE SETUP")), rect);
    image_target(app, area, &surf)
}

fn draw_race(frame: &mut Frame, app: &App, race: &Race, fb: &mut FrameBuf, sprites: &mut Vec<SpriteInst>, area: Rect) -> Option<(Rect, usize)> {
    let [hud, game] = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(area);

    // --- HUD -------------------------------------------------------------
    let label = Style::default().fg(Color::Gray);
    let value = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    let sep = Span::styled("  │  ", Style::default().fg(Color::DarkGray));
    let lap = race.lap.min(race.laps);
    let mut spans = vec![
        Span::styled("LAP ", label),
        Span::styled(format!("{lap}/{}", race.laps), value),
        sep.clone(),
        Span::styled("TIME ", label),
        Span::styled(fmt_time(race.time), value),
        sep.clone(),
        Span::styled("BEST ", label),
        Span::styled(race.best_lap.map_or("-:--.--".into(), fmt_time), value),
        sep.clone(),
        Span::styled("POS ", label),
        Span::styled(format!("{}/{GRID_SIZE}", race.position()), value.fg(ACCENT)),
        sep.clone(),
        Span::styled(format!("{:>3}", race.kmh().round() as i32), value),
        Span::styled(" km/h", label),
        sep.clone(),
    ];
    let gear_style = |g: Gear| {
        if race.gear == g {
            Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };
    spans.push(Span::styled(" LO ", gear_style(Gear::Lo)));
    spans.push(Span::styled(" HI ", gear_style(Gear::Hi)));
    spans.push(Span::raw("  "));
    let segs = 12;
    let lit = ((race.rev.min(1.05) * segs as f32).round() as usize).min(segs);
    for i in 0..segs {
        let c = match i {
            0..=6 => Color::Green,
            7..=9 => Color::Yellow,
            _ => Color::Red,
        };
        let ch = if i < lit { "▮" } else { "▯" };
        spans.push(Span::styled(ch, Style::default().fg(if i < lit { c } else { Color::DarkGray })));
    }
    if race.auto_gear {
        spans.push(Span::styled("  AUTO", label));
    }
    if app.auto_throttle() {
        spans.push(Span::styled("  AUTO-THROTTLE", Style::default().fg(Color::Yellow)));
    }
    if app.muted {
        spans.push(Span::styled("  MUTED", label));
    }
    let title = format!("PEGASUS F1 · {}", race.track.name);
    frame.render_widget(Paragraph::new(Line::from(spans)).block(panel(&title)), hud);

    // --- World -----------------------------------------------------------
    let surf = surface(app, game);
    draw_scene(fb, race, sprites, &surf, app.clock);
    if let Some(n) = race.lights() {
        draw_start_lights(fb, n, surf.aspect);
    }
    let scale = text_scale(fb, surf.aspect, 30.0);
    if let Some((msg, _)) = &race.message {
        let (top, bottom) = match msg.as_str() {
            "GO!" => (Rgb(140, 255, 140), Rgb(20, 180, 40)),
            "CRASH!" => (Rgb(255, 180, 120), Rgb(230, 30, 20)),
            _ => (Rgb(255, 255, 255), Rgb(255, 200, 40)),
        };
        draw_text_centered(fb, msg, (fb.h as f32 * 0.24) as i32, scale, top, bottom, Some(Rgb(10, 10, 10)));
    }

    let finished = race.finished_for().is_some_and(|t| t > 2.5);
    if finished || app.paused {
        fb.dim(0.45);
    }
    if finished {
        let pos = race.final_pos;
        let headline = if pos == 1 { "WINNER!".to_string() } else { format!("P{pos}") };
        draw_text_centered(fb, &headline, (fb.h as f32 * 0.12) as i32, (scale.0 + scale.0 / 2, scale.1 + scale.1 / 2), Rgb(255, 240, 120), Rgb(255, 120, 20), Some(Rgb(0, 0, 0)));
    }
    frame.render_widget(FrameView { fb, truecolor: app.truecolor, mode: app.mode }, game);

    let pad = app.pad.connected();
    if finished {
        let row = |k: &str, v: String| Line::from(vec![Span::styled(format!("{k:<16}"), label), Span::styled(v, value)]);
        let lines = vec![
            Line::default(),
            row("Final position", format!("{} of {GRID_SIZE}", ordinal(race.final_pos))),
            row("Race time", fmt_time(race.time)),
            row("Best lap", race.best_lap.map_or("-".into(), fmt_time)),
            Line::default(),
            Line::from(Span::styled(
                if pad { "[✕] Menu    [△] Race again" } else { "[Enter] Menu    [R] Race again" },
                Style::default().fg(ACCENT),
            ))
            .alignment(Alignment::Center),
        ];
        let rect = centered(game, 40, lines.len() as u16 + 2, 0.62);
        frame.render_widget(Clear, rect);
        frame.render_widget(Paragraph::new(lines).block(panel("RACE RESULT")), rect);
    } else if app.paused {
        let lines = vec![
            Line::default(),
            Line::from(if pad {
                "[Options] Resume   [△] Restart   [○] Quit to menu"
            } else {
                "[Esc] Resume   [R] Restart   [Q] Quit to menu"
            })
            .alignment(Alignment::Center),
        ];
        let rect = centered(game, 50, 4, 0.5);
        frame.render_widget(Clear, rect);
        frame.render_widget(Paragraph::new(lines).block(panel("PAUSED")), rect);
    } else if race.lights().is_some() {
        let hint = if pad {
            "Hold R2/✕ to rev · stick steers · L2/□ brake · L1/R1 shift gear"
        } else if app.keys.enhanced {
            "Hold ↑ to rev · ←→ steer · ↓ brake · Space shift gear"
        } else {
            "←→ steer · hold ↓ to brake · Space shift gear"
        };
        let rect = Rect::new(game.x, game.y + game.height - 1, game.width, 1);
        frame.render_widget(Paragraph::new(hint).alignment(Alignment::Center).style(Style::default().fg(Color::White).bg(Color::Black)), rect);
    }
    image_target(app, game, &surf)
}
