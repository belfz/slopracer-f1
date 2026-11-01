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
    CarColor,
    Gearbox,
    Laps,
    #[cfg(not(target_arch = "wasm32"))]
    Graphics,
    Start,
}

const MENU: &[MenuItem] = &[
    MenuItem::Track,
    MenuItem::CarColor,
    MenuItem::Gearbox,
    MenuItem::Laps,
    #[cfg(not(target_arch = "wasm32"))]
    MenuItem::Graphics,
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
        let mut demo = Race::new(tracks[track_idx].clone(), 99, true, true);
        demo.set_player_color(settings.car_color.rgb());
        App {
            tracks,
            screen: if intro { Screen::Boot } else { Screen::Menu },
            menu_item: MenuItem::Start,
            track_idx,
            auto_gear: true,
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

    pub fn visible_tracks(&self) -> Vec<usize> {
        (0..self.tracks.len()).filter(|&index| !self.tracks[index].classic).collect()
    }

    fn finish_boot(&mut self) {
        self.screen = Screen::Menu;
        self.keys.clear();
    }

    fn restart_demo(&mut self) {
        self.demo = Race::new(self.tracks[self.track_idx].clone(), 99, true, true);
        self.demo.set_player_color(self.settings.car_color.rgb());
    }

    fn start_race(&mut self) {
        self.audio.play(Sfx::Select);
        let mut race = Race::new(self.tracks[self.track_idx].clone(), self.laps, self.auto_gear, false);
        race.set_player_color(self.settings.car_color.rgb());
        self.race = Some(race);
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
        self.keys.event(k.key, k.kind, now);
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
                    MenuItem::CarColor => {
                        self.settings.car_color = self.settings.car_color.cycle(dir);
                        self.demo.set_player_color(self.settings.car_color.rgb());
                        self.settings.save();
                    }
                    MenuItem::Laps => self.laps = (self.laps as i32 + dir).clamp(1, 9) as u32,
                    #[cfg(not(target_arch = "wasm32"))]
                    MenuItem::Graphics => self.cycle_mode(dir),
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::settings::CarColor;

    #[test]
    fn track_picker_only_cycles_available_tracks() {
        let mut app = App::new(true, true, true, Mode::Pixels, true, false);
        assert_eq!(app.tracks.len(), 6);
        assert_eq!(app.visible_tracks().iter().map(|&index| app.tracks[index].name).collect::<Vec<_>>(), ["Neon Sakura", "Shirayuki Pass", "Tokyo Midnight"]);
        app.menu_item = MenuItem::Track;
        let first = app.track_idx;
        for key in [Key::Right, Key::Right, Key::Right, Key::Left, Key::Left, Key::Left] {
            let previous = app.track_idx;
            app.menu_key(key);
            assert_ne!(app.track_idx, previous);
            assert!(!app.tracks[app.track_idx].classic);
        }
        assert_eq!(app.track_idx, first);
    }

    #[test]
    fn chosen_color_survives_track_changes_and_race_restarts() {
        let mut app = App::new(true, true, true, Mode::Pixels, true, false);
        let mut sprites = Vec::new();
        for color in CarColor::ALL {
            app.settings.car_color = color;
            for track_index in app.visible_tracks() {
                app.track_idx = track_index;
                app.restart_demo();
                app.demo.sprites(&mut sprites);
                assert_eq!(sprites.last().unwrap().body, color.rgb());
                app.start_race();
                app.paused = true;
                app.race_key(Key::Char('r'));
                let race = app.race.as_ref().unwrap();
                assert_eq!(race.car_color, color.rgb());
                race.sprites(&mut sprites);
                assert_eq!(sprites.last().unwrap().body, color.rgb());
                app.back_to_menu();
                assert_eq!(app.demo.car_color, color.rgb());
            }
        }
    }

    #[test]
    fn automatic_gearbox_is_the_default_in_every_graphics_mode() {
        let mut app = App::new(true, true, true, Mode::Pixels, true, false);
        for mode in [Mode::Pixels, Mode::Quadrant, Mode::HalfBlock] {
            assert_eq!(app.mode, mode);
            assert!(app.auto_gear);
            app.start_race();
            assert!(app.race.as_ref().unwrap().auto_gear);
            app.cycle_mode(1);
        }
        app.menu_item = MenuItem::Gearbox;
        app.menu_key(Key::Right);
        app.start_race();
        assert!(!app.race.as_ref().unwrap().auto_gear);
    }
}
