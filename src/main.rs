mod audio;
mod input;
#[cfg(test)]
mod tests;

use app::{App, Command};
use audio::Audio;
use config::{Config, Resolution};
use input::Pads;
use render::Renderer;
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Scancode,
    mouse::MouseButton,
    pixels::PixelFormatEnum,
    video::{DisplayMode, FullscreenType, GLContext, SwapInterval, Window},
    VideoSubsystem,
};
use std::{
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};
use super_dr_meerkat::{app, config, render, scores, ui};
use ui::Ui;

fn settings() -> Result<Option<Config>, String> {
    let mut args = std::env::args_os().skip(1);
    let mut path = None;
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("Super Dr Meerkat\n\nUsage: super-dr-meerkat [--config PATH]\n\nKeys 5-8 insert credits for P1-P4. Keys 1-4 start those seats. H opens the controls.\nThe default config is super-dr-meerkat.toml in the working directory or beside the executable.");
            return Ok(None);
        } else if arg == "--version" {
            println!("Super Dr Meerkat {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        } else if arg == "--config" && path.is_none() {
            path = Some(PathBuf::from(
                args.next().ok_or("--config needs a file path")?,
            ));
        } else {
            return Err(format!(
                "Unknown or duplicate argument {:?}. Use --help for usage.",
                arg
            ));
        }
    }
    Config::load(path).map(Some)
}

fn open_window(
    video: &VideoSubsystem,
    resolution: Option<Resolution>,
) -> Result<(Window, GLContext), String> {
    let attributes = video.gl_attr();
    // Windows graphics drivers expose desktop OpenGL through SDL's WGL backend.
    #[cfg(windows)]
    attributes.set_context_version(3, 0);
    #[cfg(target_os = "macos")]
    {
        attributes.set_context_profile(sdl2::video::GLProfile::Core);
        attributes.set_context_version(3, 2);
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        attributes.set_context_profile(sdl2::video::GLProfile::GLES);
        attributes.set_context_version(2, 0);
    }
    attributes.set_double_buffer(true);
    attributes.set_red_size(8);
    attributes.set_green_size(8);
    attributes.set_blue_size(8);
    attributes.set_depth_size(0);
    attributes.set_stencil_size(0);
    let desktop = video
        .display_usable_bounds(0)
        .or_else(|_| video.display_bounds(0))
        .map(|b| Resolution {
            width: b.width(),
            height: b.height(),
        })
        .unwrap_or_else(|_| resolution.unwrap_or(Resolution::FULL_HD));
    let (w, h) = resolution
        .unwrap_or(desktop)
        .fit_within(desktop.width, desktop.height);
    let mut builder = video.window("Super Dr Meerkat", w, h);
    builder.opengl().resizable().position_centered();
    if resolution.is_none() {
        builder.maximized().allow_highdpi();
    }
    // Retry with a regular framebuffer when the display lacks MSAA.
    let create = |samples| -> Result<(Window, GLContext), String> {
        attributes.set_multisample_buffers(u8::from(samples > 0));
        attributes.set_multisample_samples(samples);
        let window = builder
            .build()
            .map_err(|e| format!("SDL OpenGL window: {e}"))?;
        let context = window
            .gl_create_context()
            .map_err(|e| format!("SDL OpenGL context: {e}"))?;
        window.gl_make_current(&context)?;
        Ok((window, context))
    };
    create(4).or_else(|_| create(0))
}

fn enter_fullscreen(window: &mut Window, resolution: Option<Resolution>) -> Result<(), String> {
    let Some(resolution) = resolution else {
        return window.set_fullscreen(FullscreenType::Desktop);
    };
    let mode = DisplayMode::new(
        PixelFormatEnum::Unknown,
        resolution.width as i32,
        resolution.height as i32,
        0,
    );
    // sdl2 0.38's Window::set_display_mode passes a pointer to a temporary.
    // Keep the mode alive while SDL copies it into the window.
    let raw_mode = mode.to_ll();
    let exact = match unsafe { sdl2::sys::SDL_SetWindowDisplayMode(window.raw(), &raw_mode) } {
        0 => window.display_mode().and_then(|available| {
            // SDL can substitute a larger mode when the requested size is unavailable.
            if (available.w, available.h) == (mode.w, mode.h) {
                window.set_fullscreen(FullscreenType::True)
            } else {
                Err(format!("closest mode is {}x{}", available.w, available.h))
            }
        }),
        _ => Err(sdl2::get_error()),
    };
    exact.or_else(|e| {
        eprintln!(
            "Fullscreen {}x{} unavailable ({e}). Using the desktop resolution.",
            mode.w, mode.h
        );
        window.set_fullscreen(FullscreenType::Desktop)
    })
}

fn run() -> Result<(), String> {
    let Some(config) = settings()? else {
        return Ok(());
    };
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let (mut window, _context) = open_window(&video, config.resolution)?;
    if config.fullscreen {
        enter_fullscreen(&mut window, config.resolution)?;
    }
    let _ = video.gl_set_swap_interval(SwapInterval::VSync);
    // Renderer is dropped before the current GL context and its SDL window.
    let gl =
        unsafe { glow::Context::from_loader_function(|s| video.gl_get_proc_address(s).cast()) };
    let mut renderer = Renderer::new(gl)?;
    let mut audio = Audio::new(&sdl, config.sound)
        .map_err(|e| eprintln!("Audio unavailable: {e}"))
        .ok();
    let mut app = App::new(&config, audio.is_some(), scores::HighScores::cabinet());
    let mut pads = Pads::new(sdl.game_controller()?, config.max_players);
    let mut ui = Ui::default();
    let mut events = sdl.event_pump()?;
    let mut pointer = None;
    let start = Instant::now();
    let mut last = start;
    let mut accumulator = Duration::ZERO;
    let step = Duration::from_secs_f64(1.0 / 60.0);
    while !app.quit {
        let frame = Instant::now();
        let elapsed = frame.duration_since(last);
        accumulator += elapsed.min(step * 8);
        last = frame;
        let previous_screen = app.screen;
        app.clock(elapsed);
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::Window {
                    win_event: WindowEvent::Close,
                    ..
                } => app.command(Command::Quit),
                Event::Window {
                    win_event: WindowEvent::FocusLost,
                    ..
                } => app.command(Command::Pause),
                Event::KeyDown {
                    scancode: Some(Scancode::F11),
                    repeat: false,
                    ..
                } => {
                    if app.press(None) {
                        continue;
                    }
                    let result = if window.fullscreen_state() == FullscreenType::Off {
                        enter_fullscreen(&mut window, config.resolution)
                    } else {
                        window.set_fullscreen(FullscreenType::Off)
                    };
                    if let Err(e) = result {
                        eprintln!("Fullscreen toggle: {e}");
                    }
                }
                Event::KeyDown {
                    scancode: Some(k),
                    repeat: false,
                    ..
                } => {
                    pointer = None;
                    input::key_press(&mut app, k);
                }
                Event::ControllerDeviceAdded { which, .. } => pads.add(which),
                Event::ControllerDeviceRemoved { which, .. } => pads.remove(which, &mut app),
                Event::ControllerButtonDown { which, button, .. } => {
                    pointer = None;
                    pads.press(&mut app, which, button);
                }
                Event::ControllerAxisMotion {
                    which, axis, value, ..
                } => pads.axis(&mut app, which, axis, value),
                Event::MouseMotion { x, y, .. } => {
                    pointer = Some((x, y));
                }
                Event::MouseButtonDown {
                    mouse_btn, x, y, ..
                } => {
                    pointer = Some((x, y));
                    let cmd = if mouse_btn == MouseButton::Left {
                        ui.click(app.screen, point((x, y), window.size(), &renderer))
                    } else {
                        None
                    };
                    let coin = match cmd {
                        Some(Command::Coin(seat)) => Some(seat),
                        _ => None,
                    };
                    if !app.press(coin) {
                        if let Some(cmd) = cmd {
                            app.command(cmd);
                        }
                    }
                }
                _ => {}
            }
        }
        // Time spent in a menu or a paused window never advances a new bottle.
        if app.screen != previous_screen {
            accumulator = Duration::ZERO;
        }
        let held = pads.held(events.keyboard_state());
        while accumulator >= step {
            app.tick(&held);
            accumulator -= step;
        }
        if let Some(audio) = &mut audio {
            audio.update(app.sound, &app.sounds);
        }
        app.sounds.clear();
        let (w, h) = window.drawable_size();
        if w > 0 && h > 0 && !app.quit {
            renderer.canvas.begin(w, h);
            let cursor = pointer.map(|p| point(p, window.size(), &renderer));
            ui.draw(
                &mut renderer.canvas,
                &app,
                cursor,
                (start.elapsed().as_secs_f64() % 3600.0) as f32,
            );
            renderer.present(w, h);
            window.gl_swap_window();
        }
        // Also pace backends that accept swap interval 1 without blocking.
        if let Some(rest) = step.checked_sub(frame.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    Ok(())
}

fn point((x, y): (i32, i32), (w, h): (u32, u32), renderer: &Renderer) -> (f32, f32) {
    (
        x as f32 / w.max(1) as f32 * renderer.canvas.w,
        y as f32 / h.max(1) as f32 * renderer.canvas.h,
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Super Dr Meerkat: {e}");
            ExitCode::FAILURE
        }
    }
}
