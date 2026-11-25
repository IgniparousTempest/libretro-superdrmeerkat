mod ffi;
mod input;

use ffi::*;
use std::{
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    ptr,
    sync::{LazyLock, Mutex},
    time::Duration,
};
use super_dr_meerkat::{
    app::{App, Command},
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
    config: Config,
    save_dir: Option<PathBuf>,
    app: App,
    ui: Ui,
    mixer: Mixer,
    previous: [u16; MAX_PLAYERS],
    frame: u32,
}

impl Game {
    fn new(config: Config, save_dir: Option<PathBuf>) -> Self {
        let scores = save_dir.as_ref().map_or_else(HighScores::cabinet, |dir| {
            HighScores::open(dir.join("super-dr-meerkat/high-scores.toml"))
        });
        Self {
            app: App::new(&config, true, scores),
            mixer: Mixer::new(SAMPLE_RATE as i32, config.sound),
            config,
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
    disconnected: [bool; MAX_PLAYERS],
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
    if let Some(disconnected) = state.disconnected.get_mut(port as usize) {
        *disconnected = !matches!(device & 0xff, input::JOYPAD | input::ANALOG);
        if *disconnected {
            if let Some(game) = &mut state.game {
                game.previous[port as usize] = 0;
                if game.app.seats.contains(&(port as usize)) {
                    game.app.command(Command::Pause);
                }
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
            valid_extensions: c"toml".as_ptr(),
            need_fullpath: true,
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
    let Resolution { width, height } = core()
        .game
        .as_ref()
        .map_or_else(|| Config::default().resolution, |g| g.config.resolution);
    info.write(AvInfo {
        geometry: Geometry {
            base_width: width,
            base_height: height,
            max_width: width,
            max_height: height,
            aspect_ratio: width as f32 / height as f32,
        },
        timing: Timing {
            fps: FPS as f64,
            sample_rate: SAMPLE_RATE as f64,
        },
    });
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

fn directory(env: Environment, command: u32) -> Option<PathBuf> {
    let mut path: *const c_char = ptr::null();
    if unsafe { env(command, (&mut path as *mut *const c_char).cast()) } {
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
        Ok(renderer) => state.renderer = Some(renderer),
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

fn load(info: Option<&GameInfo>) -> Result<(), String> {
    let env = core()
        .callbacks
        .env
        .ok_or("missing libretro environment callback")?;
    let config_path = match info {
        Some(info) => Some(path_from_c(info.path).ok_or("missing cabinet config path")?),
        None => directory(env, GET_SYSTEM_DIRECTORY)
            .map(|p| p.join("super-dr-meerkat.toml"))
            .filter(|p| !matches!(p.try_exists(), Ok(false))),
    };
    let config = match config_path {
        Some(path) => Config::load(Some(path))?,
        None => Config::default(),
    };
    let save_dir = directory(env, GET_SAVE_DIRECTORY);
    input::describe(env, config.max_players);
    let mut xrgb8888 = 1u32;
    unsafe { env(SET_PIXEL_FORMAT, (&mut xrgb8888 as *mut u32).cast()) };
    request_graphics(env)?;
    core().game = Some(Game::new(config, save_dir));
    Ok(())
}

/// # Safety
/// A non-null `info` and its NUL-terminated path must be readable for this call.
#[no_mangle]
pub unsafe extern "C" fn retro_load_game(info: *const GameInfo) -> bool {
    match load(info.as_ref()) {
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
        state.game = Some(Game::new(game.config, game.save_dir));
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
    let (cb, disconnected, cap) = {
        let state = core();
        let Some(game) = &state.game else {
            return;
        };
        (state.callbacks, state.disconnected, game.app.cap)
    };
    if let Some(poll) = cb.poll {
        unsafe { poll() };
    }
    let buttons = std::array::from_fn(|seat| match cb.input {
        Some(input) if seat < cap && !disconnected[seat] => input::read(input, seat as u32),
        _ => 0,
    });
    let framebuffer = cb.framebuffer.map(|get| unsafe { get() } as u32);
    let mut audio = [0i16; AUDIO_FRAMES * 2];
    let (resolution, rendered, quit) = {
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
        game.advance(buttons, &mut audio);
        let size = game.config.resolution;
        let mut rendered = false;
        if let (Some(renderer), Some(fbo)) = (renderer, framebuffer) {
            renderer.canvas.begin(size.width, size.height);
            game.ui.draw(
                &mut renderer.canvas,
                &game.app,
                None,
                game.frame as f32 / FPS as f32,
            );
            renderer.bind_framebuffer(fbo);
            renderer.present(size.width, size.height);
            rendered = true;
        }
        (size, rendered, *graphics_failed || game.app.quit)
    };
    unsafe {
        if let Some(video) = cb.video {
            let frame = if rendered {
                usize::MAX as *const c_void
            } else {
                ptr::null()
            };
            video(frame, resolution.width, resolution.height, 0);
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
