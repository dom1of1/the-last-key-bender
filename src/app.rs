use std::collections::HashMap;

use eframe::egui::{self, Event, Key};

use crate::audio::Audio;
use crate::keymap;

pub struct PianoApp {
    audio: Result<Audio, String>,
    // Remembers the note each key started, so releasing Shift first still stops the right note.
    held: HashMap<Key, u8>,
}

impl PianoApp {
    pub fn new() -> Self {
        Self {
            audio: Audio::new(),
            held: HashMap::new(),
        }
    }

    fn handle_key(&mut self, key: Key, pressed: bool, shift: bool) {
        let Ok(audio) = &self.audio else { return };
        if pressed {
            if let Some(note) = keymap::note_for(key, shift) {
                self.held.insert(key, note);
                audio.note_on(note);
            }
        } else if let Some(note) = self.held.remove(&key) {
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
        });
    }
}
