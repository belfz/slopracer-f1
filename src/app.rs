//! Application state machine: menu <-> race, input dispatch, audio routing.

use crate::audio::{Audio, Sfx};
use crate::boot::Boot;
use crate::gfx::Mode;
use crate::input::{Btn, Key, KeyEvent, KeyKind, Keys};
use crate::pad::{Pad, Press};
use crate::race::{Controls, Race};
use crate::settings::Settings;
use crate::track::{all_tracks, Track};
use std::rc::Rc;
use web_time::Instant;

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
    #[cfg(not(target_arch = "wasm32"))]
    Graphics,
    Classics,
    Start,
}

const MENU: &[MenuItem] = &[
    MenuItem::Track,
    MenuItem::Gearbox,
    MenuItem::Laps,
    #[cfg(not(target_arch = "wasm32"))]
    MenuItem::Graphics,
    MenuItem::Classics,
    MenuItem::Start,
];

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
    pub pad: Pad,
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
    pub settings: Settings,
}

impl App {
    pub fn new(enhanced: bool, truecolor: bool, muted: bool, mode: Mode, pixels_available: bool, intro: bool) -> App {
        let tracks: Vec<Rc<Track>> = all_tracks().into_iter().map(Rc::new).collect();
        let settings = Settings::load();
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
            pad: Pad::new(),
            audio: Audio::new(muted),
            muted,
            truecolor,
            mode,
            pixels_available,
            cell_px: (9.0, 18.0),
            clock: 0.0,
            quit: false,
            boot: Boot::new(),
            settings,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn cycle_mode(&mut self, dir: i32) {
        let modes: &[Mode] = if self.pixels_available {
            &[Mode::Pixels, Mode::Quadrant, Mode::HalfBlock]
        } else {
            &[Mode::Quadrant, Mode::HalfBlock]
        };
        let i = modes.iter().position(|&m| m == self.mode).unwrap_or(0) as i32;
        self.mode = modes[(i + dir).rem_euclid(modes.len() as i32) as usize];
    }

    /// Indices of the tracks shown in the menu: classic tracks stay hidden
    /// until unlocked in the settings.
    pub fn visible_tracks(&self) -> Vec<usize> {
        (0..self.tracks.len()).filter(|&i| self.settings.classic_tracks || !self.tracks[i].classic).collect()
    }

    pub fn locked_tracks(&self) -> usize {
        self.tracks.len() - self.visible_tracks().len()
    }

    fn toggle_classics(&mut self) {
        self.settings.classic_tracks = !self.settings.classic_tracks;
        self.settings.save();
        let visible = self.visible_tracks();
        if !visible.contains(&self.track_idx) {
            self.track_idx = visible[0];
            self.restart_demo();
        }
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
        if let Some(b) = Btn::from_key(k.key) {
            self.keys.event(b, k.kind, now);
        }
        if k.kind == KeyKind::Release {
            return;
        }
        if k.ctrl && k.key == Key::Char('c') {
            self.quit = true;
            return;
        }
        if let Key::Char('m' | 'M') = k.key {
            if k.kind == KeyKind::Press {
                self.muted = self.audio.toggle_mute();
            }
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Key::Char('v' | 'V') = k.key {
            if k.kind == KeyKind::Press {
                self.cycle_mode(1);
            }
            return;
        }
        match self.screen {
            Screen::Boot => {
                if k.kind == KeyKind::Press {
                    self.finish_boot();
                }
            }
            Screen::Menu => self.menu_key(k.key),
            Screen::Race if k.kind == KeyKind::Press => self.race_key(k.key),
            Screen::Race => {}
        }
    }

    /// The window lost focus (browser tab switch): forget held keys and
    /// pause a race in progress.
    pub fn focus_lost(&mut self) {
        self.keys.clear();
        let racing = self.race.as_ref().is_some_and(|r| r.finished_for().is_none());
        if self.screen == Screen::Race && racing {
            self.paused = true;
        }
    }

    /// Without key-release events holding a pedal is unreliable, so throttle
    /// is automatic, unless a gamepad is there to hold it.
    pub fn auto_throttle(&self) -> bool {
        !self.keys.enhanced && !self.pad.connected()
    }

    /// Route controller button presses through the same menu/race actions as
    /// their keyboard equivalents.
    pub fn poll_pad(&mut self) {
        for p in self.pad.poll() {
            let code = match (p, self.screen) {
                (_, Screen::Boot) => {
                    self.finish_boot();
                    continue;
                }
                (Press::Up, _) => Key::Up,
                (Press::Down, _) => Key::Down,
                (Press::Left, _) => Key::Left,
                (Press::Right, _) => Key::Right,
                (Press::Confirm, _) | (Press::Start, Screen::Menu) => Key::Enter,
                (Press::Start, _) => Key::Esc,
                // Circle must not quit the game from the menu.
                (Press::Back, Screen::Menu) => continue,
                (Press::Back, _) => Key::Char('q'),
                (Press::Restart, _) => Key::Char('r'),
                (Press::Shift, _) => Key::Char('g'),
            };
            match self.screen {
                Screen::Menu => self.menu_key(code),
                _ => self.race_key(code),
            }
        }
    }

    fn menu_key(&mut self, code: Key) {
        let idx = MENU.iter().position(|&m| m == self.menu_item).unwrap_or(0);
        match code {
            Key::Up | Key::Char('w' | 'k') => {
                self.menu_item = MENU[(idx + MENU.len() - 1) % MENU.len()];
                self.audio.play(Sfx::Blip);
            }
            Key::Down | Key::Char('s' | 'j') | Key::Tab => {
                self.menu_item = MENU[(idx + 1) % MENU.len()];
                self.audio.play(Sfx::Blip);
            }
            Key::Left | Key::Right | Key::Char('a' | 'd') => {
                let dir: i32 = if matches!(code, Key::Left | Key::Char('a')) { -1 } else { 1 };
                match self.menu_item {
                    MenuItem::Track => {
                        let visible = self.visible_tracks();
                        let n = visible.len() as i32;
                        let at = visible.iter().position(|&i| i == self.track_idx).unwrap_or(0) as i32;
                        self.track_idx = visible[(at + dir).rem_euclid(n) as usize];
                        self.restart_demo();
                    }
                    MenuItem::Gearbox => self.auto_gear = !self.auto_gear,
                    MenuItem::Laps => self.laps = (self.laps as i32 + dir).clamp(1, 9) as u32,
                    #[cfg(not(target_arch = "wasm32"))]
                    MenuItem::Graphics => self.cycle_mode(dir),
                    MenuItem::Classics => self.toggle_classics(),
                    MenuItem::Start => return,
                }
                self.audio.play(Sfx::Blip);
            }
            Key::Enter | Key::Char(' ') => self.start_race(),
            Key::Esc | Key::Char('q' | 'Q') => self.quit = true,
            _ => {}
        }
    }

    fn race_key(&mut self, code: Key) {
        let Some(race) = self.race.as_mut() else { return };
        let results = race.finished_for().is_some_and(|t| t > 2.5);
        if results {
            match code {
                Key::Enter | Key::Esc => self.back_to_menu(),
                Key::Char('r' | 'R') => self.start_race(),
                _ => {}
            }
            return;
        }
        if self.paused {
            match code {
                Key::Esc | Key::Char('p' | 'P') => self.paused = false,
                Key::Char('r' | 'R') => self.start_race(),
                Key::Char('q' | 'Q') => self.back_to_menu(),
                _ => {}
            }
            return;
        }
        match code {
            Key::Esc | Key::Char('p' | 'P') => {
                self.paused = true;
                self.keys.clear();
            }
            Key::Char(' ') | Key::Char('g' | 'G') => race.shift(),
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
                let (k, pad) = (&self.keys, self.pad.held());
                let steer = k.held(Btn::Right, now) as i32 as f32 - k.held(Btn::Left, now) as i32 as f32;
                let steer = (steer + pad.steer).clamp(-1.0, 1.0);
                let brake = k.held(Btn::Down, now) || pad.brake;
                let accel = pad.accel
                    || if k.enhanced {
                        k.held(Btn::Up, now)
                    } else {
                        !self.pad.connected() && !brake
                    };
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
