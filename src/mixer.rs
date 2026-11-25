use crate::game::Sound;
use crate::music::Music;

#[derive(Clone, Copy)]
struct Voice {
    clip: usize,
    sample: usize,
}

pub struct Mixer {
    enabled: bool,
    music: Music,
    music_gain: f32,
    attack: f32,
    release: f32,
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
    pub fn new(rate: i32, enabled: bool) -> Self {
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
            enabled,
            music: Music::new(rate as u32, 0),
            music_gain: 1.0,
            attack: 1.0 / (rate as f32 * 0.008),
            release: 1.0 / (rate as f32 * 0.12),
            clips,
            voices: [None; 8],
            next_voice: 0,
        }
    }

    pub fn update(&mut self, enabled: bool, sounds: &[Sound]) {
        self.enabled = enabled;
        if enabled {
            for &sound in sounds {
                self.play(sound);
            }
        } else {
            self.voices.fill(None);
        }
    }

    fn play(&mut self, sound: Sound) {
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
        let i = self.next_voice;
        self.voices[i] = Some(Voice { clip, sample: 0 });
        self.next_voice = (i + 1) % self.voices.len();
    }

    pub fn mix(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        if !self.enabled {
            return;
        }
        self.music.mix(out);
        // Duck the tune under each effect, then ease it back up.
        let active_samples = self
            .voices
            .iter()
            .flatten()
            .map(|v| self.clips[v.clip].len() - v.sample)
            .max()
            .unwrap_or(0);
        for (i, sample) in out.iter_mut().enumerate() {
            self.music_gain = if i < active_samples {
                (self.music_gain - self.attack).max(0.35)
            } else {
                (self.music_gain + self.release).min(1.0)
            };
            *sample *= self.music_gain;
        }
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
