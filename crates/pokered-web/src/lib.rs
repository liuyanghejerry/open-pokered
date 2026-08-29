//! Pokémon Red/Blue - Web/WASM Build
//!
//! Thin game-specific shell over `dotzuki_web::game_shell` (the generic
//! pixels+winit loop: canvas replacement/resize, GB-frame pacing, FPS
//! counter, key mapping, native fallback window). What stays here is the
//! Pokémon wiring: the title, the `?link=<channel>` URL entry, and the
//! host-page `linkJoin`/`linkLeave` runtime controls.
//!
//! The game logic is shared with pokered-app crate via PokemonGame,
//! ensuring identical behavior between web and native builds.

use std::cell::RefCell;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use dotzuki_web::{GameLoop, GameShellConfig, run_game};
use pokered_app::PokemonGame;
use pokered_core::data::wild_data::GameVersion;
use pokered_renderer::FrameBuffer;
use pokered_renderer::input::InputState;

#[cfg(target_arch = "wasm32")]
use pokered_app::link::broadcast_channel::BroadcastChannelTransport;
#[cfg(target_arch = "wasm32")]
use pokered_core::link::LinkRole;

const SCREEN_W: u32 = 160;
const SCREEN_H: u32 = 144;
const SCALE: u32 = 3;

/// Newtype around [`PokemonGame`] so this crate can implement the engine's
/// [`GameLoop`] trait (the orphan rule blocks implementing a dotzuki-web
/// trait for a pokered-app type directly).
struct WebGame(PokemonGame);

impl GameLoop for WebGame {
    type Fb = FrameBuffer;

    fn update(&mut self, input: &InputState) {
        self.0.update(input);
    }

    fn draw(&mut self, fb: &mut FrameBuffer) {
        self.0.draw(fb);
    }

    fn should_exit(&self) -> bool {
        self.0.should_exit()
    }

    fn on_user_gesture(&self) {
        // Resume AudioContext on first user gesture (browser requirement).
        if let Some(ref audio) = self.0.audio {
            audio.try_resume();
        }
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// The live game instance, registered by `run()`. Lets the host page
    /// join/leave a BroadcastChannel link session at runtime (`linkJoin` /
    /// `linkLeave`). Borrowing is safe because JS is single-threaded and
    /// these calls land between frames, never inside `game.update`/`draw`.
    static GAME: RefCell<Option<Rc<RefCell<WebGame>>>> = RefCell::new(None);
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
    game.borrow_mut().0.attach_link_transport(Box::new(transport), role);
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
        game.borrow_mut().0.detach_link();
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
    let config = GameShellConfig::new(
        format!(
            "Pokémon {} - Rust (Web)",
            match version {
                GameVersion::Red => "Red",
                GameVersion::Blue => "Blue",
            }
        ),
        SCREEN_W,
        SCREEN_H,
        SCALE,
    );

    let game = Rc::new(RefCell::new(WebGame(PokemonGame::new(version))));

    #[cfg(target_arch = "wasm32")]
    {
        GAME.with(|g| *g.borrow_mut() = Some(Rc::clone(&game)));

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
                        .0
                        .attach_link_transport(Box::new(transport), role);
                }
                Err(e) => log::error!("[link] BroadcastChannel '{}' failed: {}", channel, e),
            }
        }
    }

    run_game(config, game).await?;
    Ok(())
}
