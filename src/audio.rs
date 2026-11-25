use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use super_dr_meerkat::{game::Sound, mixer::Mixer};

struct Playback(Mixer);

impl AudioCallback for Playback {
    type Channel = f32;
    fn callback(&mut self, out: &mut [f32]) {
        self.0.mix(out);
    }
}

pub struct Audio(AudioDevice<Playback>);

impl Audio {
    pub fn new(sdl: &sdl2::Sdl, enabled: bool) -> Result<Self, String> {
        let spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),
            samples: Some(512),
        };
        let device = sdl
            .audio()?
            .open_playback(None, &spec, |spec| Playback(Mixer::new(spec.freq, enabled)))?;
        device.resume();
        Ok(Self(device))
    }

    pub fn update(&mut self, enabled: bool, sounds: &[Sound]) {
        self.0.lock().0.update(enabled, sounds);
    }
}
