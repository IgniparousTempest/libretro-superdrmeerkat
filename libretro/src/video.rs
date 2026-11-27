use crate::{ffi::*, FPS, SAMPLE_RATE};
use std::{ffi::CStr, ptr};
use super_dr_meerkat::config::Resolution;

const RESOLUTION_KEY: &CStr = c"superdrmeerkat_resolution";

pub fn register_option(env: Environment) {
    let mut options = [
        Variable {
            key: RESOLUTION_KEY.as_ptr(),
            value: c"Internal Resolution; Auto|960x540|1280x720|1920x1080|2560x1440".as_ptr(),
        },
        Variable {
            key: ptr::null(),
            value: ptr::null(),
        },
    ];
    unsafe { env(SET_VARIABLES, options.as_mut_ptr().cast()) };
}

fn display_resolution(env: Environment) -> Option<Resolution> {
    let mut display = DisplayMetadata {
        version: 1,
        size: size_of::<DisplayMetadata>() as u32,
        ..DisplayMetadata::default()
    };
    if unsafe {
        env(
            GET_DISPLAY_METADATA,
            (&mut display as *mut DisplayMetadata).cast(),
        )
    } {
        Resolution::new(display.native_width, display.native_height)
    } else {
        None
    }
}

pub fn resolution(env: Environment) -> Resolution {
    let mut option = Variable {
        key: RESOLUTION_KEY.as_ptr(),
        value: ptr::null(),
    };
    if unsafe { env(GET_VARIABLE, (&mut option as *mut Variable).cast()) }
        && !option.value.is_null()
    {
        let value = unsafe { CStr::from_ptr(option.value) };
        if let Some(size) = value
            .to_str()
            .ok()
            .and_then(|s| Resolution::try_from(s.to_owned()).ok())
        {
            return size;
        }
    }
    display_resolution(env).unwrap_or(Resolution::FULL_HD)
}

impl Geometry {
    pub fn new(size: Resolution) -> Self {
        Self {
            base_width: size.width,
            base_height: size.height,
            max_width: size.width,
            max_height: size.height,
            aspect_ratio: size.width as f32 / size.height as f32,
        }
    }

    pub fn av_info(self) -> AvInfo {
        AvInfo {
            geometry: self,
            timing: Timing {
                fps: FPS as f64,
                sample_rate: SAMPLE_RATE as f64,
            },
        }
    }

    pub fn update(self, env: Environment, context_reset: bool) -> Option<Self> {
        let mut changed = false;
        let has_update =
            unsafe { env(GET_VARIABLE_UPDATE, (&mut changed as *mut bool).cast()) } && changed;
        if !has_update && !context_reset {
            return None;
        }
        let size = resolution(env);
        if (size.width, size.height) == (self.base_width, self.base_height) {
            return None;
        }
        let mut next = Self::new(size);
        let accepted = if size.width <= self.max_width && size.height <= self.max_height {
            next.max_width = self.max_width;
            next.max_height = self.max_height;
            unsafe { env(SET_GEOMETRY, (&mut next as *mut Geometry).cast()) }
        } else {
            let mut av = next.av_info();
            unsafe { env(SET_SYSTEM_AV_INFO, (&mut av as *mut AvInfo).cast()) }
        };
        accepted.then_some(next)
    }
}
