use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const MAX_PLAYERS: usize = 4;
const FILENAME: &str = "super-dr-meerkat.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl TryFrom<String> for Resolution {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let (width, height) = value
            .split_once(['x', 'X', '×'])
            .ok_or("resolution must be WIDTHxHEIGHT, for example \"1920x1080\"")?;
        // SDL2 accepts window dimensions of up to 16384 pixels.
        let dimension = |part: &str| {
            part.trim()
                .parse::<u32>()
                .ok()
                .filter(|pixels| (1..=16_384).contains(pixels))
                .ok_or("resolution width and height must each be integers from 1 to 16384")
        };
        Ok(Self {
            width: dimension(width)?,
            height: dimension(height)?,
        })
    }
}

impl Resolution {
    pub fn fit_within(self, width: u32, height: u32) -> (u32, u32) {
        let scale = (width as f64 / self.width as f64)
            .min(height as f64 / self.height as f64)
            .min(1.0);
        (
            ((self.width as f64 * scale).round() as u32).max(1),
            ((self.height as f64 * scale).round() as u32).max(1),
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub max_players: usize,
    pub resolution: Resolution,
    pub fullscreen: bool,
    pub sound: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_players: MAX_PLAYERS,
            resolution: Resolution {
                width: 1920,
                height: 1080,
            },
            fullscreen: false,
            sound: true,
        }
    }
}

impl Config {
    fn parse(source: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(source).map_err(|e| e.to_string())?;
        if !(1..=MAX_PLAYERS).contains(&config.max_players) {
            return Err("max_players must be between 1 and 4".into());
        }
        Ok(config)
    }

    pub fn load(explicit: Option<PathBuf>) -> Result<Self, String> {
        let required = explicit.is_some();
        let paths = match explicit {
            Some(path) => vec![path],
            None => {
                let mut paths = vec![PathBuf::from(FILENAME)];
                if let Some(dir) = std::env::current_exe()
                    .ok()
                    .as_deref()
                    .and_then(Path::parent)
                {
                    paths.push(dir.join(FILENAME));
                }
                paths
            }
        };
        for path in paths {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    return Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
                }
                Err(e) if !required && e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_and_older_configs_start_at_1080p() {
        let expected = Resolution {
            width: 1920,
            height: 1080,
        };
        assert_eq!(Config::default().resolution, expected);
        assert_eq!(
            Config::parse("max_players = 2").unwrap().resolution,
            expected
        );
        let profile = include_str!("../config/default.toml");
        assert_eq!(Config::parse(profile).unwrap().resolution, expected);
    }

    #[test]
    fn pi2_and_pi3_profile_starts_at_720p() {
        let profile = include_str!("../config/pi2-pi3.toml");
        assert_eq!(
            Config::parse(profile).unwrap().resolution,
            Resolution {
                width: 1280,
                height: 720,
            }
        );
    }

    #[test]
    fn parses_custom_resolutions_and_rejects_invalid_dimensions() {
        for (value, width, height) in [
            ("960x540", 960, 540),
            ("1280x720", 1280, 720),
            ("2560X1440", 2560, 1440),
            (" 768 × 1024 ", 768, 1024),
        ] {
            let config = Config::parse(&format!("resolution = \"{value}\"")).unwrap();
            assert_eq!(config.resolution, Resolution { width, height });
        }
        for value in ["720p", "1280x", "0x720", "1280x-1", "16385x1080", "1x2x3"] {
            assert!(Config::parse(&format!("resolution = \"{value}\"")).is_err());
        }
    }

    #[test]
    fn window_fits_the_desktop_without_upscaling_or_stretching() {
        let full_hd = Config::default().resolution;
        assert_eq!(full_hd.fit_within(2560, 1440), (1920, 1080));
        assert_eq!(full_hd.fit_within(1024, 768), (1024, 576));
        assert_eq!(full_hd.fit_within(1920, 900), (1600, 900));
    }

    #[test]
    fn only_one_to_four_slots_can_be_configured() {
        for n in 1..=4 {
            assert_eq!(
                Config::parse(&format!("max_players = {n}"))
                    .unwrap()
                    .max_players,
                n
            );
        }
        for bad in ["0", "5", "-1", "\"4\"", "1.5"] {
            assert!(Config::parse(&format!("max_players = {bad}")).is_err());
        }
    }
}
