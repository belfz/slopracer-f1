//! Persistent player settings, stored as `key=value` lines in
//! `$XDG_CONFIG_HOME/slopracer-f1/settings` (or `~/.config/...`), or in the
//! browser's localStorage.

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Settings {
    /// Show the original (non-Japanese) tracks in the track list.
    pub classic_tracks: bool,
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
                if k.trim() == "classic_tracks" {
                    s.classic_tracks = v.trim() == "true";
                }
            }
        }
        s
    }

    fn serialize(&self) -> String {
        format!("classic_tracks={}\n", self.classic_tracks)
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
        let s = Settings { classic_tracks: true };
        assert_eq!(Settings::parse(&s.serialize()), s);
        assert_eq!(Settings::parse("garbage\nclassic_tracks = false\n"), Settings::default());
        assert_eq!(Settings::parse(""), Settings::default());
    }
}
