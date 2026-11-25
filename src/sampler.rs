pub enum Command {
    NoteOn(u8),
    NoteOff(u8),
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
}

const MAX_VOICES: usize = 32;
const MASTER_GAIN: f32 = 0.4;
const RELEASE_SECONDS: f32 = 0.3;
const SILENT: f32 = 1e-4;

pub struct Sampler {
    samples: Vec<Sample>,
    voices: Vec<Voice>,
    output_rate: u32,
    release_factor: f32,
}

impl Sampler {
    pub fn new(samples: Vec<Sample>, output_rate: u32) -> Self {
        // Per-frame multiplier that takes a released note down 60 dB over RELEASE_SECONDS.
        let release_factor = 0.001f32.powf(1.0 / (RELEASE_SECONDS * output_rate as f32));
        Self {
            samples,
            voices: Vec::with_capacity(MAX_VOICES),
            output_rate,
            release_factor,
        }
    }

    pub fn handle(&mut self, command: Command) {
        match command {
            Command::NoteOn(note) => self.note_on(note),
            Command::NoteOff(note) => self.release(note),
        }
    }

    fn note_on(&mut self, note: u8) {
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
        if self.voices.len() == MAX_VOICES {
            self.voices.remove(0);
        }
        self.voices.push(Voice {
            note,
            sample: index,
            pos: 0.0,
            step,
            gain: 1.0,
            released: false,
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
        let release_factor = self.release_factor;

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
                if voice.released {
                    voice.gain *= release_factor;
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
        s.handle(Command::NoteOn(61));
        assert_eq!(s.voices[0].sample, 0);
        assert!((s.voices[0].step - 2f64.powf(1.0 / 12.0)).abs() < 1e-9);

        s.handle(Command::NoteOn(62));
        assert_eq!(s.voices[1].sample, 1);
        assert!(s.voices[1].step < 1.0);
    }

    #[test]
    fn plays_sound_after_note_on() {
        let mut s = sampler();
        let mut out = vec![0.0; 512];
        s.render(&mut out, 2);
        assert!(out.iter().all(|&x| x == 0.0));

        s.handle(Command::NoteOn(60));
        s.render(&mut out, 2);
        assert!(out.iter().all(|&x| x > 0.0));
    }

    #[test]
    fn released_note_fades_out_and_is_removed() {
        let mut s = sampler();
        s.handle(Command::NoteOn(60));
        s.handle(Command::NoteOff(60));
        let mut out = vec![0.0; RATE as usize / 2];
        s.render(&mut out, 1);
        assert!(s.voices.is_empty());
    }

    #[test]
    fn repeated_note_releases_the_previous_voice() {
        let mut s = sampler();
        s.handle(Command::NoteOn(60));
        s.handle(Command::NoteOn(60));
        assert!(s.voices[0].released);
        assert!(!s.voices[1].released);
    }

    #[test]
    fn voice_count_is_capped() {
        let mut s = sampler();
        for _ in 0..MAX_VOICES + 5 {
            s.handle(Command::NoteOn(60));
        }
        assert_eq!(s.voices.len(), MAX_VOICES);
    }
}
