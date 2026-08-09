//! Pokémon Red/Blue Android native build (winit + pixels/wgpu).
//! Uses NativeActivity for rendering + Kotlin GamepadView overlay via JNI.

use std::sync::Arc;
use std::time::{Duration, Instant};

use log::error;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::input::{GbButton, InputState};
use pokered_renderer::{FrameBuffer, RenderConfig, Rgba};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);
const SCREEN_WIDTH: u32 = 160;
const SCREEN_HEIGHT: u32 = 144;
const SCALE: u32 = 3;
const LOADING_BAR_H: usize = 4;

// ── JNI input bridge (Kotlin GamepadView → Rust) ─────────────────────

#[cfg(target_os = "android")]
mod jni_input {
    use std::sync::{Mutex, OnceLock};
    use pokered_renderer::input::{GbButton, InputState};

    static JNI_INPUT: OnceLock<Mutex<InputState>> = OnceLock::new();

    pub fn ensure_init() {
        JNI_INPUT.get_or_init(|| Mutex::new(InputState::new()));
    }

    pub fn take() -> InputState {
        let lock = JNI_INPUT.get().expect("JNI_INPUT not initialized");
        let mut state = lock.lock().unwrap();
        let result = state.clone();
        state.begin_frame();
        result
    }

    fn int_to_gb_button(button: jni::sys::jint) -> Option<GbButton> {
        match button {
            0 => Some(GbButton::A),
            1 => Some(GbButton::B),
            2 => Some(GbButton::Select),
            3 => Some(GbButton::Start),
            4 => Some(GbButton::Right),
            5 => Some(GbButton::Left),
            6 => Some(GbButton::Up),
            7 => Some(GbButton::Down),
            _ => None,
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_pokered_app_NativeBridge_pressButton(
        _env: jni::JNIEnv,
        _class: jni::objects::JClass,
        button: jni::sys::jint,
    ) {
        if let Some(btn) = int_to_gb_button(button) {
            if let Some(input) = JNI_INPUT.get() {
                input.lock().unwrap().press(btn);
            }
        }
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_pokered_app_NativeBridge_releaseButton(
        _env: jni::JNIEnv,
        _class: jni::objects::JClass,
        button: jni::sys::jint,
    ) {
        if let Some(btn) = int_to_gb_button(button) {
            if let Some(input) = JNI_INPUT.get() {
                input.lock().unwrap().release(btn);
            }
        }
    }
}

// ── Android entry point ──────────────────────────────────────────────

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use std::sync::OnceLock;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    std::env::set_var("WGPU_BACKEND", "vulkan");

    static LOGGER: OnceLock<()> = OnceLock::new();
    LOGGER.get_or_init(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("pokered"),
        );
    });

    log::info!("Pokémon Red/Blue - Android starting");

    jni_input::ensure_init();

    let event_loop = match EventLoop::builder().with_android_app(app).build() {
        Ok(el) => el,
        Err(e) => {
            log::error!("Failed to create Android event loop: {}", e);
            return;
        }
    };

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut game = AndroidGame::new();
    if let Err(e) = event_loop.run_app(&mut game) {
        log::error!("Event loop error: {}", e);
    }
}

#[cfg(not(target_os = "android"))]
fn main() {
    env_logger::init();
    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = AndroidGame::new();
    event_loop.run_app(&mut app).unwrap();
}

// ── Game state ──────────────────────────────────────────────────────

struct AndroidGame {
    game: Option<PokemonGame>,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    frame_buffer: FrameBuffer,
    input: InputState,
    next_frame: Instant,
    loading: bool,
    loading_tick: u32,
}

impl AndroidGame {
    fn new() -> Self {
        Self {
            game: None,
            window: None,
            pixels: None,
            frame_buffer: FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE),
            input: InputState::new(),
            next_frame: Instant::now(),
            loading: true,
            loading_tick: 0,
        }
    }
}

impl ApplicationHandler for AndroidGame {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let size = LogicalSize::new(
            (SCREEN_WIDTH * SCALE) as f64,
            (SCREEN_HEIGHT * SCALE) as f64,
        );

        #[allow(deprecated)]
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("Pokémon Red - Rust")
                .with_inner_size(size)
                .with_min_inner_size(LogicalSize::new(
                    SCREEN_WIDTH as f64,
                    SCREEN_HEIGHT as f64,
                ))
                .with_resizable(true),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("Failed to create window: {}", e);
                event_loop.exit();
                return;
            }
        };

        let logical_w = (SCREEN_WIDTH * SCALE) as u32;
        let logical_h = (SCREEN_HEIGHT * SCALE) as u32;
        let surface = SurfaceTexture::new(logical_w, logical_h, Arc::clone(&window));

        let pixels = match PixelsBuilder::new(SCREEN_WIDTH, SCREEN_HEIGHT, surface)
            .surface_texture_format(pixels::wgpu::TextureFormat::Rgba8Unorm)
            .build()
        {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to create pixel buffer: {}", e);
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.pixels = Some(pixels);
        self.next_frame = Instant::now();
        log::info!("Window and GPU surface created");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                if let Some(ref mut p) = self.pixels {
                    let w = physical_size.width.max(1);
                    let h = physical_size.height.max(1);
                    if let Err(err) = p.resize_surface(w, h) {
                        log_error("pixels.resize_surface", err);
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut p) = self.pixels {
                    if let Some(ref mut game) = self.game {
                        game.draw(&mut self.frame_buffer);
                    } else {
                        draw_loading_screen(&mut self.frame_buffer, self.loading_tick);
                    }
                    p.frame_mut().copy_from_slice(&self.frame_buffer.data);
                    if let Err(err) = p.render() {
                        log_error("pixels.render", err);
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if let PhysicalKey::Code(kc) = key_event.physical_key {
                    let pressed = key_event.state == ElementState::Pressed;
                    if pressed && kc == KeyCode::Escape {
                        event_loop.exit();
                        return;
                    }
                    self.input.set_from_keycode(kc, pressed);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_frame {
            if self.loading {
                self.loading_tick = self.loading_tick.wrapping_add(1);
                if self.loading_tick == 3 {
                    log::info!("Initializing game...");
                    self.game = Some(PokemonGame::new(GameVersion::Red));
                    self.loading = false;
                    log::info!("Game initialized, entering game loop");
                }
            }

            #[cfg(target_os = "android")]
            {
                let jni_state = jni_input::take();
                for btn in &GbButton::ALL {
                    if jni_state.is_held(*btn) {
                        self.input.press(*btn);
                    } else {
                        self.input.release(*btn);
                    }
                }
            }

            if let Some(ref mut game) = self.game {
                game.update(&self.input);
                self.input.begin_frame();

                if game.should_exit() {
                    event_loop.exit();
                    return;
                }
            }

            if let Some(ref w) = self.window {
                w.request_redraw();
            }

            self.next_frame += FRAME_DURATION;
            if self.next_frame < now {
                self.next_frame = now + FRAME_DURATION;
            }
        }
    }
}

fn draw_loading_screen(fb: &mut FrameBuffer, tick: u32) {
    let h = SCREEN_HEIGHT as usize;
    let w = SCREEN_WIDTH as usize;
    let dark = [40, 40, 50, 255u8];
    let accent = [255, 80, 80, 255u8];
    let bar_w = 80;
    let bar_x = (w - bar_w) / 2;
    let bar_y = h / 2 + 20;

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            fb.data[idx..idx + 4].copy_from_slice(&dark);
        }
    }

    let progress = (tick as usize / 6) % (bar_w + 10);
    let fill = if progress > bar_w { bar_w } else { progress };
    let pulse = ((tick as f32 * 0.1).sin() * 0.3 + 0.7) as f32;
    let pulse_color = [
        (accent[0] as f32 * pulse) as u8,
        (accent[1] as f32 * pulse) as u8,
        (accent[2] as f32 * pulse) as u8,
        255,
    ];

    for y in bar_y..(bar_y + LOADING_BAR_H) {
        for x in bar_x..(bar_x + fill) {
            if x >= w { continue; }
            let idx = (y * w + x) * 4;
            fb.data[idx..idx + 4].copy_from_slice(&pulse_color);
        }
    }
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    let mut source = std::error::Error::source(&err);
    while let Some(s) = source {
        error!("  Caused by: {s}");
        source = std::error::Error::source(s);
    }
}
