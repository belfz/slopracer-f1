//! Application state machine: menu <-> race, input dispatch, audio routing.

use crate::audio::{Audio, Sfx};
use crate::boot::Boot;
use crate::gfx::Mode;
use crate::input::{Btn, Keys};
use crate::race::{Controls, Race};
use crate::track::{all_tracks, Track};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::rc::Rc;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Boot,
    Menu,
    Race,
}

#[derive(Clone, Copy, PartialEq)]
pub enum MenuItem {
    Track,
    Gearbox,
    Laps,
    Graphics,
    Start,
}

const MENU: [MenuItem; 5] = [MenuItem::Track, MenuItem::Gearbox, MenuItem::Laps, MenuItem::Graphics, MenuItem::Start];

pub struct App {
    pub tracks: Vec<Rc<Track>>,
    pub screen: Screen,
    pub menu_item: MenuItem,
    pub track_idx: usize,
    pub auto_gear: bool,
    pub laps: u32,
    pub demo: Race,
    pub race: Option<Race>,
    pub paused: bool,
    pub keys: Keys,
    pub audio: Audio,
    pub muted: bool,
    pub truecolor: bool,
    pub mode: Mode,
    pub pixels_available: bool,
    /// Size of one terminal cell in pixels (width, height).
    pub cell_px: (f32, f32),
    pub clock: f32,
    pub quit: bool,
    pub boot: Boot,
}

impl App {
    pub fn new(enhanced: bool, truecolor: bool, muted: bool, mode: Mode, pixels_available: bool, intro: bool) -> App {
        let tracks: Vec<Rc<Track>> = all_tracks().into_iter().map(Rc::new).collect();
        // Open on the synthwave track; it's the showpiece.
        let track_idx = tracks.iter().position(|t| t.name == "Neon Sakura").unwrap_or(0);
        let demo = Race::new(tracks[track_idx].clone(), 99, true, true);
        App {
            tracks,
            screen: if intro { Screen::Boot } else { Screen::Menu },
            menu_item: MenuItem::Start,
            track_idx,
            auto_gear: false,
            laps: 3,
            demo,
            race: None,
            paused: false,
            keys: Keys::new(enhanced),
            audio: Audio::new(muted),
            muted,
            truecolor,
            mode,
            pixels_available,
            cell_px: (9.0, 18.0),
            clock: 0.0,
            quit: false,
            boot: Boot::new(),
        }
    }

    pub fn cycle_mode(&mut self, dir: i32) {
        let modes: &[Mode] = if self.pixels_available {
            &[Mode::Pixels, Mode::Quadrant, Mode::HalfBlock]
        } else {
            &[Mode::Quadrant, Mode::HalfBlock]
        };
        let i = modes.iter().position(|&m| m == self.mode).unwrap_or(0) as i32;
        self.mode = modes[(i + dir).rem_euclid(modes.len() as i32) as usize];
    }

    fn finish_boot(&mut self) {
        self.screen = Screen::Menu;
        self.keys.clear();
    }

    fn restart_demo(&mut self) {
        self.demo = Race::new(self.tracks[self.track_idx].clone(), 99, true, true);
    }

    fn start_race(&mut self) {
        self.audio.play(Sfx::Select);
        self.race = Some(Race::new(self.tracks[self.track_idx].clone(), self.laps, self.auto_gear, false));
        self.screen = Screen::Race;
        self.paused = false;
        self.keys.clear();
    }

    fn back_to_menu(&mut self) {
        self.race = None;
        self.screen = Screen::Menu;
        self.paused = false;
        self.restart_demo();
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        if let Some(b) = Btn::from_code(k.code) {
            self.keys.event(b, k.kind, now);
        }
        if k.kind == KeyEventKind::Release {
            return;
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if let KeyCode::Char('m' | 'M') = k.code {
            if k.kind == KeyEventKind::Press {
                self.muted = self.audio.toggle_mute();
            }
            return;
        }
        if let KeyCode::Char('v' | 'V') = k.code {
            if k.kind == KeyEventKind::Press {
                self.cycle_mode(1);
            }
            return;
        }
        match self.screen {
            Screen::Boot => {
                if k.kind == KeyEventKind::Press {
                    self.finish_boot();
                }
            }
            Screen::Menu => self.menu_key(k.code),
            Screen::Race if k.kind == KeyEventKind::Press => self.race_key(k.code),
            Screen::Race => {}
        }
    }

    fn menu_key(&mut self, code: KeyCode) {
        let idx = MENU.iter().position(|&m| m == self.menu_item).unwrap_or(0);
        match code {
            KeyCode::Up | KeyCode::Char('w' | 'k') => {
                self.menu_item = MENU[(idx + MENU.len() - 1) % MENU.len()];
                self.audio.play(Sfx::Blip);
            }
            KeyCode::Down | KeyCode::Char('s' | 'j') | KeyCode::Tab => {
                self.menu_item = MENU[(idx + 1) % MENU.len()];
                self.audio.play(Sfx::Blip);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('a' | 'd') => {
                let dir: i32 = if matches!(code, KeyCode::Left | KeyCode::Char('a')) { -1 } else { 1 };
                match self.menu_item {
                    MenuItem::Track => {
                        let n = self.tracks.len() as i32;
                        self.track_idx = ((self.track_idx as i32 + dir).rem_euclid(n)) as usize;
                        self.restart_demo();
                    }
                    MenuItem::Gearbox => self.auto_gear = !self.auto_gear,
                    MenuItem::Laps => self.laps = (self.laps as i32 + dir).clamp(1, 9) as u32,
                    MenuItem::Graphics => self.cycle_mode(dir),
                    MenuItem::Start => return,
                }
                self.audio.play(Sfx::Blip);
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.start_race(),
            KeyCode::Esc | KeyCode::Char('q' | 'Q') => self.quit = true,
            _ => {}
        }
    }

    fn race_key(&mut self, code: KeyCode) {
        let Some(race) = self.race.as_mut() else { return };
        let results = race.finished_for().is_some_and(|t| t > 2.5);
        if results {
            match code {
                KeyCode::Enter | KeyCode::Esc => self.back_to_menu(),
                KeyCode::Char('r' | 'R') => self.start_race(),
                _ => {}
            }
            return;
        }
        if self.paused {
            match code {
                KeyCode::Esc | KeyCode::Char('p' | 'P') => self.paused = false,
                KeyCode::Char('r' | 'R') => self.start_race(),
                KeyCode::Char('q' | 'Q') => self.back_to_menu(),
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Esc | KeyCode::Char('p' | 'P') => {
                self.paused = true;
                self.keys.clear();
            }
            KeyCode::Char(' ') | KeyCode::Char('g' | 'G') => race.shift(),
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32, now: Instant) {
        self.clock += dt;
        match self.screen {
            Screen::Boot => {
                self.audio.set_engine(0.0, false, false);
                let mut sfx = Vec::new();
                let done = self.boot.update(dt, &mut sfx);
                for s in sfx {
                    self.audio.play(s);
                }
                if done {
                    self.finish_boot();
                }
            }
            Screen::Menu => {
                self.demo.update(dt, Controls::default());
                self.demo.sfx.clear();
                if self.demo.d > self.demo.track.length * 30.0 {
                    self.restart_demo();
                }
                self.audio.set_engine(0.0, false, false);
                // Preview the selected track's soundtrack, softly.
                self.audio.set_music(self.tracks[self.track_idx].music, 0.45);
            }
            Screen::Race => {
                let Some(race) = self.race.as_mut() else { return };
                if self.paused {
                    self.audio.set_engine(0.0, false, false);
                    self.audio.set_music(race.track.music, 0.25);
                    return;
                }
                self.audio.set_music(race.track.music, 1.0);
                let k = &self.keys;
                let steer = k.held(Btn::Right, now) as i32 as f32 - k.held(Btn::Left, now) as i32 as f32;
                let brake = k.held(Btn::Down, now);
                // Without key-release events holding a pedal is unreliable, so throttle is automatic.
                let accel = if k.enhanced { k.held(Btn::Up, now) } else { !brake };
                race.update(dt, Controls { steer, accel, brake });
                for s in race.sfx.drain(..) {
                    self.audio.play(s);
                }
                let engine_on = race.finished_for().is_none_or(|t| t < 6.0);
                self.audio.set_engine(race.rev, race.throttle, engine_on);
            }
        }
    }
}
