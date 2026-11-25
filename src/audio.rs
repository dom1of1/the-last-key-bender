use std::io::Cursor;
use std::sync::mpsc::{self, Sender};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig, SupportedBufferSize};
use lewton::inside_ogg::OggStreamReader;

use crate::sampler::{Command, Sample, Sampler};

const BUFFER_FRAMES: u32 = 256;

const PIANO: [(u8, &[u8]); 21] = [
    (36, include_bytes!("../assets/piano/C2.ogg")),
    (39, include_bytes!("../assets/piano/Ds2.ogg")),
    (42, include_bytes!("../assets/piano/Fs2.ogg")),
    (45, include_bytes!("../assets/piano/A2.ogg")),
    (48, include_bytes!("../assets/piano/C3.ogg")),
    (51, include_bytes!("../assets/piano/Ds3.ogg")),
    (54, include_bytes!("../assets/piano/Fs3.ogg")),
    (57, include_bytes!("../assets/piano/A3.ogg")),
    (60, include_bytes!("../assets/piano/C4.ogg")),
    (63, include_bytes!("../assets/piano/Ds4.ogg")),
    (66, include_bytes!("../assets/piano/Fs4.ogg")),
    (69, include_bytes!("../assets/piano/A4.ogg")),
    (72, include_bytes!("../assets/piano/C5.ogg")),
    (75, include_bytes!("../assets/piano/Ds5.ogg")),
    (78, include_bytes!("../assets/piano/Fs5.ogg")),
    (81, include_bytes!("../assets/piano/A5.ogg")),
    (84, include_bytes!("../assets/piano/C6.ogg")),
    (87, include_bytes!("../assets/piano/Ds6.ogg")),
    (90, include_bytes!("../assets/piano/Fs6.ogg")),
    (93, include_bytes!("../assets/piano/A6.ogg")),
    (96, include_bytes!("../assets/piano/C7.ogg")),
];

pub struct Audio {
    commands: Sender<Command>,
    _stream: cpal::Stream,
}

impl Audio {
    pub fn new() -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No audio output device found")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;

        let mut config = supported.config();
        if let SupportedBufferSize::Range { min, max } = *supported.buffer_size() {
            config.buffer_size = BufferSize::Fixed(BUFFER_FRAMES.clamp(min, max));
        }

        let sampler = Sampler::new(load_piano(), config.sample_rate);
        let (commands, receiver) = mpsc::channel();

        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config, sampler, receiver),
            SampleFormat::I16 => build::<i16>(&device, config, sampler, receiver),
            SampleFormat::I32 => build::<i32>(&device, config, sampler, receiver),
            SampleFormat::U16 => build::<u16>(&device, config, sampler, receiver),
            format => return Err(format!("Unsupported sample format: {format}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;

        Ok(Self {
            commands,
            _stream: stream,
        })
    }

    pub fn note_on(&self, note: u8) {
        let _ = self.commands.send(Command::NoteOn(note));
    }

    pub fn note_off(&self, note: u8) {
        let _ = self.commands.send(Command::NoteOff(note));
    }
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    mut sampler: Sampler,
    commands: mpsc::Receiver<Command>,
) -> Result<cpal::Stream, cpal::Error> {
    let channels = usize::from(config.channels);
    let mut mix = Vec::new();

    device.build_output_stream(
        config,
        move |out: &mut [T], _| {
            for command in commands.try_iter() {
                sampler.handle(command);
            }
            mix.resize(out.len(), 0.0);
            sampler.render(&mut mix, channels);
            for (o, &s) in out.iter_mut().zip(&mix) {
                *o = T::from_sample(s);
            }
        },
        |e| eprintln!("Audio stream error: {e}"),
        None,
    )
}

fn load_piano() -> Vec<Sample> {
    PIANO
        .iter()
        .map(|&(note, bytes)| decode(note, bytes))
        .collect()
}

fn decode(note: u8, bytes: &[u8]) -> Sample {
    let mut reader =
        OggStreamReader::new(Cursor::new(bytes)).expect("bundled piano sample is valid");
    let mut data = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .expect("bundled piano sample is valid")
    {
        data.extend(packet);
    }
    Sample {
        note,
        rate: reader.ident_hdr.audio_sample_rate,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_samples_decode_as_mono() {
        for &(note, bytes) in &PIANO {
            let reader = OggStreamReader::new(Cursor::new(bytes)).unwrap();
            assert_eq!(reader.ident_hdr.audio_channels, 1, "note {note}");
            assert!(!decode(note, bytes).data.is_empty(), "note {note}");
        }
    }
}
