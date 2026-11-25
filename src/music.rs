// MIDI pitches in eighth notes. A zero rests and T ties the preceding note.
const T: u8 = 1;
type Bar = [u8; 8];

struct Tune {
    bpm: u32,
    swing: u32, // Percent of a beat occupied by the first eighth.
    kicks: u8,
    snares: u8,
    melody: [Bar; 8],
    bass: [Bar; 8],
    chords: [[u8; 3]; 8],
}

const TUNES: [Tune; 3] = [
    // Pill Pop Hop — F major, swung eighths and a bouncing octave bass.
    Tune {
        bpm: 144,
        swing: 58,
        kicks: 0b0001_0001,
        snares: 0b0100_0100,
        melody: [
            [77, 0, 81, 84, 0, 81, 79, 77],
            [76, T, 0, 72, 76, 79, 0, 76],
            [77, 0, 81, 84, 86, T, 84, 81],
            [79, T, 76, 0, 72, 74, 76, 0],
            [77, 0, 81, 84, 89, 0, 88, 84],
            [86, T, 82, 81, 79, 0, 77, 74],
            [79, 82, 0, 86, 84, 79, 76, 72],
            [77, T, 81, 84, 77, T, 0, 72],
        ],
        bass: [
            [41, T, 53, 0, 48, 0, 52, 53],
            [40, T, 52, 0, 43, 0, 47, 48],
            [38, T, 50, 0, 45, 0, 48, 50],
            [36, T, 48, 0, 43, 0, 46, 48],
            [41, T, 53, 0, 48, 0, 52, 53],
            [46, T, 58, 0, 53, 0, 50, 48],
            [36, T, 48, 0, 43, 0, 46, 48],
            [41, 0, 48, 53, 41, T, 0, 36],
        ],
        chords: [
            [65, 69, 72],
            [64, 67, 72],
            [62, 65, 69],
            [64, 67, 70],
            [65, 69, 72],
            [62, 65, 70],
            [64, 67, 70],
            [65, 69, 72],
        ],
    },
    // Double Dose Dash — D major, straight eighths with a displaced second kick.
    Tune {
        bpm: 158,
        swing: 50,
        kicks: 0b0010_0001,
        snares: 0b0100_0100,
        melody: [
            [74, 81, 0, 78, 81, 0, 83, 81],
            [74, 81, 0, 78, 85, T, 83, 81],
            [79, 0, 83, 81, 79, 78, 76, 74],
            [76, 81, 0, 85, 88, 85, 81, 0],
            [74, 81, 0, 78, 86, 0, 85, 81],
            [83, 0, 86, 85, 83, 81, 78, 0],
            [79, 83, 86, 0, 88, 85, 81, 76],
            [78, 81, 0, 85, 86, T, 0, 81],
        ],
        bass: [
            [38, 38, 50, 45, 38, 50, 45, 49],
            [42, 42, 54, 49, 42, 54, 49, 0],
            [43, 43, 55, 50, 43, 55, 50, 54],
            [45, 45, 57, 52, 45, 57, 52, 0],
            [38, 38, 50, 45, 38, 50, 45, 49],
            [47, 47, 59, 54, 47, 59, 54, 0],
            [43, 43, 55, 50, 45, 45, 52, 57],
            [38, 45, 50, 0, 38, T, 0, 45],
        ],
        chords: [
            [62, 66, 69],
            [61, 66, 69],
            [62, 67, 71],
            [61, 64, 69],
            [62, 66, 69],
            [62, 66, 71],
            [61, 64, 69],
            [62, 66, 69],
        ],
    },
    // Sunrise Scramble — A major, a 3+3+2 bass pattern against the backbeat.
    Tune {
        bpm: 136,
        swing: 50,
        kicks: 0b0100_1001,
        snares: 0b0001_0000,
        melody: [
            [0, 76, 81, 0, 80, 76, 73, 0],
            [74, 0, 78, 81, 0, 78, 76, 74],
            [0, 76, 80, 0, 83, 80, 76, 0],
            [73, 0, 76, 81, 0, 83, 85, T],
            [0, 81, 85, 0, 88, 85, 83, 81],
            [78, 0, 81, 86, 0, 85, 81, 78],
            [0, 80, 83, 0, 88, 83, 80, 76],
            [81, T, 76, 0, 73, 76, 81, 0],
        ],
        bass: [
            [45, T, 0, 52, T, 0, 57, 0],
            [38, T, 0, 45, T, 0, 50, 0],
            [40, T, 0, 47, T, 0, 52, 0],
            [45, T, 0, 52, T, 0, 57, 52],
            [42, T, 0, 49, T, 0, 54, 0],
            [38, T, 0, 45, T, 0, 50, 0],
            [40, T, 0, 47, T, 0, 52, 47],
            [45, T, 0, 52, T, 0, 57, 0],
        ],
        chords: [
            [61, 64, 69],
            [62, 66, 69],
            [59, 64, 68],
            [61, 64, 69],
            [61, 66, 69],
            [62, 66, 69],
            [59, 64, 68],
            [61, 64, 69],
        ],
    },
];

#[derive(Default)]
struct Voice {
    phase: f32,
    increment: f32,
    age: u32,
    length: u32,
}

impl Voice {
    fn start(&mut self, pitch: u8, rate: u32, length: u32) {
        self.phase = 0.0;
        self.age = 0;
        self.length = length;
        self.increment = 440.0 * 2.0_f32.powf((f32::from(pitch) - 69.0) / 12.0) / rate as f32;
    }

    fn sample(&mut self, ramp: u32, pulse: bool) -> f32 {
        if self.age >= self.length {
            return 0.0;
        }
        let envelope = self.age.min(self.length - self.age).min(ramp) as f32 / ramp as f32;
        self.age += 1;
        self.phase += self.increment;
        self.phase -= self.phase.floor();
        // A short slope at each edge softens the upper harmonics of the pulse.
        let wave = if pulse {
            ((0.16 - (self.phase - 0.5).abs()) * 24.0).clamp(-1.0, 1.0) * 0.8 + 0.288
        } else {
            1.0 - 4.0 * (self.phase - 0.5).abs()
        };
        wave * envelope
    }
}

pub struct Music {
    rate: u32,
    track: usize,
    step: usize,
    left: u32,
    lead: Voice,
    bass: Voice,
    keys: Voice,
    kick: Voice,
    noise: u16,
    noise_left: u32,
    noise_length: u32,
    noise_gain: f32,
}

impl Music {
    pub fn new(rate: u32, track: usize) -> Self {
        Self {
            rate,
            track,
            step: 0,
            left: 0,
            lead: Voice::default(),
            bass: Voice::default(),
            keys: Voice::default(),
            kick: Voice::default(),
            noise: 0x7251,
            noise_left: 0,
            noise_length: 1,
            noise_gain: 0.0,
        }
    }

    fn advance(&mut self) {
        if self.step == 64 {
            self.track = (self.track + 1) % TUNES.len();
            self.step = 0;
        }
        let tune = &TUNES[self.track];
        let beat = self.rate * 60 / tune.bpm;
        let first = beat * tune.swing / 100;
        let duration = |step: usize| if step % 2 == 0 { first } else { beat - first };
        let bar = self.step / 8;
        let eighth = self.step % 8;
        self.left = duration(self.step);
        for (notes, voice) in [
            (&tune.melody[bar], &mut self.lead),
            (&tune.bass[bar], &mut self.bass),
        ] {
            if notes[eighth] > T {
                let mut length = self.left;
                for (offset, _) in notes[eighth + 1..]
                    .iter()
                    .take_while(|&&n| n == T)
                    .enumerate()
                {
                    length += duration(eighth + offset + 1);
                }
                voice.start(notes[eighth], self.rate, length * 7 / 8);
            }
        }
        if eighth % 2 == 1 {
            let chord = tune.chords[bar];
            self.keys.start(
                chord[[0, 2, 1, 2][eighth / 2]],
                self.rate,
                self.left * 2 / 3,
            );
        }
        if tune.kicks & (1 << eighth) != 0 {
            self.kick.start(43, self.rate, self.rate / 12);
        }
        let snare = tune.snares & (1 << eighth) != 0;
        self.noise_length = self.rate / if snare { 18 } else { 85 };
        self.noise_left = self.noise_length;
        self.noise_gain = if snare { 0.014 } else { 0.005 };
        self.step += 1;
    }

    pub fn mix(&mut self, samples: &mut [f32]) {
        let ramp = (self.rate / 150).max(1);
        for out in samples {
            if self.left == 0 {
                self.advance();
            }
            self.left -= 1;
            let mut drum = self.kick.sample(ramp, false) * 0.024;
            if self.noise_left > 0 {
                let bit = (self.noise ^ (self.noise >> 1)) & 1;
                self.noise = (self.noise >> 1) | (bit << 14);
                let decay = self.noise_left as f32 / self.noise_length as f32;
                drum += (f32::from(self.noise & 1) * 2.0 - 1.0) * decay * decay * self.noise_gain;
                self.noise_left -= 1;
            }
            *out += drum
                + self.lead.sample(ramp, true) * 0.028
                + self.bass.sample(ramp, false) * 0.036
                + self.keys.sample(ramp, false) * 0.014;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_wraps_after_all_three_scores() {
        let rate = 8000;
        let mut music = Music::new(rate, 0);
        for (index, tune) in TUNES.iter().enumerate() {
            let length = (rate * 60 / tune.bpm * 32) as usize;
            let mut bar = vec![0.0; length];
            // Frontends ask for different buffer sizes.
            for chunk in bar.chunks_mut(311) {
                music.mix(chunk);
            }
            assert_eq!(music.track, index);
            assert_eq!(music.left, 0);
            assert!(bar.iter().any(|v| v.abs() > 0.01));
            assert!(bar.iter().all(|v| v.abs() < 0.12));
        }
        music.mix(&mut [0.0]);
        assert_eq!(music.track, 0);
    }
}
