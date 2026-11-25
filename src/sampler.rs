pub enum Command {
    Play(u8),
    Stop(u8),
    /// A song note: starts after `delay` seconds and rings for `length` seconds, then fades out
    /// slowly like a piano with the sustain pedal down.
    Schedule {
        delay: f32,
        note: u8,
        length: f32,
    },
    /// Drops scheduled notes that haven't started yet.
    CancelScheduled,
}

pub struct Sample {
    pub note: u8,
    pub rate: u32,
    pub data: Vec<i16>,
}

struct Voice {
    note: u8,
    sample: usize,
    pos: f64,
    step: f64,
    gain: f32,
    released: bool,
    frames_left: Option<u32>,
    release_factor: f32,
}

struct Pending {
    frames_until: u32,
    note: u8,
    length: u32,
}

const MAX_VOICES: usize = 64;
const MASTER_GAIN: f32 = 0.35;
const RELEASE_SECONDS: f32 = 0.3;
const PEDAL_RELEASE_SECONDS: f32 = 1.2;
const SILENT: f32 = 1e-4;

pub struct Sampler {
    samples: Vec<Sample>,
    voices: Vec<Voice>,
    pending: Vec<Pending>,
    output_rate: u32,
    release_factor: f32,
    pedal_release_factor: f32,
}

impl Sampler {
    pub fn new(samples: Vec<Sample>, output_rate: u32) -> Self {
        // Per-frame multiplier that takes a released note down 60 dB over the given time.
        let factor = |seconds: f32| 0.001f32.powf(1.0 / (seconds * output_rate as f32));
        Self {
            samples,
            voices: Vec::with_capacity(MAX_VOICES),
            pending: Vec::with_capacity(256),
            output_rate,
            release_factor: factor(RELEASE_SECONDS),
            pedal_release_factor: factor(PEDAL_RELEASE_SECONDS),
        }
    }

    pub fn handle(&mut self, command: Command) {
        match command {
            Command::Play(note) => self.note_on(note, None, self.release_factor),
            Command::Stop(note) => self.release(note),
            Command::Schedule {
                delay,
                note,
                length,
            } => self.pending.push(Pending {
                frames_until: self.frames(delay),
                note,
                length: self.frames(length),
            }),
            Command::CancelScheduled => self.pending.clear(),
        }
    }

    fn frames(&self, seconds: f32) -> u32 {
        (seconds * self.output_rate as f32) as u32
    }

    // Scheduled notes start on buffer boundaries, which is accurate to a few milliseconds.
    fn start_due(&mut self, frames: u32) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].frames_until < frames {
                let p = self.pending.swap_remove(i);
                self.note_on(p.note, Some(p.length), self.pedal_release_factor);
            } else {
                self.pending[i].frames_until -= frames;
                i += 1;
            }
        }
    }

    fn note_on(&mut self, note: u8, frames_left: Option<u32>, release_factor: f32) {
        let Some((index, sample)) = self
            .samples
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| s.note.abs_diff(note))
        else {
            return;
        };

        let semitones = f64::from(note) - f64::from(sample.note);
        let step =
            2f64.powf(semitones / 12.0) * f64::from(sample.rate) / f64::from(self.output_rate);

        self.release(note);
        if self.voices.len() == MAX_VOICES
            && let Some(quietest) =
                (0..MAX_VOICES).min_by(|&a, &b| self.voices[a].gain.total_cmp(&self.voices[b].gain))
        {
            self.voices.swap_remove(quietest);
        }
        self.voices.push(Voice {
            note,
            sample: index,
            pos: 0.0,
            step,
            gain: 1.0,
            released: false,
            frames_left,
            release_factor,
        });
    }

    fn release(&mut self, note: u8) {
        for voice in self.voices.iter_mut().filter(|v| v.note == note) {
            voice.released = true;
        }
    }

    /// Fills an interleaved buffer, writing the same mono signal to every channel.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        out.fill(0.0);
        self.start_due((out.len() / channels) as u32);

        for voice in &mut self.voices {
            let data = &self.samples[voice.sample].data;
            for frame in out.chunks_exact_mut(channels) {
                let i = voice.pos as usize;
                if i + 1 >= data.len() || voice.gain < SILENT {
                    voice.gain = 0.0;
                    break;
                }
                let frac = (voice.pos - i as f64) as f32;
                let a = f32::from(data[i]);
                let b = f32::from(data[i + 1]);
                let value = (a + (b - a) * frac) / 32768.0 * voice.gain * MASTER_GAIN;

                for s in frame.iter_mut() {
                    *s += value;
                }
                voice.pos += voice.step;
                if let Some(frames) = voice.frames_left.as_mut() {
                    if *frames == 0 {
                        voice.released = true;
                    } else {
                        *frames -= 1;
                    }
                }
                if voice.released {
                    voice.gain *= voice.release_factor;
                }
            }
        }

        self.voices.retain(|v| v.gain > 0.0);
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sampler() -> Sampler {
        let samples = [60, 63]
            .map(|note| Sample {
                note,
                rate: RATE,
                data: vec![i16::MAX / 2; RATE as usize],
            })
            .into();
        Sampler::new(samples, RATE)
    }

    #[test]
    fn picks_nearest_sample_and_pitch_shifts() {
        let mut s = sampler();
        s.handle(Command::Play(61));
        assert_eq!(s.voices[0].sample, 0);
        assert!((s.voices[0].step - 2f64.powf(1.0 / 12.0)).abs() < 1e-9);

        s.handle(Command::Play(62));
        assert_eq!(s.voices[1].sample, 1);
        assert!(s.voices[1].step < 1.0);
    }

    #[test]
    fn plays_sound_after_note_on() {
        let mut s = sampler();
        let mut out = vec![0.0; 512];
        s.render(&mut out, 2);
        assert!(out.iter().all(|&x| x == 0.0));

        s.handle(Command::Play(60));
        s.render(&mut out, 2);
        assert!(out.iter().all(|&x| x > 0.0));
    }

    #[test]
    fn released_note_fades_out_and_is_removed() {
        let mut s = sampler();
        s.handle(Command::Play(60));
        s.handle(Command::Stop(60));
        let mut out = vec![0.0; RATE as usize / 2];
        s.render(&mut out, 1);
        assert!(s.voices.is_empty());
    }

    #[test]
    fn scheduled_note_starts_after_its_delay_and_releases_itself() {
        let mut s = sampler();
        s.handle(Command::Schedule {
            delay: 0.01,
            note: 60,
            length: 0.01,
        });
        let mut out = vec![0.0; 100];
        s.render(&mut out, 1);
        assert!(s.voices.is_empty());

        let mut out = vec![0.0; RATE as usize / 50];
        s.render(&mut out, 1);
        assert_eq!(s.voices.len(), 1);

        let mut out = vec![0.0; RATE as usize * 3];
        s.render(&mut out, 1);
        assert!(s.voices.is_empty());
    }

    #[test]
    fn cancel_drops_notes_that_have_not_started() {
        let mut s = sampler();
        s.handle(Command::Schedule {
            delay: 0.01,
            note: 60,
            length: 1.0,
        });
        s.handle(Command::CancelScheduled);
        let mut out = vec![0.0; RATE as usize / 10];
        s.render(&mut out, 1);
        assert!(s.voices.is_empty());
    }

    #[test]
    fn repeated_note_releases_the_previous_voice() {
        let mut s = sampler();
        s.handle(Command::Play(60));
        s.handle(Command::Play(60));
        assert!(s.voices[0].released);
        assert!(!s.voices[1].released);
    }

    #[test]
    fn voice_count_is_capped() {
        let mut s = sampler();
        for _ in 0..MAX_VOICES + 5 {
            s.handle(Command::Play(60));
        }
        assert_eq!(s.voices.len(), MAX_VOICES);
    }
}
