use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::song::{self, Chart, MAX_STEPS};

pub const MAX_FILE_BYTES: u64 = 1_000_000;

const BUNDLED: [(&str, &str); 3] = [
    ("Fur Elise", include_str!("../songs/Fur Elise.txt")),
    ("Ode to Joy", include_str!("../songs/Ode to Joy.txt")),
    (
        "Twinkle Twinkle Little Star",
        include_str!("../songs/Twinkle Twinkle Little Star.txt"),
    ),
];

pub struct Song {
    pub title: String,
    pub chart: Chart,
    /// Where an uploaded song is saved. Built-in songs have none.
    pub path: Option<PathBuf>,
}

pub struct Library {
    pub songs: Vec<Song>,
    dir: Option<PathBuf>,
}

impl Library {
    pub fn load() -> Self {
        let dir =
            ProjectDirs::from("", "", "the-last-key-bender").map(|d| d.data_dir().join("songs"));
        let mut library = Self {
            songs: BUNDLED
                .iter()
                .map(|&(title, sheet)| Song {
                    title: title.into(),
                    chart: song::parse(sheet),
                    path: None,
                })
                .collect(),
            dir,
        };

        let saved = library.dir.as_deref().and_then(|d| fs::read_dir(d).ok());
        for path in saved.into_iter().flatten().flatten().map(|e| e.path()) {
            if let (Some(title), Ok(chart)) = (title_of(&path), read_chart(&path)) {
                library.add(title, chart, path);
            }
        }
        library.songs.sort_by_key(|s| s.title.to_lowercase());
        library
    }

    /// Imports a `.mid` or `.txt` file, copies it to the user's song folder and returns its title.
    pub fn import(&mut self, path: &Path) -> Result<String, String> {
        let title = title_of(path).ok_or("File has no name")?;
        let chart = read_chart(path)?;

        let dir = self
            .dir
            .as_ref()
            .ok_or("Could not find a folder to save songs in")?;
        let extension = if is_midi(path) { "mid" } else { "txt" };
        let saved = dir.join(format!("{title}.{extension}"));
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::copy(path, &saved).map_err(|e| e.to_string())?;

        self.add(title.clone(), chart, saved);
        self.songs.sort_by_key(|s| s.title.to_lowercase());
        Ok(title)
    }

    /// Deletes an uploaded song's saved copy. The file it was uploaded from is left alone.
    pub fn remove(&mut self, title: &str) -> Result<(), String> {
        let index = self
            .songs
            .iter()
            .position(|s| s.title == title)
            .ok_or("Song not found")?;
        let path = self.songs[index]
            .path
            .as_ref()
            .ok_or("Built-in songs can't be removed")?;
        fs::remove_file(path).map_err(|e| e.to_string())?;
        self.songs.remove(index);
        Ok(())
    }

    fn add(&mut self, title: String, chart: Chart, path: PathBuf) {
        let path = Some(path);
        match self.songs.iter_mut().find(|s| s.title == title) {
            Some(song) => {
                song.chart = chart;
                song.path = path;
            }
            None => self.songs.push(Song { title, chart, path }),
        }
    }
}

fn read_chart(path: &Path) -> Result<Chart, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_FILE_BYTES {
        return Err(format!(
            "File is too large (max {} KB)",
            MAX_FILE_BYTES / 1000
        ));
    }

    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let chart = match path.extension().and_then(|e| e.to_str()) {
        _ if is_midi(path) => song::from_midi(&bytes)?,
        Some(e) if e.eq_ignore_ascii_case("txt") => {
            song::parse(&String::from_utf8(bytes).map_err(|_| "Sheet is not valid text")?)
        }
        _ => return Err("Only .mid, .midi and .txt files are supported".into()),
    };

    match chart.steps.len() {
        0 => Err("No playable notes found".into()),
        n if n > MAX_STEPS => Err(format!(
            "Song is too long (max {MAX_STEPS} notes and chords)"
        )),
        _ => Ok(chart),
    }
}

fn is_midi(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mid") || e.eq_ignore_ascii_case("midi"))
}

fn title_of(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_songs_are_playable() {
        for (title, sheet) in BUNDLED {
            assert!(!song::parse(sheet).steps.is_empty(), "{title}");
        }
    }
}
