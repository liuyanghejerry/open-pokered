//! Generic pixels + winit game shell: runs a [`GameLoop`] game in a browser
//! canvas (wasm32) or a native fallback window.
//!
//! This is the web counterpart to `dotzuki_renderer::window::run` (which is
//! native-only): the same pixels+winit stack and the same GB-frame pacing
//! (4194304 Hz / 70224 cycles ≈ 59.7275 Hz), plus the browser plumbing a wasm
//! game needs — replacing a placeholder `<canvas>` in the host page, tracking
//! its parent's width on window resize, an optional FPS-counter element, and
//! `requestAnimationFrame`-driven polling (`ControlFlow::Poll` +
//! `EventLoop::spawn`; without `Poll`, winit 0.30's web backend only ticks
//! when an event arrives, producing ~5 FPS).
//!
//! Game-specific wiring stays in the game crate: it builds a
//! [`GameShellConfig`], implements [`GameLoop`], and — when the host page
//! needs runtime control over the live game (join/leave a link session, …) —
//! keeps its own clone of the `Rc<RefCell<G>>` it passes to [`run_game`].

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use error_iter::ErrorIter as _;
use log::error;
use pixels::{PixelsBuilder, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

use dotzuki_renderer::FbSurface;
use dotzuki_renderer::input::InputState;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

/// GB VBlank: 4194304 Hz / 70224 cycles ≈ 59.7275 Hz (≈16.7427 ms per frame).
#[cfg(not(target_arch = "wasm32"))]
const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);
/// Milliseconds per GB frame (≈16.7427 ms).
#[cfg(target_arch = "wasm32")]
const FRAME_MS: f64 = 1_000.0 * 70_224.0 / 4_194_304.0;

/// A game driven by [`run_game`]. Mirrors
/// `dotzuki_renderer::window::GameLoop` (native-only); kept as a separate
/// trait so this crate stays wasm-compatible.
pub trait GameLoop {
    /// The framebuffer type the game draws into: either the engine's RGBA
    /// [`dotzuki_renderer::FrameBuffer`] (true-color games) or the indexed
    /// [`dotzuki_renderer::RgbaIndexedFrameBuffer`] (fixed-palette games).
    type Fb: FbSurface;

    /// Called once per GB frame. Process input before returning.
    fn update(&mut self, input: &InputState);

    /// Called once per redraw. Draw the current screen into the framebuffer.
    fn draw(&mut self, fb: &mut Self::Fb);

    /// Should the loop exit?
    fn should_exit(&self) -> bool {
        false
    }

    /// Called on every key press. Browsers count key presses as user
    /// gestures, so this is the hook to resume a suspended `AudioContext`.
    fn on_user_gesture(&self) {}
}

/// Window/canvas configuration for [`run_game`].
pub struct GameShellConfig {
    /// Window title.
    pub title: String,
    /// Integer scale factor for the initial window size.
    pub scale: u32,
    /// Whether the window is user-resizable.
    pub resizable: bool,
    /// Logical framebuffer width in pixels (e.g. 160 for GB-resolution games,
    /// 240 for GBA-resolution games).
    pub width: u32,
    /// Logical framebuffer height in pixels (e.g. 144 / 160).
    pub height: u32,
    /// (wasm only) DOM id of the placeholder `<canvas>` in the host page: the
    /// shell replaces it with the winit canvas and tracks its parent's width
    /// when sizing the surface.
    pub canvas_id: String,
    /// (wasm only) DOM id of an element updated with `"NN FPS"` roughly every
    /// 30 frames; `None` disables the FPS counter.
    pub fps_element_id: Option<String>,
}

impl GameShellConfig {
    /// A config with the conventional defaults: resizable, canvas id
    /// `"game-canvas"`, FPS counter element `"fps-counter"` (both no-op when
    /// the host page lacks the elements).
    pub fn new(title: impl Into<String>, width: u32, height: u32, scale: u32) -> Self {
        GameShellConfig {
            title: title.into(),
            scale,
            resizable: true,
            width,
            height,
            canvas_id: "game-canvas".to_string(),
            fps_element_id: Some("fps-counter".to_string()),
        }
    }
}

/// Errors [`run_game`] can report before or while entering the event loop.
#[derive(Debug)]
pub enum GameShellError {
    /// Failed to create or run the winit event loop.
    EventLoop(String),
    /// Failed to create the window.
    WindowCreation(String),
    /// Failed to create the pixels framebuffer.
    PixelBuffer(pixels::Error),
}

impl std::fmt::Display for GameShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameShellError::EventLoop(e) => write!(f, "event loop error: {}", e),
            GameShellError::WindowCreation(e) => write!(f, "window creation failed: {}", e),
            GameShellError::PixelBuffer(e) => write!(f, "pixel buffer creation failed: {}", e),
        }
    }
}

impl std::error::Error for GameShellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GameShellError::PixelBuffer(e) => Some(e),
            _ => None,
        }
    }
}

impl From<pixels::Error> for GameShellError {
    fn from(e: pixels::Error) -> Self {
        GameShellError::PixelBuffer(e)
    }
}

/// Run `game` to completion (native) or hand it to the browser's event loop
/// (wasm, where this returns immediately after `EventLoop::spawn`).
///
/// The game is shared behind an `Rc<RefCell>` so the caller can keep a
/// handle: on wasm the host page drives the live game between frames through
/// its own exported functions (borrowing is safe — JS is single-threaded and
/// those calls never land inside `update`/`draw`).
///
/// Async because pixels' surface creation is async on wasm; native callers
/// wrap it in `pollster::block_on`.
pub async fn run_game<G: GameLoop + 'static>(
    config: GameShellConfig,
    game: Rc<RefCell<G>>,
) -> Result<(), GameShellError> {
    let event_loop = EventLoop::new().map_err(|e| GameShellError::EventLoop(e.to_string()))?;
    // Drive the loop continuously (see the module docs).
    event_loop.set_control_flow(ControlFlow::Poll);

    let (fb_width, fb_height) = (config.width, config.height);
    let window = {
        let size = LogicalSize::new(
            (fb_width * config.scale) as f64,
            (fb_height * config.scale) as f64,
        );
        #[allow(deprecated)]
        Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&config.title)
                        .with_inner_size(size)
                        .with_min_inner_size(LogicalSize::new(fb_width as f64, fb_height as f64))
                        .with_resizable(config.resizable),
                )
                .map_err(|e| GameShellError::WindowCreation(e.to_string()))?,
        )
    };

    #[cfg(target_arch = "wasm32")]
    install_canvas(&window, &config);

    let mut pixels = {
        #[cfg(not(target_arch = "wasm32"))]
        let window_size = window.inner_size();

        #[cfg(target_arch = "wasm32")]
        let window_size = get_window_size(&config).to_physical::<u32>(window.scale_factor());

        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, Arc::clone(&window));
        let builder = PixelsBuilder::new(fb_width, fb_height, surface_texture);

        #[cfg(target_arch = "wasm32")]
        let builder = {
            use pixels::wgpu::Backends;

            let texture_format = pixels::wgpu::TextureFormat::Rgba8Unorm;

            builder
                .texture_format(texture_format)
                .surface_texture_format(texture_format)
                // Some browsers expose partial WebGPU limits and return
                // `undefined` for numeric fields, which can panic in
                // wasm-bindgen. Keep wasm on WebGL for broad compatibility.
                .wgpu_backend(Backends::GL)
        };

        builder.build_async().await?
    };

    let mut frame_buffer = G::Fb::new_screen(fb_width, fb_height);
    let mut input = InputState::new();

    #[cfg(not(target_arch = "wasm32"))]
    let mut next_frame_time = Instant::now();

    // Negative sentinel means "not yet initialized".
    #[cfg(target_arch = "wasm32")]
    let mut next_frame_ms: f64 = -1.0;
    #[cfg(target_arch = "wasm32")]
    let mut fps_frame_count: u32 = 0;
    #[cfg(target_arch = "wasm32")]
    let mut fps_last_time: f64 = 0.0;
    #[cfg(target_arch = "wasm32")]
    let fps_element_id = config.fps_element_id.clone();

    let event_handler = move |event, elwt: &winit::event_loop::ActiveEventLoop| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::RedrawRequested => {
                game.borrow_mut().draw(&mut frame_buffer);
                frame_buffer.present_into(pixels.frame_mut());
                if let Err(err) = pixels.render() {
                    log_error("pixels.render", err);
                    elwt.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        log_error("pixels.resize_surface", err);
                        elwt.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if let PhysicalKey::Code(keycode) = key_event.physical_key {
                    let pressed = key_event.state == ElementState::Pressed;
                    if pressed && keycode == KeyCode::Escape {
                        elwt.exit();
                        return;
                    }
                    if pressed {
                        game.borrow().on_user_gesture();
                    }
                    input.set_from_keycode(keycode, pressed);
                }
            }
            _ => {}
        },
        Event::AboutToWait => {
            #[cfg(target_arch = "wasm32")]
            {
                let now = web_sys::window()
                    .and_then(|w| w.performance())
                    .map(|p| p.now())
                    .unwrap_or(0.0);

                // First call: initialise timing references
                if next_frame_ms < 0.0 {
                    next_frame_ms = now;
                    fps_last_time = now;
                }

                if now >= next_frame_ms {
                    game.borrow_mut().update(&input);
                    input.begin_frame();
                    if game.borrow().should_exit() {
                        elwt.exit();
                        return;
                    }
                    window.request_redraw();

                    fps_frame_count += 1;

                    // Refresh the FPS overlay roughly every 30 frames
                    if fps_frame_count >= 30 {
                        let elapsed = now - fps_last_time;
                        if elapsed > 0.0 {
                            if let Some(ref fps_id) = fps_element_id {
                                let fps = fps_frame_count as f64 * 1_000.0 / elapsed;
                                if let Some(el) = web_sys::window()
                                    .and_then(|w| w.document())
                                    .and_then(|d| d.get_element_by_id(fps_id))
                                {
                                    el.set_text_content(Some(&format!("{:.0} FPS", fps)));
                                }
                            }
                        }
                        fps_frame_count = 0;
                        fps_last_time = now;
                    }

                    next_frame_ms += FRAME_MS;
                    // Prevent spiral of death if we fall too far behind
                    if next_frame_ms < now {
                        next_frame_ms = now + FRAME_MS;
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let now = Instant::now();
                if now >= next_frame_time {
                    game.borrow_mut().update(&input);
                    input.begin_frame();
                    if game.borrow().should_exit() {
                        elwt.exit();
                        return;
                    }
                    window.request_redraw();
                    next_frame_time += FRAME_DURATION;
                    if next_frame_time < now {
                        next_frame_time = now + FRAME_DURATION;
                    }
                }
                let sleep_duration = next_frame_time.saturating_duration_since(Instant::now());
                if !sleep_duration.is_zero() {
                    std::thread::sleep(sleep_duration);
                }
            }
        }
        _ => {}
    };

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;

        #[allow(deprecated)]
        event_loop.spawn(event_handler);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[allow(deprecated)]
        let res = event_loop.run(event_handler);
        res.map_err(|e| GameShellError::EventLoop(e.to_string()))
    }
}

/// The size the canvas should take: its parent's width (falling back to the
/// viewport), clamped to the viewport and the `scale`-d framebuffer size,
/// keeping the framebuffer aspect ratio.
#[cfg(target_arch = "wasm32")]
fn get_window_size(config: &GameShellConfig) -> LogicalSize<f64> {
    let client_window = web_sys::window().unwrap();
    let vw = client_window.inner_width().unwrap().as_f64().unwrap();
    let max_w = (config.width * config.scale) as f64;
    let available_w = client_window
        .document()
        .and_then(|doc| doc.get_element_by_id(&config.canvas_id))
        .and_then(|canvas| canvas.parent_element())
        .map(|parent| parent.client_width() as f64)
        .filter(|width| *width > 0.0)
        .unwrap_or(vw);

    let w = available_w.min(vw).min(max_w).max(1.0);
    let h = w * config.height as f64 / config.width as f64;
    LogicalSize::new(w, h)
}

/// Replace the host page's placeholder canvas with winit's, re-fit on browser
/// window resize, and apply the initial fit.
#[cfg(target_arch = "wasm32")]
fn install_canvas(window: &Arc<Window>, config: &GameShellConfig) {
    use wasm_bindgen::JsCast;
    use winit::platform::web::WindowExtWebSys;

    let game_canvas = window.canvas().expect("winit web canvas");
    game_canvas.set_id(&config.canvas_id);

    let old_canvas = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.get_element_by_id(&config.canvas_id))
        .expect("couldn't find placeholder canvas element");

    let parent = old_canvas.parent_node().expect("canvas has no parent");
    parent
        .replace_child(&web_sys::Element::from(game_canvas), &old_canvas)
        .expect("couldn't replace canvas element");

    let resize_window = Arc::clone(window);
    let resize_config = GameShellConfig {
        title: String::new(),
        scale: config.scale,
        resizable: config.resizable,
        width: config.width,
        height: config.height,
        canvas_id: config.canvas_id.clone(),
        fps_element_id: None,
    };
    let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
        let _ = resize_window.request_inner_size(get_window_size(&resize_config));
    }) as Box<dyn FnMut(_)>);
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();

    let _ = window.request_inner_size(get_window_size(config));
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }
}
