//! The libretro v1 C types used by this core.
use std::ffi::{c_char, c_uint, c_void};

pub type Environment = unsafe extern "C" fn(c_uint, *mut c_void) -> bool;
pub type Video = unsafe extern "C" fn(*const c_void, c_uint, c_uint, usize);
pub type Audio = unsafe extern "C" fn(i16, i16);
pub type AudioBatch = unsafe extern "C" fn(*const i16, usize) -> usize;
pub type Poll = unsafe extern "C" fn();
pub type Input = unsafe extern "C" fn(c_uint, c_uint, c_uint, c_uint) -> i16;
pub type Framebuffer = unsafe extern "C" fn() -> usize;
pub type ProcAddress = unsafe extern "C" fn(*const c_char) -> Option<unsafe extern "C" fn()>;

pub const SHUTDOWN: c_uint = 7;
pub const GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const SET_PIXEL_FORMAT: c_uint = 10;
pub const SET_INPUT_DESCRIPTORS: c_uint = 11;
pub const SET_HW_RENDER: c_uint = 14;
pub const SET_SUPPORT_NO_GAME: c_uint = 18;
pub const GET_SAVE_DIRECTORY: c_uint = 31;
pub const SET_CONTROLLER_INFO: c_uint = 35;
pub const GET_PREFERRED_HW_RENDER: c_uint = 56;

pub const OPENGL: c_uint = 1;
pub const GLES2: c_uint = 2;
pub const OPENGL_CORE: c_uint = 3;
pub const GLES3: c_uint = 4;
pub const GLES_VERSION: c_uint = 5;

#[repr(C)]
#[derive(Default)]
pub struct HwRender {
    pub context_type: c_uint,
    pub context_reset: Option<extern "C" fn()>,
    pub get_current_framebuffer: Option<Framebuffer>,
    pub get_proc_address: Option<ProcAddress>,
    pub depth: bool,
    pub stencil: bool,
    pub bottom_left_origin: bool,
    pub version_major: c_uint,
    pub version_minor: c_uint,
    pub cache_context: bool,
    pub context_destroy: Option<extern "C" fn()>,
    pub debug_context: bool,
}

#[repr(C)]
pub struct SystemInfo {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

#[repr(C)]
pub struct GameInfo {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

#[repr(C)]
pub struct Geometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
pub struct Timing {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
pub struct AvInfo {
    pub geometry: Geometry,
    pub timing: Timing,
}

#[repr(C)]
pub struct InputDescriptor {
    pub port: c_uint,
    pub device: c_uint,
    pub index: c_uint,
    pub id: c_uint,
    pub description: *const c_char,
}

#[repr(C)]
pub struct ControllerDescription {
    pub description: *const c_char,
    pub id: c_uint,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ControllerInfo {
    pub types: *const ControllerDescription,
    pub num_types: c_uint,
}
