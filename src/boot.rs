//! The power-on sequence: a "Crystal BIOS" self-test typed out in terminal
//! text, reporting real facts about the terminal before the menu appears.
//! Purely decorative; any key skips it.

use crate::audio::Sfx;

/// Length of the whole sequence, in seconds.
pub const TOTAL: f32 = 4.0;

pub struct Boot {
    pub t: f32,
    events: Vec<(f32, Sfx)>,
}

impl Boot {
    pub fn new() -> Boot {
        let mut events = Vec::new();
        for line in BIOS_SCRIPT {
            events.push((line.at, Sfx::Tick));
            if let Some((_, after)) = line.status {
                events.push((line.at + after, Sfx::Blip));
            }
        }
        let mut t = MEM_START;
        while t < MEM_END {
            events.push((t, Sfx::Tick));
            t += 0.045;
        }
        events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Boot { t: 0.0, events }
    }

    /// Advance the clock, emitting sounds as their moments pass. Returns
    /// true once the sequence is over.
    pub fn update(&mut self, dt: f32, sfx: &mut Vec<Sfx>) -> bool {
        let (t0, t1) = (self.t, self.t + dt);
        sfx.extend(self.events.iter().filter(|(at, _)| *at >= t0 && *at < t1).map(|&(_, s)| s));
        self.t = t1;
        self.t >= TOTAL
    }
}

// ---------------------------------------------------------------------------
// BIOS self-test (terminal text)
// ---------------------------------------------------------------------------

/// What the "BIOS" detects: real facts about the terminal and game setup.
pub struct BiosInfo {
    pub cols: u16,
    pub rows: u16,
    pub res: (usize, usize),
    pub mode: &'static str,
    pub truecolor: bool,
    pub enhanced: bool,
    pub audio: bool,
    /// Name of the connected gamepad, if any.
    pub pad: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Header,
    Dim,
    Label,
    Ok,
    Warn,
}

pub struct BiosLine {
    /// (text, tone) runs, already cut to what has been typed so far.
    pub runs: Vec<(String, Tone)>,
    pub typing: bool,
}

struct Script {
    at: f32,
    key: &'static str,
    /// Status shown in brackets, and when it appears after the line starts.
    status: Option<(&'static str, f32)>,
}

const MEM_START: f32 = 1.15;
const MEM_END: f32 = 1.95;
const MEM_KB: u32 = 4096;

const BIOS_SCRIPT: &[Script] = &[
    Script { at: 0.0, key: "header", status: None },
    Script { at: 0.08, key: "copyright", status: None },
    Script { at: 0.4, key: "cpu", status: Some(("OK", 0.28)) },
    Script { at: 0.75, key: "clock", status: Some(("LOCKED", 0.32)) },
    Script { at: MEM_START, key: "memory", status: Some(("OK", MEM_END - MEM_START + 0.05)) },
    Script { at: 2.1, key: "video", status: Some(("OK", 0.3)) },
    Script { at: 2.45, key: "audio", status: Some(("OK", 0.25)) },
    Script { at: 2.75, key: "input", status: Some(("OK", 0.25)) },
    Script { at: 3.1, key: "cart", status: Some(("VALID", 0.38)) },
    Script { at: 3.6, key: "boot", status: None },
];

const TYPE_SPEED: f32 = 140.0;

/// Coloured text runs of a line, plus an optional status overriding "OK".
type ScriptLine = (Vec<(String, Tone)>, Option<(&'static str, Tone)>);

fn script_text(key: &str, info: &BiosInfo, t: f32) -> ScriptLine {
    let l = |s: &str| (s.to_string(), Tone::Label);
    let v = |s: String| (s, Tone::Dim);
    match key {
        "header" => (vec![("SLOPRACER-16 CRYSTAL BIOS v2.86".into(), Tone::Header)], None),
        "copyright" => (vec![v("Copyright (C) 1993 Slopracer Systems Ltd. All rights reserved.".into())], None),
        "cpu" => (vec![l("CPU    "), v("FIREFLY 65C816X @ 21.477 MHz, 2 cores of pure enthusiasm".into())], None),
        "clock" => (vec![l("CLOCK  "), v("Crystal oscillator 21.477272 MHz".into())], Some(("LOCKED", Tone::Ok))),
        "memory" => {
            let k = ((t - MEM_START) / (MEM_END - MEM_START)).clamp(0.0, 1.0);
            let kb = ((k * MEM_KB as f32) as u32 / 64) * 64;
            (vec![l("MEMORY "), v(format!("{kb:>4}K"))], None)
        }
        "video" => {
            let colors = if info.truecolor { "16.7M colours" } else { "256 colours" };
            let text = format!("NEBULA PPU · {} · {}x{} px · {}x{} cells · {colors}", info.mode, info.res.0, info.res.1, info.cols, info.rows);
            (vec![l("VIDEO  "), v(text)], None)
        }
        "audio" => {
            let status = if info.audio { ("OK", Tone::Ok) } else { ("NO DEVICE", Tone::Warn) };
            (vec![l("AUDIO  "), v("8-voice wavetable synth".into())], Some(status))
        }
        "input" => {
            let (text, status) = if let Some(name) = &info.pad {
                (format!("gamepad: {name}, analog steering"), ("OK", Tone::Ok))
            } else if info.enhanced {
                let kbd = if cfg!(target_arch = "wasm32") { "browser keyboard" } else { "kitty keyboard protocol" };
                (format!("{kbd}, key release tracking"), ("OK", Tone::Ok))
            } else {
                ("legacy keyboard: auto-throttle engaged".into(), ("LIMITED", Tone::Warn))
            };
            (vec![l("INPUT  "), v(text)], Some(status))
        }
        "cart" => (vec![l("SLOT A "), v("\"Slopracer F1\" 4 Mbit ROM · checksum 7F3A".into())], None),
        "boot" => {
            let dots = ".".repeat(((t * 4.0) as usize) % 4);
            (vec![("Booting cartridge".into(), Tone::Header), (dots, Tone::Header)], None)
        }
        _ => (Vec::new(), None),
    }
}

/// The BIOS screen at local time `t`, typed out progressively.
pub fn bios_lines(t: f32, info: &BiosInfo) -> Vec<BiosLine> {
    let mut out = Vec::new();
    for (i, s) in BIOS_SCRIPT.iter().enumerate() {
        if t < s.at {
            break;
        }
        if i == 2 || i == 8 {
            out.push(BiosLine { runs: Vec::new(), typing: false });
        }
        let (runs, status_override) = script_text(s.key, info, t);
        let mut budget = ((t - s.at) * TYPE_SPEED) as usize;
        let total: usize = runs.iter().map(|(r, _)| r.chars().count()).sum();
        let typing = budget < total;
        let mut cut = Vec::new();
        for (r, tone) in runs {
            let n = r.chars().count().min(budget);
            budget -= n;
            cut.push((r.chars().take(n).collect::<String>(), tone));
        }
        if let Some((st, after)) = s.status {
            if t >= s.at + after {
                let (text, tone) = status_override.unwrap_or((st, Tone::Ok));
                cut.push(("  [ ".into(), Tone::Dim));
                cut.push((text.into(), tone));
                cut.push((" ]".into(), Tone::Dim));
            } else if !typing {
                let dots = ".".repeat(((t - s.at) * 12.0) as usize % 4);
                cut.push((format!(" {dots}"), Tone::Dim));
            }
        }
        out.push(BiosLine { runs: cut, typing });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bios_types_out_and_reports_status() {
        let info = BiosInfo { cols: 140, rows: 42, res: (384, 216), mode: "PIXELS", truecolor: true, enhanced: false, audio: true, pad: None };
        let early = bios_lines(0.5, &info);
        let late = bios_lines(TOTAL - 0.01, &info);
        assert!(early.len() < late.len());
        let text: String = late.iter().flat_map(|l| l.runs.iter().map(|r| r.0.clone())).collect();
        assert!(text.contains("4096K"), "{text}");
        assert!(text.contains("LIMITED"));
        assert!(text.contains("384x216"));
    }

    #[test]
    fn gamepad_lifts_the_input_limit() {
        let pad = Some("PS4 Controller".to_string());
        let info = BiosInfo { cols: 140, rows: 42, res: (384, 216), mode: "PIXELS", truecolor: true, enhanced: false, audio: true, pad };
        let text: String = bios_lines(TOTAL - 0.01, &info).iter().flat_map(|l| l.runs.iter().map(|r| r.0.clone())).collect();
        assert!(text.contains("gamepad: PS4 Controller"), "{text}");
        assert!(!text.contains("LIMITED"));
    }

    #[test]
    fn sounds_fire_once_each() {
        let mut boot = Boot::new();
        let mut sfx = Vec::new();
        while !boot.update(1.0 / 60.0, &mut sfx) {}
        assert_eq!(sfx.len(), boot.events.len());
    }
}
