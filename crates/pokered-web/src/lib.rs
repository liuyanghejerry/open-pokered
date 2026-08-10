//! Pokémon Red/Blue - Web/WASM Build
//!
//! This crate provides a WebAssembly-compatible build of the game,
//! using pixels + winit with async initialization for wasm32 targets.
//! Also runs natively as a fallback for development.
//!
//! The game logic is shared with pokered-app crate via PokemonGame,
//! ensuring identical behavior between web and native builds.

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

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::input::InputState;
use pokered_renderer::{FbSurface, FrameBuffer, RenderConfig, Rgba};

#[cfg(target_arch = "wasm32")]
use pokered_app::link::broadcast_channel::BroadcastChannelTransport;
#[cfg(target_arch = "wasm32")]
use pokered_core::link::LinkRole;

const SCREEN_W: u32 = 160;
const SCREEN_H: u32 = 144;
const SCALE: u32 = 3;

#[cfg(target_arch = "wasm32")]
fn get_window_size() -> LogicalSize<f64> {
    let client_window = web_sys::window().unwrap();
    let vw = client_window.inner_width().unwrap().as_f64().unwrap();
    let max_w = (SCREEN_W * SCALE) as f64;
    let available_w = client_window
        .document()
        .and_then(|doc| doc.get_element_by_id("game-canvas"))
        .and_then(|canvas| canvas.parent_element())
        .map(|parent| parent.client_width() as f64)
        .filter(|width| *width > 0.0)
        .unwrap_or(vw);

    let w = available_w.min(vw).min(max_w).max(1.0);
    let h = w * SCREEN_H as f64 / SCREEN_W as f64;
    LogicalSize::new(w, h)
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// The live game instance, registered by `run()`. Lets the host page
    /// join/leave a BroadcastChannel link session at runtime (`linkJoin` /
    /// `linkLeave`). Borrowing is safe because JS is single-threaded and
    /// these calls land between frames, never inside `game.update`/`draw`.
    static GAME: RefCell<Option<Rc<RefCell<PokemonGame>>>> = RefCell::new(None);
}

/// Parse the URL link params: `?link=<channel>[&linkHost=1]`.
///
/// `link` (non-empty) names the BroadcastChannel to join. `linkHost=1` (or
/// `=true`) makes this tab the clock host (Player role: auto-acks the
/// peer's Hello, its random list feeds the shared battle RNG); absent or
/// any other value → guest (Friend role: starts the Hello). Exactly ONE tab
/// must set it: if neither sets it, BOTH start the handshake and the Hello
/// collision surfaces as a driver protocol error; if both set it, neither
/// starts and the session stays pending. Convention: the host tab opens
/// `?link=<channel>&linkHost=1`, the friend's tab just `?link=<channel>`.
#[cfg(target_arch = "wasm32")]
fn link_params_from_url() -> Option<(String, bool)> {
    let search = web_sys::window()?.location().search().ok()?;
    let mut channel: Option<String> = None;
    let mut host = false;
    for pair in search.trim_start_matches('?').split('&') {
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        match key {
            "link" if !value.is_empty() => channel = Some(value.to_string()),
            "linkHost" => host = value == "1" || value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    channel.map(|c| (c, host))
}

/// Join a BroadcastChannel link session at runtime (host-page control; the
/// URL-param entry `?link=<channel>` covers the static case). `host` picks
/// the clock role: `true` = Host (auto-acks the peer's Hello), `false` =
/// Guest (starts the Hello). Errors (invalid channel name, game not
/// started) surface as a thrown `Error`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
#[allow(non_snake_case)] // JS-facing name stays camelCase
pub fn linkJoin(channel: &str, host: bool) -> Result<(), JsValue> {
    let transport =
        BroadcastChannelTransport::new(channel).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let role = if host { LinkRole::Host } else { LinkRole::Guest };
    let game = GAME
        .with(|g| g.borrow().clone())
        .ok_or_else(|| JsValue::from_str("game not started"))?;
    game.borrow_mut().attach_link_transport(Box::new(transport), role);
    log::info!("[link] linkJoin '{}' ({:?})", channel, role);
    Ok(())
}

/// Leave the current link session (host-page control): drops the transport
/// (the peer sees a disconnect) and resets the Cable Club link state.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
#[allow(non_snake_case)] // JS-facing name stays camelCase
pub fn linkLeave() {
    if let Some(game) = GAME.with(|g| g.borrow().clone()) {
        game.borrow_mut().detach_link();
        log::info!("[link] linkLeave");
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    if let Err(e) = console_log::init_with_level(log::Level::Info) {
        web_sys::console::error_1(&format!("Failed to initialize logger: {}", e).into());
    }

    if let Err(e) = run().await {
        web_sys::console::error_1(&format!("Game initialization failed: {}", e).into());

        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("no document");

        let error_div = document.get_element_by_id("error").expect("no error div");
        error_div.set_attribute("class", "").ok();

        let loading = document
            .get_element_by_id("loading")
            .expect("no loading div");
        loading.set_attribute("class", "hidden").ok();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    env_logger::init();
    if let Err(e) = pollster::block_on(run()) {
        eprintln!("Game initialization failed: {}", e);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let version = GameVersion::Red;
    let event_loop = EventLoop::new()?;
    // Drive the loop continuously. On web this routes through
    // `requestAnimationFrame` (~60 Hz); on native it cooperates with the
    // explicit `thread::sleep` pacing below. Without this, winit 0.30's
    // default `ControlFlow::Wait` causes the web backend to only tick when
    // an event arrives, producing ~5 FPS.
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = {
        let size = LogicalSize::new(
            (SCREEN_W * SCALE) as f64,
            (SCREEN_H * SCALE) as f64,
        );
        #[allow(deprecated)]
        Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(&format!(
                        "Pokémon {} - Rust (Web)",
                        match version {
                            GameVersion::Red => "Red",
                            GameVersion::Blue => "Blue",
                        }
                    ))
                    .with_inner_size(size)
                    .with_min_inner_size(LogicalSize::new(
                        SCREEN_W as f64,
                        SCREEN_H as f64,
                    ))
                    .with_resizable(true),
            )?,
        )
    };

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowExtWebSys;

        let game_canvas = window.canvas().unwrap();
        game_canvas.set_id("game-canvas");

        let old_canvas = web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.get_element_by_id("game-canvas"))
            .expect("couldn't find canvas with id 'game-canvas'");

        let parent = old_canvas.parent_node().expect("canvas has no parent");
        parent
            .replace_child(&web_sys::Element::from(game_canvas), &old_canvas)
            .expect("couldn't replace canvas element");

        let resize_window = Arc::clone(&window);
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let _ = resize_window.request_inner_size(get_window_size());
        }) as Box<dyn FnMut(_)>);
        web_sys::window()
            .unwrap()
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();

        let _ = window.request_inner_size(get_window_size());
    }

    let mut pixels = {
        #[cfg(not(target_arch = "wasm32"))]
        let window_size = window.inner_size();

        #[cfg(target_arch = "wasm32")]
        let window_size = get_window_size().to_physical::<u32>(window.scale_factor());

        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, Arc::clone(&window));
        let builder = PixelsBuilder::new(SCREEN_W, SCREEN_H, surface_texture);

        #[cfg(target_arch = "wasm32")]
        let builder = {
            use pixels::wgpu::Backends;

            let texture_format = pixels::wgpu::TextureFormat::Rgba8Unorm;

            builder
                .texture_format(texture_format)
                .surface_texture_format(texture_format)
                // Some browsers expose partial WebGPU limits and return
                // `undefined` for numeric fields, which can panic in wasm-bindgen.
                // Keep wasm on WebGL for broad compatibility.
                .wgpu_backend(Backends::GL)
        };

        builder.build_async().await?
    };

    let game = Rc::new(RefCell::new(PokemonGame::new(version)));
    #[cfg(target_arch = "wasm32")]
    GAME.with(|g| *g.borrow_mut() = Some(Rc::clone(&game)));

    #[cfg(target_arch = "wasm32")]
    {
        // BroadcastChannel link entry: `?link=<channel>[&linkHost=1]` joins
        // a link session with another tab on the same origin — no server.
        // The host tab (`linkHost=1`) takes the Host role (auto-acks), the
        // other tab the Guest role (starts the Hello/HelloAck handshake on
        // its battle driver on the first frame). The game polls the session
        // inside `game.update`, so nothing else is needed here.
        if let Some((channel, host)) = link_params_from_url() {
            match BroadcastChannelTransport::new(&channel) {
                Ok(transport) => {
                    let role = if host { LinkRole::Host } else { LinkRole::Guest };
                    log::info!(
                        "[link] joining BroadcastChannel '{}' ({:?})",
                        channel,
                        role
                    );
                    game.borrow_mut()
                        .attach_link_transport(Box::new(transport), role);
                }
                Err(e) => log::error!("[link] BroadcastChannel '{}' failed: {}", channel, e),
            }
        }
    }

    let mut frame_buffer = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
    let mut input = InputState::new();

    // GB VBlank: 4194304 Hz / 70224 cycles ≈ 59.7275 Hz
    #[cfg(not(target_arch = "wasm32"))]
    const FRAME_DURATION: Duration = Duration::from_nanos(16_742_706);
    #[cfg(not(target_arch = "wasm32"))]
    let mut next_frame_time = Instant::now();

    // Web: milliseconds per GB frame (≈16.7427 ms)
    #[cfg(target_arch = "wasm32")]
    const FRAME_MS: f64 = 1_000.0 * 70_224.0 / 4_194_304.0;
    // Negative sentinel means "not yet initialized"
    #[cfg(target_arch = "wasm32")]
    let mut next_frame_ms: f64 = -1.0;
    #[cfg(target_arch = "wasm32")]
    let mut fps_frame_count: u32 = 0;
    #[cfg(target_arch = "wasm32")]
    let mut fps_last_time: f64 = 0.0;

    let event_handler = move |event, elwt: &winit::event_loop::ActiveEventLoop| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::RedrawRequested => {
                game.borrow_mut().draw(&mut frame_buffer);
                // Indexed 2bpp buffer → RGBA texture via the display palette.
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
                    // Resume AudioContext on first user gesture (browser requirement)
                    if pressed {
                        if let Some(ref audio) = game.borrow().audio {
                            audio.try_resume();
                        }
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
                    window.request_redraw();

                    fps_frame_count += 1;

                    // Refresh the FPS overlay roughly every 30 frames
                    if fps_frame_count >= 30 {
                        let elapsed = now - fps_last_time;
                        if elapsed > 0.0 {
                            let fps = fps_frame_count as f64 * 1_000.0 / elapsed;
                            if let Some(el) = web_sys::window()
                                .and_then(|w| w.document())
                                .and_then(|d| d.get_element_by_id("fps-counter"))
                            {
                                el.set_text_content(Some(&format!("{:.0} FPS", fps)));
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
        let res = event_loop.run(event_handler);
        res?;
        Ok(())
    }
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }
}
