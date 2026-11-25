use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::practice::Difficulty;

/// Choices remembered between runs. Missing or unreadable values fall back to the defaults.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub octave: i8,
    /// Speed and difficulty for each song, by title.
    pub songs: HashMap<String, SongSettings>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct SongSettings {
    pub speed: f32,
    pub difficulty: Difficulty,
}

impl Default for SongSettings {
    fn default() -> Self {
        Self {
            speed: 1.0,
            difficulty: Difficulty::Normal,
        }
    }
}

impl Settings {
    pub fn song(&self, title: &str) -> SongSettings {
        self.songs.get(title).copied().unwrap_or_default()
    }

    pub fn song_mut(&mut self, title: &str) -> &mut SongSettings {
        self.songs.entry(title.into()).or_default()
    }

    pub fn load() -> Self {
        path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, text);
        }
    }
}

fn path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "the-last-key-bender").map(|d| d.config_dir().join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_files_keep_defaults() {
        let text = r#"{"songs": {"Ode to Joy": {"speed": 0.5}}}"#;
        let settings: Settings = serde_json::from_str(text).unwrap();
        assert_eq!(settings.octave, 0);
        assert_eq!(settings.song("Ode to Joy").speed, 0.5);
        assert!(settings.song("Ode to Joy").difficulty == Difficulty::Normal);
        assert_eq!(settings.song("Fur Elise").speed, 1.0);
    }
}
