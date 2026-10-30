//! Persistent player settings, stored as `key=value` lines in
//! `$XDG_CONFIG_HOME/pegasus-f1/settings` (or `~/.config/...`).

use std::path::PathBuf;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Settings {
    /// Show the original (non-Japanese) tracks in the track list.
    pub classic_tracks: bool,
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("pegasus-f1").join("settings"))
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
        path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Settings::parse(&t)).unwrap_or_default()
    }

    /// Best effort: a read-only home just means settings don't stick.
    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.serialize());
        }
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
