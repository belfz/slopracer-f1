//! Gamepad support via gilrs (DualShock 4, DualSense, Xbox and other
//! SDL-mapped controllers). Unlike a terminal keyboard, a pad always reports
//! real held state, so it gets full analog steering and pedal control.

use gilrs::{Axis, Button, EventType, Gamepad, Gilrs};

/// A discrete button press, for menus and race actions.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Press {
    Up,
    Down,
    Left,
    Right,
    /// Cross (A on Xbox).
    Confirm,
    /// Circle (B).
    Back,
    /// Triangle (Y).
    Restart,
    /// L1 / R1.
    Shift,
    /// Options (Start / Menu).
    Start,
}

/// Held driving inputs, merged across all connected pads.
#[derive(Clone, Copy, Default)]
pub struct Held {
    pub steer: f32,
    pub accel: bool,
    pub brake: bool,
}

/// Stick travel ignored around the centre, and the point where it counts as
/// a menu move (with hysteresis so one flick is one move).
const DEADZONE: f32 = 0.12;
const NAV_ON: f32 = 0.6;
const NAV_OFF: f32 = 0.35;
/// Analog trigger travel that counts as pressed.
const TRIGGER: f32 = 0.15;

pub struct Pad {
    gilrs: Option<Gilrs>,
    stick: Option<Press>,
}

impl Pad {
    pub fn new() -> Pad {
        Pad { gilrs: Gilrs::new().ok(), stick: None }
    }

    fn pads(&self) -> impl Iterator<Item = Gamepad<'_>> {
        self.gilrs.iter().flat_map(|g| g.gamepads().map(|(_, p)| p))
    }

    /// Name of the first connected controller. Browsers append vendor
    /// details in parentheses, which are dropped.
    pub fn name(&self) -> Option<String> {
        self.pads().next().map(|p| p.name().split(" (").next().unwrap_or_default().trim().to_string())
    }

    pub fn connected(&self) -> bool {
        self.pads().next().is_some()
    }

    /// Drain pending controller events, returning the presses since last call.
    pub fn poll(&mut self) -> Vec<Press> {
        let mut out = Vec::new();
        let Some(g) = self.gilrs.as_mut() else { return out };
        while let Some(ev) = g.next_event() {
            if let EventType::ButtonPressed(b, _) = ev.event {
                out.extend(press_for(b));
            }
        }
        // Treat the left stick like a D-pad for menus: one press per flick.
        let (x, y) = self.pads().fold((0.0f32, 0.0f32), |(x, y), p| {
            (x + p.value(Axis::LeftStickX), y + p.value(Axis::LeftStickY))
        });
        let dir = if x.abs().max(y.abs()) < NAV_OFF {
            None
        } else if x.abs() > y.abs() {
            Some(if x < 0.0 { Press::Left } else { Press::Right })
        } else {
            Some(if y > 0.0 { Press::Up } else { Press::Down })
        };
        if dir.is_none() || x.abs().max(y.abs()) >= NAV_ON {
            if dir.is_some() && dir != self.stick {
                out.extend(dir);
            }
            self.stick = dir;
        }
        out
    }

    pub fn held(&self) -> Held {
        let mut h = Held::default();
        for p in self.pads() {
            let x = p.value(Axis::LeftStickX);
            let stick = x.signum() * ((x.abs() - DEADZONE) / (1.0 - DEADZONE)).max(0.0);
            let dpad = p.is_pressed(Button::DPadRight) as i32 as f32 - p.is_pressed(Button::DPadLeft) as i32 as f32;
            h.steer += stick + dpad;
            h.accel |= trigger(&p, Button::RightTrigger2) || p.is_pressed(Button::South) || p.is_pressed(Button::DPadUp);
            h.brake |= trigger(&p, Button::LeftTrigger2) || p.is_pressed(Button::West) || p.is_pressed(Button::DPadDown);
        }
        h.steer = h.steer.clamp(-1.0, 1.0);
        h
    }
}

fn trigger(p: &Gamepad, b: Button) -> bool {
    p.is_pressed(b) || p.button_data(b).is_some_and(|d| d.value() > TRIGGER)
}

fn press_for(b: Button) -> Option<Press> {
    Some(match b {
        Button::DPadUp => Press::Up,
        Button::DPadDown => Press::Down,
        Button::DPadLeft => Press::Left,
        Button::DPadRight => Press::Right,
        Button::South => Press::Confirm,
        Button::East => Press::Back,
        Button::North => Press::Restart,
        Button::LeftTrigger | Button::RightTrigger => Press::Shift,
        Button::Start => Press::Start,
        _ => return None,
    })
}
