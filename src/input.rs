//! Held-key tracking. Terminals with the kitty keyboard protocol report key
//! releases; everywhere else we infer "held" from the auto-repeat stream.

use ratatui::crossterm::event::{KeyCode, KeyEventKind};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Btn {
    Left,
    Right,
    Up,
    Down,
}

impl Btn {
    pub fn from_code(code: KeyCode) -> Option<Btn> {
        match code {
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => Some(Btn::Left),
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => Some(Btn::Right),
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => Some(Btn::Up),
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => Some(Btn::Down),
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

    pub fn event(&mut self, b: Btn, kind: KeyEventKind, now: Instant) {
        let i = b as usize;
        if self.enhanced {
            self.down[i] = kind != KeyEventKind::Release;
        } else if kind != KeyEventKind::Release {
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
