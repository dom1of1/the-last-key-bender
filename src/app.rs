use std::collections::HashMap;
use std::path::Path;

use eframe::egui::{self, Event, Key};

use crate::audio::Audio;
use crate::library::Library;
use crate::piano_view::{self, KeyState};
use crate::practice::{Difficulty, Practice, Press};
use crate::{keymap, lane};

const SPEEDS: [f32; 3] = [0.5, 0.75, 1.0];

enum Action {
    TogglePlay,
    Restart,
    Speed(f32),
    Difficulty(Difficulty),
}

pub struct PianoApp {
    audio: Result<Audio, String>,
    // The note each key started, and whether letting go should stop it. Remembering the note
    // means releasing Shift first still stops the right one; song notes stop on their own.
    held: HashMap<Key, (u8, bool)>,
    mouse_note: Option<(u8, bool)>,
    library: Library,
    search: String,
    practice: Option<Practice>,
    status: Option<Result<String, String>>,
}

impl PianoApp {
    pub fn new() -> Self {
        Self {
            audio: Audio::new(),
            held: HashMap::new(),
            mouse_note: None,
            library: Library::load(),
            search: String::new(),
            practice: None,
            status: None,
        }
    }

    fn handle_key(&mut self, key: Key, pressed: bool, shift: bool) {
        if key == Key::Space {
            if pressed && let Some(practice) = &self.practice {
                self.set_playing(!practice.is_playing());
            }
        } else if pressed {
            if let Some(note) = self.note_for(key, shift) {
                let stop_on_release = self.press(note);
                self.held.insert(key, (note, stop_on_release));
            }
        } else if let Some((note, true)) = self.held.remove(&key) {
            self.release(note);
        }
    }

    /// While a song is playing, a key plays whichever of its two notes (with or without Shift)
    /// the song expects, so black keys don't depend on Shift timing.
    fn note_for(&self, key: Key, shift: bool) -> Option<u8> {
        let wanted = keymap::note_for(key, shift);
        let Some(practice) = self.practice.as_ref().filter(|p| p.is_playing()) else {
            return wanted;
        };
        [wanted, keymap::note_for(key, !shift)]
            .into_iter()
            .flatten()
            .find(|&n| practice.expects(n))
            .or(wanted)
    }

    fn set_mouse_note(&mut self, note: Option<u8>) {
        if note == self.mouse_note.map(|(n, _)| n) {
            return;
        }
        if let Some((old, true)) = self.mouse_note {
            self.release(old);
        }
        self.mouse_note = note.map(|n| (n, self.press(n)));
    }

    /// Starts a note and returns whether releasing the key should stop it. A hit melody note
    /// rings for its written length however briefly the key is tapped.
    fn press(&mut self, note: u8) -> bool {
        let press = match &mut self.practice {
            Some(practice) if practice.is_playing() => practice.press(note),
            _ => Press::Miss,
        };
        let Ok(audio) = &self.audio else {
            return true;
        };
        match press {
            Press::Hit(n) => {
                audio.schedule(n.delay, n.key, n.length);
                false
            }
            Press::Miss => {
                audio.note_on(note);
                true
            }
        }
    }

    /// Starts or stops the song clock, rescheduling the accompaniment to match.
    fn set_playing(&mut self, play: bool) {
        let Some(practice) = &mut self.practice else {
            return;
        };
        let backing = if play && !practice.is_done() {
            practice.play()
        } else {
            practice.pause();
            Vec::new()
        };
        if let Ok(audio) = &self.audio {
            audio.cancel_scheduled();
            for n in backing {
                audio.schedule(n.delay, n.key, n.length);
            }
        }
    }

    fn release(&self, note: u8) {
        if let Ok(audio) = &self.audio {
            audio.note_off(note);
        }
    }

    fn key_state(&self, note: u8, sounding: &[bool; 128]) -> KeyState {
        let down = self
            .held
            .values()
            .chain(&self.mouse_note)
            .any(|&(n, _)| n == note);
        if down {
            KeyState::Down
        } else if sounding[usize::from(note)] {
            KeyState::Sounding
        } else {
            KeyState::Up
        }
    }

    fn open(&mut self, title: &str) {
        if let Some(song) = self.library.songs.iter().find(|s| s.title == title) {
            self.practice = Some(Practice::new(song.title.clone(), song.chart.clone()));
            self.set_playing(false);
        }
    }

    fn remove(&mut self, title: &str) {
        let result = self.library.remove(title);
        if result.is_ok() && self.practice.as_ref().is_some_and(|p| p.title == title) {
            self.practice = None;
        }
        self.status = Some(result.map(|()| format!("Removed \"{title}\"")));
    }

    fn import(&mut self, path: &Path) {
        let result = self.library.import(path);
        if let Ok(title) = &result {
            self.open(title);
        }
        self.status = Some(result.map(|title| format!("Added \"{title}\"")));
    }

    fn library_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Songs");
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search"));
        ui.add_space(4.0);

        let mut chosen = None;
        let mut removed = None;
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 90.0)
            .show(ui, |ui| {
                if ui
                    .selectable_label(self.practice.is_none(), "Free play")
                    .clicked()
                {
                    self.practice = None;
                }
                let query = self.search.to_lowercase();
                for song in &self.library.songs {
                    if !song.title.to_lowercase().contains(&query) {
                        continue;
                    }
                    let open = self
                        .practice
                        .as_ref()
                        .is_some_and(|p| p.title == song.title);
                    ui.horizontal(|ui| {
                        if song.path.is_some()
                            && bin_button(ui).on_hover_text("Remove this song").clicked()
                        {
                            removed = Some(song.title.clone());
                        }
                        if ui.selectable_label(open, &song.title).clicked() {
                            chosen = Some(song.title.clone());
                        }
                    });
                }
            });
        if let Some(title) = chosen {
            self.open(&title);
        }
        if let Some(title) = removed {
            self.remove(&title);
        }

        ui.separator();
        if ui.button("Upload song…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Songs", &["mid", "midi", "txt"])
                .pick_file()
        {
            self.import(&path);
        }
        ui.weak("or drop a .mid or .txt file here");
        match &self.status {
            Some(Ok(message)) => {
                ui.label(message);
            }
            Some(Err(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            None => {}
        }
    }

    fn sheet(&mut self, ui: &mut egui::Ui) {
        let Some(practice) = &self.practice else {
            ui.weak("Pick a song from the list, or just play.");
            return;
        };

        let mut action = None;
        ui.horizontal(|ui| {
            ui.heading(&practice.title);
            let label = if practice.is_playing() {
                "Pause"
            } else {
                "Play"
            };
            if ui.button(label).on_hover_text("Space").clicked() {
                action = Some(Action::TogglePlay);
            }
            if ui.button("Restart").clicked() {
                action = Some(Action::Restart);
            }
            ui.separator();
            for speed in SPEEDS {
                let label = format!("{:.0}%", speed * 100.0);
                if ui
                    .selectable_label(practice.speed() == speed, label)
                    .clicked()
                {
                    action = Some(Action::Speed(speed));
                }
            }
            ui.separator();
            for difficulty in Difficulty::ALL {
                let selected = practice.difficulty() == difficulty;
                if ui.selectable_label(selected, difficulty.name()).clicked() {
                    action = Some(Action::Difficulty(difficulty));
                }
            }
            ui.separator();
            ui.label(format!(
                "Perfect {}   Good {}   Miss {}   Streak {}",
                practice.perfect, practice.good, practice.missed, practice.streak
            ));
        });
        ui.add_space(12.0);

        if practice.is_done() {
            ui.heading(format!(
                "Song complete! {:.0}% accuracy",
                practice.accuracy() * 100.0
            ));
        } else {
            lane::show(ui, practice);
            if !practice.is_playing() {
                ui.add_space(8.0);
                ui.weak("Press Space to play. Type each note as it reaches the line.");
            }
        }

        match action {
            Some(Action::TogglePlay) => self.set_playing(!practice.is_playing()),
            Some(Action::Restart) => {
                if let Some(practice) = &mut self.practice {
                    practice.restart();
                }
                self.set_playing(false);
            }
            Some(Action::Speed(speed)) => {
                let playing = practice.is_playing();
                if let Some(practice) = &mut self.practice {
                    practice.set_speed(speed);
                }
                self.set_playing(playing);
            }
            Some(Action::Difficulty(difficulty)) => {
                let playing = practice.is_playing();
                if let Some(practice) = &mut self.practice {
                    practice.set_difficulty(difficulty);
                }
                self.set_playing(playing);
            }
            None => {}
        }
    }
}

/// A small red bin icon button. Drawn by hand because the bundled fonts have no bin glyph.
fn bin_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    let red = ui.visuals().error_fg_color;
    let color = if response.hovered() {
        red
    } else {
        red.gamma_multiply(0.7)
    };
    let stroke = egui::Stroke::new(1.5, color);
    let painter = ui.painter();
    let c = rect.center();

    painter.line_segment(
        [c + egui::vec2(-6.0, -4.0), c + egui::vec2(6.0, -4.0)],
        stroke,
    );
    painter.line_segment(
        [c + egui::vec2(-2.0, -6.0), c + egui::vec2(2.0, -6.0)],
        stroke,
    );
    let body = egui::Rect::from_min_max(c + egui::vec2(-4.5, -3.0), c + egui::vec2(4.5, 7.0));
    painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Inside);
    for x in [-1.5, 1.5] {
        painter.line_segment([c + egui::vec2(x, -1.0), c + egui::vec2(x, 5.0)], stroke);
    }
    response
}

impl eframe::App for PianoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Typing in the search box shouldn't play notes, but releases still go through
        // so a key held before focusing the box doesn't get stuck.
        let typing = ui.ctx().egui_wants_keyboard_input();
        let (keys, dropped) = ui.input(|input| {
            let keys: Vec<_> = input
                .events
                .iter()
                .filter_map(|event| match *event {
                    Event::Key {
                        key,
                        physical_key,
                        pressed,
                        repeat: false,
                        modifiers,
                    } if !(pressed && typing) => {
                        Some((physical_key.unwrap_or(key), pressed, modifiers.shift))
                    }
                    _ => None,
                })
                .collect();
            let dropped: Vec<_> = input
                .raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_owned())
                .collect();
            (keys, dropped)
        });
        for (key, pressed, shift) in keys {
            self.handle_key(key, pressed, shift);
        }
        for path in dropped {
            self.import(&path);
        }
        if let Some(practice) = &mut self.practice
            && practice.is_playing()
        {
            practice.update();
            if practice.is_done() {
                practice.pause();
            }
            ui.ctx().request_repaint();
        }

        egui::Panel::left("library")
            .default_size(220.0)
            .show(ui, |ui| self.library_panel(ui));

        egui::Panel::bottom("piano").show(ui, |ui| {
            if let Err(error) = &self.audio {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            let playing = self.practice.as_ref().filter(|p| p.is_playing());
            let sounding = playing.map_or([false; 128], |p| p.sounding());
            let mouse_note = piano_view::show(ui, playing.is_some(), |note| {
                self.key_state(note, &sounding)
            });
            self.set_mouse_note(mouse_note);
        });

        egui::CentralPanel::default().show(ui, |ui| self.sheet(ui));
    }
}
