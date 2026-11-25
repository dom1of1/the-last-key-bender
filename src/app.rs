use std::collections::HashMap;

use eframe::egui::{self, Event, Key};

use crate::audio::Audio;
use crate::{keymap, piano_view};

pub struct PianoApp {
    audio: Result<Audio, String>,
    // Remembers the note each key started, so releasing Shift first still stops the right note.
    held: HashMap<Key, u8>,
    mouse_note: Option<u8>,
}

impl PianoApp {
    pub fn new() -> Self {
        Self {
            audio: Audio::new(),
            held: HashMap::new(),
            mouse_note: None,
        }
    }

    fn handle_key(&mut self, key: Key, pressed: bool, shift: bool) {
        if pressed {
            if let Some(note) = keymap::note_for(key, shift) {
                self.held.insert(key, note);
                self.note_on(note);
            }
        } else if let Some(note) = self.held.remove(&key) {
            self.note_off(note);
        }
    }

    fn set_mouse_note(&mut self, note: Option<u8>) {
        if note == self.mouse_note {
            return;
        }
        if let Some(old) = self.mouse_note {
            self.note_off(old);
        }
        if let Some(new) = note {
            self.note_on(new);
        }
        self.mouse_note = note;
    }

    fn is_pressed(&self, note: u8) -> bool {
        self.mouse_note == Some(note) || self.held.values().any(|&n| n == note)
    }

    fn note_on(&self, note: u8) {
        if let Ok(audio) = &self.audio {
            audio.note_on(note);
        }
    }

    fn note_off(&self, note: u8) {
        if let Ok(audio) = &self.audio {
            audio.note_off(note);
        }
    }
}

impl eframe::App for PianoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.input(|input| {
            for event in &input.events {
                if let Event::Key {
                    key,
                    physical_key,
                    pressed,
                    repeat: false,
                    modifiers,
                } = *event
                {
                    self.handle_key(physical_key.unwrap_or(key), pressed, modifiers.shift);
                }
            }
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("The Last Key Bender");
            if let Err(error) = &self.audio {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            let mouse_note = piano_view::show(ui, |note| self.is_pressed(note));
            self.set_mouse_note(mouse_note);
        });
    }
}
