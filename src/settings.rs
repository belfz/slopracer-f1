//! Persistent player settings, stored as `key=value` lines in
//! `$XDG_CONFIG_HOME/slopracer-f1/settings` (or `~/.config/...`), or in the
//! browser's localStorage.

use crate::gfx::Rgb;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum CarColor {
    #[default]
    Purple,
    Red,
    Blue,
    Green,
    Yellow,
}

impl CarColor {
    pub const ALL: [Self; 5] = [Self::Purple, Self::Red, Self::Blue, Self::Green, Self::Yellow];

    pub fn label(self) -> &'static str {
        match self {
            Self::Purple => "Purple",
            Self::Red => "Red",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Yellow => "Yellow",
        }
    }

    pub fn rgb(self) -> Rgb {
        match self {
            Self::Purple => Rgb(155, 60, 245),
            Self::Red => Rgb(225, 35, 45),
            Self::Blue => Rgb(35, 115, 245),
            Self::Green => Rgb(35, 195, 100),
            Self::Yellow => Rgb(255, 205, 35),
        }
    }

    pub fn cycle(self, direction: i32) -> Self {
        let index = Self::ALL.iter().position(|&color| color == self).unwrap() as i32;
        Self::ALL[(index + direction).rem_euclid(Self::ALL.len() as i32) as usize]
    }
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Settings {
    pub car_color: CarColor,
}

#[cfg(not(target_arch = "wasm32"))]
mod store {
    use std::path::PathBuf;

    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("slopracer-f1").join("settings"))
    }

    pub fn read() -> Option<String> {
        std::fs::read_to_string(path()?).ok()
    }

    pub fn write(text: &str) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, text);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod store {
    const KEY: &str = "slopracer-f1.settings";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn read() -> Option<String> {
        storage()?.get_item(KEY).ok()?
    }

    pub fn write(text: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(KEY, text);
        }
    }
}

impl Settings {
    fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "car_color" {
                    s.car_color = CarColor::ALL.into_iter().find(|color| color.label().eq_ignore_ascii_case(v.trim())).unwrap_or_default();
                }
            }
        }
        s
    }

    fn serialize(&self) -> String {
        format!("car_color={}\n", self.car_color.label())
    }

    pub fn load() -> Settings {
        store::read().map(|t| Settings::parse(&t)).unwrap_or_default()
    }

    /// Best effort: a read-only home just means settings don't stick.
    pub fn save(&self) {
        store::write(&self.serialize());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_junk() {
        for car_color in CarColor::ALL {
            let settings = Settings { car_color };
            assert_eq!(Settings::parse(&settings.serialize()), settings);
        }
        assert_eq!(Settings::parse("garbage\nclassic_tracks = true\n"), Settings::default());
        assert_eq!(Settings::parse("car_color=not-a-color\n"), Settings::default());
        assert_eq!(Settings::parse("car_color = blue\n"), Settings { car_color: CarColor::Blue });
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn five_distinct_colors_cycle_in_both_directions() {
        for (index, color) in CarColor::ALL.into_iter().enumerate() {
            assert_eq!(color.cycle(1).cycle(-1), color);
            assert_eq!(color.cycle(-1).cycle(1), color);
            assert_eq!(color.cycle(5), color);
            for other in &CarColor::ALL[..index] {
                assert_ne!(color.rgb(), other.rgb());
                assert_ne!(color.label(), other.label());
            }
        }
        assert_eq!(CarColor::Purple.cycle(-1), CarColor::Yellow);
        assert_eq!(CarColor::Yellow.cycle(1), CarColor::Purple);
    }
}
