//! Held-key tracking. Terminals with the kitty keyboard protocol report key
//! releases; everywhere else we infer "held" from the auto-repeat stream.

use std::time::Duration;
use web_time::Instant;

/// The keys the game cares about, independent of terminal or browser.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Tab,
    Other,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum KeyKind {
    Press,
    Repeat,
    Release,
}

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub key: Key,
    pub kind: KeyKind,
    pub ctrl: bool,
}

#[derive(Clone, Copy)]
pub enum Btn {
    Left,
    Right,
    Up,
    Down,
}

impl Btn {
    pub fn from_key(key: Key) -> Option<Btn> {
        match key {
            Key::Left | Key::Char('a') | Key::Char('A') => Some(Btn::Left),
            Key::Right | Key::Char('d') | Key::Char('D') => Some(Btn::Right),
            Key::Up | Key::Char('w') | Key::Char('W') => Some(Btn::Up),
            Key::Down | Key::Char('s') | Key::Char('S') => Some(Btn::Down),
            _ => None,
        }
    }
}

pub struct Keys {
    pub enhanced: bool,
    down: [bool; 4],
    until: [Option<Instant>; 4],
}

/// Without release events, a fresh press must bridge the OS key-repeat delay;
/// subsequent repeats arrive quickly, so a short window is enough.
const FIRST_PRESS: Duration = Duration::from_millis(320);
const REPEAT: Duration = Duration::from_millis(110);

impl Keys {
    pub fn new(enhanced: bool) -> Keys {
        Keys { enhanced, down: [false; 4], until: [None; 4] }
    }

    pub fn event(&mut self, b: Btn, kind: KeyKind, now: Instant) {
        let i = b as usize;
        if self.enhanced {
            self.down[i] = kind != KeyKind::Release;
        } else if kind != KeyKind::Release {
            let repeating = self.until[i].is_some_and(|u| now <= u + Duration::from_millis(60));
            self.until[i] = Some(now + if repeating { REPEAT } else { FIRST_PRESS });
        }
    }

    pub fn held(&self, b: Btn, now: Instant) -> bool {
        let i = b as usize;
        if self.enhanced {
            self.down[i]
        } else {
            self.until[i].is_some_and(|u| now <= u)
        }
    }

    pub fn clear(&mut self) {
        self.down = [false; 4];
        self.until = [None; 4];
    }
}
