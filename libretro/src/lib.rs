mod ffi;
mod input;
mod video;

use ffi::*;
use std::{
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    ptr,
    sync::{LazyLock, Mutex},
    time::Duration,
};
use super_dr_meerkat::{
    app::App,
    config::{Config, Resolution, MAX_PLAYERS},
    mixer::Mixer,
    render::Renderer,
    scores::HighScores,
    ui::Ui,
};

const SAMPLE_RATE: usize = 44_100;
const FPS: usize = 60;
const AUDIO_FRAMES: usize = SAMPLE_RATE / FPS;

struct Game {
    geometry: Geometry,
    save_dir: Option<PathBuf>,
    app: App,
    ui: Ui,
    mixer: Mixer,
    previous: [u16; MAX_PLAYERS],
    frame: u32,
}

impl Game {
    fn new(geometry: Geometry, save_dir: Option<PathBuf>) -> Self {
        let scores = save_dir.as_ref().map_or_else(HighScores::cabinet, |dir| {
            HighScores::open(dir.join("super-dr-meerkat/high-scores.toml"))
        });
        let app = App::new(&Config::default(), true, scores);
        Self {
            mixer: Mixer::new(SAMPLE_RATE as i32, app.sound),
            app,
            geometry,
            save_dir,
            ui: Ui::default(),
            previous: [0; MAX_PLAYERS],
            frame: 0,
        }
    }

    fn advance(&mut self, buttons: [u16; MAX_PLAYERS], stereo: &mut [i16]) {
        self.app.clock(Duration::from_nanos(16_666_667));
        let held = input::apply(&mut self.app, &self.previous, &buttons);
        self.previous = buttons;
        self.app.tick(&held);
        self.mixer.update(self.app.sound, &self.app.sounds);
        self.app.sounds.clear();
        let mut mono = [0.0; AUDIO_FRAMES];
        self.mixer.mix(&mut mono);
        for (sample, pair) in mono.iter().zip(stereo.chunks_exact_mut(2)) {
            pair.fill((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
        }
        self.frame = (self.frame + 1) % (3600 * FPS as u32);
    }
}

#[derive(Clone, Copy, Default)]
struct Callbacks {
    env: Option<Environment>,
    video: Option<Video>,
    audio: Option<Audio>,
    audio_batch: Option<AudioBatch>,
    poll: Option<Poll>,
    input: Option<Input>,
    framebuffer: Option<Framebuffer>,
    proc_address: Option<ProcAddress>,
}

#[derive(Default)]
struct Core {
    callbacks: Callbacks,
    game: Option<Game>,
    renderer: Option<Renderer>,
    graphics_failed: bool,
    context_was_reset: bool,
    port_devices: [Option<bool>; MAX_PLAYERS],
}

static CORE: LazyLock<Mutex<Core>> = LazyLock::new(|| Mutex::new(Core::default()));

fn core() -> std::sync::MutexGuard<'static, Core> {
    CORE.lock().unwrap_or_else(|e| e.into_inner())
}

#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn retro_set_environment(env: Option<Environment>) {
    core().callbacks.env = env;
    if let Some(env) = env {
        unsafe { env(SET_SUPPORT_NO_GAME, (&mut true as *mut bool).cast()) };
        video::register_option(env);
    }
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: Option<Video>) {
    core().callbacks.video = cb;
}
#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: Option<Audio>) {
    core().callbacks.audio = cb;
}
#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: Option<AudioBatch>) {
    core().callbacks.audio_batch = cb;
}
#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: Option<Poll>) {
    core().callbacks.poll = cb;
}
#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: Option<Input>) {
    core().callbacks.input = cb;
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(port: u32, device: u32) {
    let mut state = core();
    if let Some(slot) = state.port_devices.get_mut(port as usize) {
        let enabled = matches!(device & 0xff, input::JOYPAD | input::ANALOG);
        *slot = Some(enabled);
        if !enabled {
            if let Some(game) = &mut state.game {
                game.previous[port as usize] = 0;
            }
        }
    }
}

/// # Safety
/// A non-null `info` must point to writable storage for one `retro_system_info`.
#[no_mangle]
pub unsafe extern "C" fn retro_get_system_info(info: *mut SystemInfo) {
    if !info.is_null() {
        info.write(SystemInfo {
            library_name: c"Super Dr Meerkat".as_ptr(),
            library_version: concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast(),
            valid_extensions: c"".as_ptr(),
            need_fullpath: false,
            block_extract: false,
        });
    }
}

/// # Safety
/// A non-null `info` must point to writable storage for one `retro_system_av_info`.
#[no_mangle]
pub unsafe extern "C" fn retro_get_system_av_info(info: *mut AvInfo) {
    if info.is_null() {
        return;
    }
    let geometry = core()
        .game
        .as_ref()
        .map_or_else(|| Geometry::new(Resolution::FULL_HD), |g| g.geometry);
    info.write(geometry.av_info());
}

fn path_from_c(path: *const c_char) -> Option<PathBuf> {
    if path.is_null() {
        return None;
    }
    let path = unsafe { CStr::from_ptr(path) };
    if path.is_empty() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(std::ffi::OsStr::from_bytes(path.to_bytes()).into())
    }
    #[cfg(not(unix))]
    path.to_str().ok().map(PathBuf::from)
}

fn save_directory(env: Environment) -> Option<PathBuf> {
    let mut path: *const c_char = ptr::null();
    if unsafe { env(GET_SAVE_DIRECTORY, (&mut path as *mut *const c_char).cast()) } {
        path_from_c(path)
    } else {
        None
    }
}

extern "C" fn context_destroy() {
    core().renderer = None;
}

extern "C" fn context_reset() {
    let get_proc = {
        let mut state = core();
        if let Some(old) = state.renderer.take() {
            old.abandon();
        }
        state.callbacks.proc_address
    };
    let Some(get_proc) = get_proc else {
        return;
    };
    let renderer = std::panic::catch_unwind(|| {
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|s| {
                get_proc(s.as_ptr()).map_or(ptr::null(), |f| f as *const c_void)
            })
        };
        Renderer::new(gl)
    })
    .unwrap_or_else(|_| Err("Unable to load the frontend's GL context".into()));
    let mut state = core();
    state.graphics_failed = renderer.is_err();
    match renderer {
        Ok(renderer) => {
            state.renderer = Some(renderer);
            state.context_was_reset = true;
        }
        Err(e) => eprintln!("Super Dr Meerkat: {e}"),
    }
}

fn request_graphics(env: Environment) -> Result<(), String> {
    let mut preferred = 0u32;
    let can_switch = unsafe { env(GET_PREFERRED_HW_RENDER, (&mut preferred as *mut u32).cast()) };
    let mut candidates = Vec::new();
    match preferred {
        // Older RetroArch releases report "gl" for both desktop GL and GLES builds.
        OPENGL => candidates.extend([OPENGL, GLES2]),
        OPENGL_CORE | GLES2 | GLES3 => candidates.push(preferred),
        GLES_VERSION => candidates.push(GLES3),
        _ => {}
    }
    if can_switch || preferred == 0 {
        let order = if cfg!(any(
            target_arch = "arm",
            all(target_arch = "aarch64", target_os = "linux")
        )) {
            [GLES2, OPENGL_CORE, OPENGL]
        } else {
            [OPENGL_CORE, OPENGL, GLES2]
        };
        for kind in order {
            if !candidates.contains(&kind) {
                candidates.push(kind);
            }
        }
    }
    for kind in candidates {
        let mut request = HwRender {
            context_type: kind,
            context_reset: Some(context_reset),
            context_destroy: Some(context_destroy),
            bottom_left_origin: true,
            version_major: if kind == OPENGL_CORE { 3 } else { 0 },
            version_minor: if kind == OPENGL_CORE { 2 } else { 0 },
            ..HwRender::default()
        };
        if unsafe { env(SET_HW_RENDER, (&mut request as *mut HwRender).cast()) }
            && request.get_proc_address.is_some()
            && request.get_current_framebuffer.is_some()
        {
            let mut state = core();
            state.callbacks.framebuffer = request.get_current_framebuffer;
            state.callbacks.proc_address = request.get_proc_address;
            state.graphics_failed = false;
            return Ok(());
        }
    }
    Err("The core requires OpenGL or OpenGL ES. Select the gl or glcore video driver.".into())
}

fn load() -> Result<(), String> {
    let env = core()
        .callbacks
        .env
        .ok_or("missing libretro environment callback")?;
    let save_dir = save_directory(env);
    input::describe(env);
    let mut xrgb8888 = 1u32;
    unsafe { env(SET_PIXEL_FORMAT, (&mut xrgb8888 as *mut u32).cast()) };
    request_graphics(env)?;
    let geometry = Geometry::new(video::resolution(env));
    core().game = Some(Game::new(geometry, save_dir));
    Ok(())
}

#[no_mangle]
pub extern "C" fn retro_load_game(_info: *const GameInfo) -> bool {
    match load() {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Super Dr Meerkat: {error}");
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_init() {}

#[no_mangle]
pub extern "C" fn retro_reset() {
    let mut state = core();
    if let Some(mut game) = state.game.take() {
        game.app.scores.flush();
        state.game = Some(Game::new(game.geometry, game.save_dir));
    }
}

#[no_mangle]
pub extern "C" fn retro_unload_game() {
    if let Some(mut game) = core().game.take() {
        game.app.scores.flush();
    }
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    retro_unload_game();
    let mut state = core();
    if let Some(old) = state.renderer.take() {
        old.abandon();
    }
    *state = Core::default();
}

#[no_mangle]
pub extern "C" fn retro_run() {
    let (env, geometry, context_reset) = {
        let mut state = core();
        let reset = std::mem::take(&mut state.context_was_reset);
        let Some(game) = &state.game else {
            return;
        };
        (state.callbacks.env, game.geometry, reset)
    };
    // Resizing can call context_destroy and context_reset before the frontend returns.
    let geometry = env
        .and_then(|env| geometry.update(env, context_reset))
        .unwrap_or(geometry);
    let cb = core().callbacks;
    if let Some(poll) = cb.poll {
        unsafe { poll() };
    }
    let devices = core().port_devices;
    let enabled = input::enabled_ports(env, &devices);
    let buttons = std::array::from_fn(|seat| match cb.input {
        Some(input) if enabled[seat] => input::read(input, seat as u32),
        _ => 0,
    });
    let framebuffer = cb.framebuffer.map(|get| unsafe { get() } as u32);
    let mut audio = [0i16; AUDIO_FRAMES * 2];
    let (width, height) = (geometry.base_width, geometry.base_height);
    let (rendered, quit) = {
        let mut state = core();
        let Core {
            game,
            renderer,
            graphics_failed,
            ..
        } = &mut *state;
        let Some(game) = game else {
            return;
        };
        game.geometry = geometry;
        game.app.set_enabled_seats(enabled);
        game.advance(buttons, &mut audio);
        let mut rendered = false;
        if let (Some(renderer), Some(fbo)) = (renderer, framebuffer) {
            renderer.canvas.begin(width, height);
            game.ui.draw(
                &mut renderer.canvas,
                &game.app,
                None,
                game.frame as f32 / FPS as f32,
            );
            renderer.bind_framebuffer(fbo);
            renderer.present(width, height);
            rendered = true;
        }
        (rendered, *graphics_failed || game.app.quit)
    };
    unsafe {
        if let Some(video) = cb.video {
            let frame = if rendered {
                usize::MAX as *const c_void
            } else {
                ptr::null()
            };
            video(frame, width, height, 0);
        }
        if let Some(batch) = cb.audio_batch {
            batch(audio.as_ptr(), AUDIO_FRAMES);
        } else if let Some(sample) = cb.audio {
            for lr in audio.chunks_exact(2) {
                sample(lr[0], lr[1]);
            }
        }
        if let (true, Some(env)) = (quit, cb.env) {
            env(SHUTDOWN, ptr::null_mut());
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_get_region() -> u32 {
    0
}
#[no_mangle]
pub extern "C" fn retro_load_game_special(_: u32, _: *const GameInfo, _: usize) -> bool {
    false
}
#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    0
}
#[no_mangle]
pub extern "C" fn retro_serialize(_: *mut c_void, _: usize) -> bool {
    false
}
#[no_mangle]
pub extern "C" fn retro_unserialize(_: *const c_void, _: usize) -> bool {
    false
}
#[no_mangle]
pub extern "C" fn retro_get_memory_data(_: u32) -> *mut c_void {
    ptr::null_mut()
}
#[no_mangle]
pub extern "C" fn retro_get_memory_size(_: u32) -> usize {
    0
}
#[no_mangle]
pub extern "C" fn retro_cheat_reset() {}
#[no_mangle]
pub extern "C" fn retro_cheat_set(_: u32, _: bool, _: *const c_char) {}
