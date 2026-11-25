use crate::game::Sound;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

#[derive(Clone, Copy)]
struct Voice {
    clip: usize,
    sample: usize,
}

struct Mixer {
    clips: Vec<Vec<f32>>,
    voices: [Option<Voice>; 8],
    next_voice: usize,
}

fn melody(rate: i32, notes: &[(f32, f32)], gain: f32) -> Vec<f32> {
    let mut samples = Vec::new();
    for &(pitch, seconds) in notes {
        let len = (seconds * rate as f32) as usize;
        for i in 0..len {
            let t = i as f32 / rate as f32;
            let phase = (t * pitch).fract();
            let triangle = 1.0 - 4.0 * (phase - 0.5).abs();
            let envelope = (t / 0.004).min(1.0) * ((seconds - t) / 0.035).min(1.0);
            samples.push(triangle * gain * envelope);
        }
    }
    samples
}

impl Mixer {
    fn new(rate: i32) -> Self {
        let mut clips = vec![
            melody(rate, &[(240.0, 0.027)], 0.055),
            melody(rate, &[(392.0, 0.032), (523.25, 0.045)], 0.080),
            melody(rate, &[(164.81, 0.036), (98.0, 0.062)], 0.11),
            melody(rate, &[(440.0, 0.045)], 0.08),
            melody(
                rate,
                &[(329.63, 0.065), (440.0, 0.065), (659.25, 0.13)],
                0.11,
            ),
            melody(
                rate,
                &[
                    (523.25, 0.09),
                    (659.25, 0.09),
                    (783.99, 0.09),
                    (1046.5, 0.22),
                ],
                0.12,
            ),
            melody(rate, &[(311.13, 0.1), (233.08, 0.12), (155.56, 0.22)], 0.11),
        ];
        for root in [440.0, 554.37, 659.25, 880.0] {
            clips.push(melody(
                rate,
                &[(root, 0.055), (root * 1.25, 0.060), (root * 1.5, 0.12)],
                0.10,
            ));
        }
        Self {
            clips,
            voices: [None; 8],
            next_voice: 0,
        }
    }
}

impl AudioCallback for Mixer {
    type Channel = f32;
    fn callback(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        for slot in &mut self.voices {
            let Some(voice) = slot else { continue };
            let remaining = &self.clips[voice.clip][voice.sample..];
            for (dst, src) in out.iter_mut().zip(remaining) {
                *dst += src;
            }
            if remaining.len() <= out.len() {
                *slot = None;
            } else {
                voice.sample += out.len();
            }
        }
    }
}

pub struct Audio(AudioDevice<Mixer>);

impl Audio {
    pub fn new(sdl: &sdl2::Sdl) -> Result<Self, String> {
        let spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),
            samples: Some(512),
        };
        let device = sdl
            .audio()?
            .open_playback(None, &spec, |spec| Mixer::new(spec.freq))?;
        device.resume();
        Ok(Self(device))
    }

    pub fn silence(&mut self) {
        self.0.lock().voices.fill(None);
    }

    pub fn play(&mut self, sound: Sound) {
        let clip = match sound {
            Sound::Move => 0,
            Sound::Turn => 1,
            Sound::Land => 2,
            Sound::Select => 3,
            Sound::Start => 4,
            Sound::Win => 5,
            Sound::Lose => 6,
            Sound::Match(n) => 7 + usize::from(n.saturating_sub(1).min(3)),
        };
        let mut mixer = self.0.lock();
        let i = mixer.next_voice;
        mixer.voices[i] = Some(Voice { clip, sample: 0 });
        mixer.next_voice = (i + 1) % mixer.voices.len();
    }
}
