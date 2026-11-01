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
    down: [u8; 4],
    until: [Option<Instant>; 4],
}

/// Without release events, a fresh press must bridge the OS key-repeat delay;
/// subsequent repeats arrive quickly, so a short window is enough.
const FIRST_PRESS: Duration = Duration::from_millis(320);
const REPEAT: Duration = Duration::from_millis(110);

impl Keys {
    pub fn new(enhanced: bool) -> Keys {
        Keys { enhanced, down: [0; 4], until: [None; 4] }
    }

    pub fn event(&mut self, key: Key, kind: KeyKind, now: Instant) {
        let Some(button) = Btn::from_key(key) else { return };
        let index = button as usize;
        if self.enhanced {
            let source = if matches!(key, Key::Char(_)) { 2 } else { 1 };
            if kind == KeyKind::Release {
                self.down[index] &= !source;
            } else {
                self.down[index] |= source;
            }
        } else if kind != KeyKind::Release {
            let repeating = self.until[index].is_some_and(|until| now <= until + Duration::from_millis(60));
            self.until[index] = Some(now + if repeating { REPEAT } else { FIRST_PRESS });
        }
    }

    pub fn held(&self, b: Btn, now: Instant) -> bool {
        let i = b as usize;
        if self.enhanced {
            self.down[i] != 0
        } else {
            self.until[i].is_some_and(|u| now <= u)
        }
    }

    pub fn clear(&mut self) {
        self.down = [0; 4];
        self.until = [None; 4];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_and_character_aliases_release_independently() {
        let now = Instant::now();
        for (arrow, character, button) in [
            (Key::Up, 'w', Btn::Up),
            (Key::Down, 's', Btn::Down),
            (Key::Left, 'a', Btn::Left),
            (Key::Right, 'd', Btn::Right),
        ] {
            for (first, second) in [(arrow, Key::Char(character)), (Key::Char(character), arrow)] {
                let mut keys = Keys::new(true);
                keys.event(first, KeyKind::Press, now);
                keys.event(second, KeyKind::Press, now);
                keys.event(second, KeyKind::Repeat, now);
                keys.event(second, KeyKind::Release, now);
                assert!(keys.held(button, now));
                keys.event(first, KeyKind::Release, now);
                assert!(!keys.held(button, now));
            }
        }
    }

    #[test]
    fn case_changes_and_focus_loss_do_not_leave_keys_held() {
        let now = Instant::now();
        let mut keys = Keys::new(true);
        keys.event(Key::Char('W'), KeyKind::Press, now);
        keys.event(Key::Char('w'), KeyKind::Repeat, now);
        keys.event(Key::Char('w'), KeyKind::Release, now);
        assert!(!keys.held(Btn::Up, now));
        keys.event(Key::Up, KeyKind::Press, now);
        keys.event(Key::Char('w'), KeyKind::Press, now);
        keys.clear();
        assert!(!keys.held(Btn::Up, now));
    }

    #[test]
    fn legacy_terminal_inputs_keep_their_repeat_deadlines() {
        let now = Instant::now();
        let mut keys = Keys::new(false);
        keys.event(Key::Up, KeyKind::Press, now);
        assert!(keys.held(Btn::Up, now + FIRST_PRESS));
        assert!(!keys.held(Btn::Up, now + FIRST_PRESS + Duration::from_millis(1)));
        let repeat = now + Duration::from_millis(300);
        keys.event(Key::Char('w'), KeyKind::Repeat, repeat);
        keys.event(Key::Char('w'), KeyKind::Release, repeat);
        assert!(keys.held(Btn::Up, repeat + REPEAT));
        assert!(!keys.held(Btn::Up, repeat + REPEAT + Duration::from_millis(1)));
        keys.clear();
        assert!(!keys.held(Btn::Up, repeat));
    }
}
