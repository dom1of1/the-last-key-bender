use std::collections::HashMap;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

use crate::keymap;

pub const MAX_STEPS: usize = 5_000;

const DEFAULT_BPM: f32 = 100.0;
const DEFAULT_MIDI_TEMPO: u32 = 500_000;
const CHORD_WINDOW_BEATS: f32 = 1.0 / 16.0;
const MIN_BEATS: f32 = 0.25;
const LOWEST_NOTE: u8 = 36;
const HIGHEST_NOTE: u8 = 96;
const DRUM_CHANNEL: u8 = 9;

#[derive(Clone)]
pub struct Note {
    pub key: u8,
    pub beats: f32,
}

/// Notes that start together. A step with several notes is a chord.
#[derive(Clone)]
pub struct Step {
    pub start: f32,
    pub notes: Vec<Note>,
}

#[derive(Clone)]
pub struct Chart {
    pub bpm: f32,
    pub steps: Vec<Step>,
}

/// Parses a letter sheet such as `t y u- [tu]--- | o`.
/// A note or chord lasts one beat, each `-` holds it a beat longer, and `|` is a one-beat rest.
/// An optional `bpm: 120` line sets the tempo. Anything else is ignored.
pub fn parse(sheet: &str) -> Chart {
    let mut chart = Chart {
        bpm: DEFAULT_BPM,
        steps: Vec::new(),
    };
    let mut time = 0.0;
    let mut can_extend = false;

    for line in sheet.lines() {
        if let Some(bpm) = line.trim().strip_prefix("bpm:") {
            if let Ok(bpm) = bpm.trim().parse::<f32>()
                && bpm > 0.0
            {
                chart.bpm = bpm;
            }
            continue;
        }

        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            let labels: Vec<char> = match c {
                '-' => {
                    if let Some(step) = chart.steps.last_mut().filter(|_| can_extend) {
                        step.notes.iter_mut().for_each(|n| n.beats += 1.0);
                        time += 1.0;
                    }
                    continue;
                }
                '|' => {
                    time += 1.0;
                    can_extend = false;
                    continue;
                }
                '[' => chars.by_ref().take_while(|&c| c != ']').collect(),
                _ => vec![c],
            };

            let notes: Vec<Note> = labels
                .into_iter()
                .filter_map(|label| {
                    Some(Note {
                        key: keymap::note_for_label(label)?,
                        beats: 1.0,
                    })
                })
                .collect();
            if !notes.is_empty() {
                chart.steps.push(Step { start: time, notes });
                time += 1.0;
                can_extend = true;
            }
        }
    }
    chart
}

/// Reads a MIDI file, keeping each note's start and length. Drums are skipped and notes
/// outside the keyboard are moved by octaves. Only the first tempo is used.
pub fn from_midi(bytes: &[u8]) -> Result<Chart, String> {
    let smf = Smf::parse(bytes).map_err(|e| format!("Not a valid MIDI file: {e}"))?;
    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(t) => f32::from(t.as_int()),
        Timing::Timecode(fps, subframes) => fps.as_f32() * f32::from(subframes) / 2.0,
    };

    let mut tempo = None;
    let mut notes = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0;
        let mut sounding = HashMap::new();
        for event in track {
            tick += event.delta.as_int();
            match event.kind {
                TrackEventKind::Midi { channel, message } if channel.as_int() != DRUM_CHANNEL => {
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel > 0 => {
                            sounding.insert((channel, key), tick);
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            if let Some(start) = sounding.remove(&(channel, key)) {
                                notes.push((start, tick, key.as_int()));
                            }
                        }
                        _ => {}
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    tempo.get_or_insert(t.as_int());
                }
                _ => {}
            }
        }
    }
    notes.sort_unstable();

    let mut steps: Vec<Step> = Vec::new();
    for (start, end, key) in notes {
        let key = fit_to_keyboard(key);
        let beats = ((end - start) as f32 / ticks_per_beat).max(MIN_BEATS);
        let start = start as f32 / ticks_per_beat;
        let note = Note { key, beats };

        match steps.last_mut() {
            Some(step) if start - step.start <= CHORD_WINDOW_BEATS => {
                match step.notes.iter_mut().find(|n| n.key == key) {
                    Some(existing) => existing.beats = existing.beats.max(beats),
                    None => step.notes.push(note),
                }
            }
            _ => steps.push(Step {
                start,
                notes: vec![note],
            }),
        }
    }

    let bpm = 60_000_000.0 / tempo.unwrap_or(DEFAULT_MIDI_TEMPO) as f32;
    Ok(Chart { bpm, steps })
}

fn fit_to_keyboard(mut note: u8) -> u8 {
    while note < LOWEST_NOTE {
        note += 12;
    }
    while note > HIGHEST_NOTE {
        note -= 12;
    }
    note
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::{u4, u7, u24, u28};
    use midly::{Format, Header, TrackEvent};

    fn summary(chart: &Chart) -> Vec<(f32, Vec<(u8, f32)>)> {
        chart
            .steps
            .iter()
            .map(|s| (s.start, s.notes.iter().map(|n| (n.key, n.beats)).collect()))
            .collect()
    }

    #[test]
    fn parses_notes_chords_holds_and_rests() {
        let chart = parse("bpm: 90\nt y-- | [tu] T\n[]x?");
        assert_eq!(chart.bpm, 90.0);
        assert_eq!(
            summary(&chart),
            [
                (0.0, vec![(60, 1.0)]),
                (1.0, vec![(62, 3.0)]),
                (5.0, vec![(60, 1.0), (64, 1.0)]),
                (6.0, vec![(61, 1.0)]),
                (7.0, vec![(88, 1.0)]),
            ]
        );
    }

    #[test]
    fn dash_after_rest_does_nothing() {
        let chart = parse("t | - y");
        assert_eq!(chart.steps[1].start, 2.0);
    }

    fn event(delta: u32, kind: TrackEventKind<'static>) -> TrackEvent<'static> {
        TrackEvent {
            delta: u28::new(delta),
            kind,
        }
    }

    fn note(delta: u32, key: u8, on: bool, channel: u8) -> TrackEvent<'static> {
        let (key, vel) = (u7::new(key), u7::new(if on { 100 } else { 0 }));
        event(
            delta,
            TrackEventKind::Midi {
                channel: u4::new(channel),
                message: MidiMessage::NoteOn { key, vel },
            },
        )
    }

    fn midi(track: Vec<TrackEvent<'static>>) -> Vec<u8> {
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(480.into())),
            tracks: vec![track],
        };
        let mut bytes = Vec::new();
        smf.write_std(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn reads_midi_timing_chords_and_tempo() {
        let bytes = midi(vec![
            event(
                0,
                TrackEventKind::Meta(MetaMessage::Tempo(u24::new(1_000_000))),
            ),
            note(0, 60, true, 0),
            note(480, 60, false, 0),
            note(0, 60, true, 0),
            note(10, 24, true, 0),
            note(0, 50, true, DRUM_CHANNEL),
            note(470, 60, false, 0),
            note(960, 24, false, 0),
            note(0, 50, false, DRUM_CHANNEL),
        ]);
        let chart = from_midi(&bytes).unwrap();
        assert_eq!(chart.bpm, 60.0);
        assert_eq!(chart.steps.len(), 2);
        assert_eq!(chart.steps[0].notes[0].beats, 1.0);
        assert_eq!(chart.steps[1].start, 1.0);
        let keys: Vec<_> = chart.steps[1].notes.iter().map(|n| n.key).collect();
        assert_eq!(keys, [60, 36]);
        assert!((chart.steps[1].notes[1].beats - 2.979).abs() < 0.01);
    }

    #[test]
    fn rejects_bad_midi() {
        assert!(from_midi(b"not midi").is_err());
    }
}
