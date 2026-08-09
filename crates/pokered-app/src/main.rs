mod audio;
mod battle_config;
mod cli;
mod demo;
mod direct_battle;
mod game;
mod link;
mod render;
mod tools;

#[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
mod hot_reload;

use clap::Parser;
use pokered_core::data::wild_data::GameVersion;
use jrpg_app::{run, GameLoop, GameWindowConfig, InputState};
use pokered_renderer::{FrameBuffer, Rgba};
use jrpg_engine::render_config::RenderConfig;

use crate::battle_config::BattleConfig;
use crate::cli::Cli;
use crate::demo::DemoGame;
use crate::direct_battle::DirectBattleGame;
use crate::game::PokemonGame;
use crate::tools::{cmd_dump_state, cmd_screenshot, cmd_screenshot_all};

fn main() {
    let cli = Cli::parse();
    let version = GameVersion::Red;

    if let Some(ref modules) = cli.debug_modules {
        eprintln!("[Debug] CLI debug_modules = '{}'", modules);
        if let Err(e) = pokered_core::debug_log::init("pokered-debug.log") {
            eprintln!("[Debug] Warning: failed to init debug logger: {}", e);
        } else {
            eprintln!("[Debug] Logger initialized OK");
        }
        pokered_core::debug_log::enable_from_str(modules);
        log::info!(target: "pokered::overworld", "[Debug] Logger initialized, modules: {}", modules);
        pokered_core::debug_log::flush();
    } else {
        eprintln!("[Debug] No --debug-modules flag provided");
    }

    // Only bind here for the no-subcommand launch; the `Run` branch binds its
    // own handle from `effective_debug_port`. Without this guard a global
    // `--debug-port` (propagated by clap) would bind twice → "address in use".
    #[cfg(feature = "debug-server")]
    let debug_handle: Option<pokered_debug_server::DebugServerHandle> =
        if let Some(port) = cli.debug_port.filter(|_| cli.command.is_none()) {
            match pokered_debug_server::DebugServer::new(port) {
                Ok((server, handle)) => {
                    eprintln!("Debug server started on port {}", port);
                    std::thread::spawn(move || {
                        server.run();
                    });
                    Some(handle)
                }
                Err(e) => {
                    eprintln!("Error: failed to start debug server on port {}: {}", port, e);
                    std::process::exit(1);
                }
            }
        } else {
            None
        };

    #[cfg(not(feature = "debug-server"))]
    if cli.debug_port.is_some() {
        eprintln!("Warning: --debug-port requires the `debug-server` feature to be enabled.");
    }

    #[cfg(not(debug_assertions))]
    if cli.watch {
        eprintln!("Warning: --watch is only supported in debug builds. Ignoring.");
    }

    if cli.demo {
        let config = GameWindowConfig {
            title: "JRPG Demo — Multi-Layer Map".to_string(),
            scale: 3,
            resizable: true,
            width: 160,
            height: 144,
        };
        let demo = DemoGame::new();
        match run(config, demo) {
            Ok(()) => println!("Demo exited normally"),
            Err(e) => eprintln!("Error: {}", e),
        }
        return;
    }

    match cli.command {
        None => {
            let config = GameWindowConfig {
                title: format!(
                    "Pokémon {} - Rust",
                    match version {
                        GameVersion::Red => "Red",
                        GameVersion::Blue => "Blue",
                    }
                ),
                scale: 3,
                resizable: true,
                width: 160,
                height: 144,
            };
            #[cfg(feature = "debug-server")]
            let mut game = PokemonGame::new_with_options(version, None, None, cli.scripts_dir, false, None, cli.watch, debug_handle);
            #[cfg(not(feature = "debug-server"))]
            let mut game = PokemonGame::new_with_options(version, None, None, cli.scripts_dir, false, None, cli.watch);
            attach_link(&mut game, cli.link_listen, cli.link_connect.clone());
            match run(config, game) {
                Ok(()) => println!("Game exited normally"),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Some(crate::cli::Commands::Run {
            save,
            snapshot,
            skip_intro,
            warp,
            ref debug_port,
            headless,
        }) => {
            // Merge debug_port from the Run subcommand with the global flag.
            let effective_debug_port = debug_port.or(cli.debug_port);

            #[cfg(feature = "debug-server")]
            let debug_handle: Option<pokered_debug_server::DebugServerHandle> =
                if let Some(port) = effective_debug_port {
                    match pokered_debug_server::DebugServer::new(port) {
                        Ok((server, handle)) => {
                            eprintln!("Debug server started on port {}", port);
                            std::thread::spawn(move || {
                                server.run();
                            });
                            Some(handle)
                        }
                        Err(e) => {
                            eprintln!("Error: failed to start debug server on port {}: {}", port, e);
                            std::process::exit(1);
                        }
                    }
                } else {
                    None
                };

            #[cfg(not(feature = "debug-server"))]
            if effective_debug_port.is_some() {
                eprintln!("Warning: --debug-port requires the `debug-server` feature to be enabled.");
            }

            let config = GameWindowConfig {
                title: format!(
                    "Pokémon {} - Rust",
                    match version {
                        GameVersion::Red => "Red",
                        GameVersion::Blue => "Blue",
                    }
                ),
                scale: 3,
                resizable: true,
                width: 160,
                height: 144,
            };
            #[cfg(feature = "debug-server")]
            let mut game = PokemonGame::new_with_options(version, save, snapshot, cli.scripts_dir, skip_intro, warp, cli.watch, debug_handle);
            #[cfg(not(feature = "debug-server"))]
            let mut game = PokemonGame::new_with_options(version, save, snapshot, cli.scripts_dir, skip_intro, warp, cli.watch);
            attach_link(&mut game, cli.link_listen, cli.link_connect.clone());
            if headless {
                // No window: drive the same update loop at the GB frame rate
                // without rendering. The debug server (polled inside update)
                // stays responsive, and step_frames gives drivers exact,
                // synchronous frame control on top.
                eprintln!("Running headless (no window). Press Ctrl-C to exit.");
                const FRAME_DURATION: std::time::Duration =
                    std::time::Duration::from_nanos(16_742_706);
                let mut game = game;
                let input = InputState::new();
                loop {
                    game.update(&input);
                    if game.should_exit() {
                        break;
                    }
                    std::thread::sleep(FRAME_DURATION);
                }
                println!("Game exited normally");
            } else {
                match run(config, game) {
                    Ok(()) => println!("Game exited normally"),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        }
        Some(crate::cli::Commands::ExportSnapshot {
            ref input,
            ref output,
        }) => match PokemonGame::export_snapshot_from_sav(input.as_deref(), output) {
            Ok(()) => println!("Snapshot exported successfully"),
            Err(e) => eprintln!("Error: {}", e),
        },
        Some(crate::cli::Commands::ImportSnapshot {
            ref input,
            ref output,
        }) => match PokemonGame::import_snapshot_from_sav(input, output) {
            Ok(()) => println!("Snapshot imported successfully"),
            Err(e) => eprintln!("Error: {}", e),
        },
        Some(crate::cli::Commands::Screenshot {
            ref screen,
            ref output,
            frames,
        }) => {
            cmd_screenshot(screen, output, frames);
        }
        Some(crate::cli::Commands::ScreenshotAll {
            ref output_dir,
            frames,
        }) => {
            cmd_screenshot_all(output_dir, frames);
        }
        Some(crate::cli::Commands::DumpState { ref screen, frames }) => {
            cmd_dump_state(screen, frames);
        }
        Some(crate::cli::Commands::Battle {
            ref config,
            ref screenshot,
            frames,
        }) => {
            let battle_config = match BattleConfig::load(config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let (player_party, enemy_party) = match battle_config.build_parties() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };
            let mut game = DirectBattleGame::new(
                battle_config.battle_type,
                player_party,
                enemy_party,
                battle_config.trainer_class,
            );

            if let Some(ref output_path) = screenshot {
                let input = InputState::new();
                for _ in 0..frames {
                    game.update(&input);
                }
                let mut fb = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
                game.draw(&mut fb);
                fb.save_png(output_path).expect("Failed to save PNG");
                println!("Battle screenshot saved: {}", output_path.display());
            } else {
                let window_config = GameWindowConfig {
                    title: "Pokémon Battle - Direct Mode".to_string(),
                    scale: 3,
                    resizable: true,
                    width: 160,
                    height: 144,
                };
                match run(window_config, game) {
                    Ok(()) => println!("Battle finished."),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        }
    }
}

/// Wire `--link-listen` / `--link-connect` into the game before it runs.
/// The link flags are global (both the plain launch and the `run`
/// subcommand). Without either flag this is a no-op.
fn attach_link(game: &mut PokemonGame, link_listen: Option<u16>, link_connect: Option<String>) {
    use crate::link::{LinkServer, LinkStatus, TcpTransport, parse_link_addr};
    use std::net::SocketAddr;

    if let Some(port) = link_listen {
        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        match LinkServer::new(addr) {
            Ok(server) => {
                eprintln!("[link] listening on port {} (waiting for peer)", port);
                // The host is the internal clock (the "player" warp side):
                // it wins simultaneous gameboy use and its random list feeds
                // the shared battle RNG (engine/menus/main_menu.asm:216-220).
                game.link_role = pokered_core::link::LinkRole::Host;
                game.link_server = Some(server);
                game.link_status = LinkStatus::WaitingForPeer;
            }
            Err(e) => {
                eprintln!("Error: failed to start link server on port {}: {}", port, e);
                std::process::exit(1);
            }
        }
    } else if let Some(ref addr_str) = link_connect {
        let transport = parse_link_addr(addr_str)
            .and_then(|addr| TcpTransport::connect(addr).map_err(|e| e.to_string()));
        match transport {
            Ok(transport) => {
                eprintln!("[link] connecting to {}", addr_str);
                // The client is the external clock (the "friend" warp side).
                // The game starts the Hello/HelloAck handshake on its battle
                // driver on the first frame (the client side starts it; the
                // server auto-acks). Same public seam the wasm
                // BroadcastChannel entry uses.
                game.attach_link_transport(
                    Box::new(transport),
                    pokered_core::link::LinkRole::Guest,
                );
            }
            Err(e) => {
                eprintln!("Error: failed to connect link peer '{}': {}", addr_str, e);
                std::process::exit(1);
            }
        }
    }
}
