use crate::alloc_prelude::*;
use boa_engine::{Context, JsArgs, JsResult, JsValue};
use dotzuki_engine_script::{BridgeView, ScriptApiRegistrar, ScriptCommand};
use crate::script_command::PokemonScriptCommand;

/// Registers Pokémon-specific JS APIs on the `game` global object.
///
/// These APIs are game-specific and depend on Pokémon game concepts
/// (species, battles, Pokédex, items). Generic APIs like `showText`,
/// `moveNpc`, `getFlag`, `warpTo`, and `playMusic` are registered
/// by the core engine and do not require this registrar.
///
/// Game-specific verbs map to `ScriptCommand::Custom` (name = the JS verb,
/// args passed through as JSON) and are dispatched game-side by
/// `pokered-core`'s overworld script bridge. `playShipDeparture`,
/// `animateHealingMachine`, `openNamingScreen`, `choosePartyPokemon` and
/// `setPartyNickname` were previously registered by the core engine; the
/// engine dropped their dedicated variants in the `Custom` refactor, so they
/// are re-registered here.
///
/// # Usage
///
/// ```ignore
/// use dotzuki_engine_script::ScriptEngine;
/// use pokered_data::script_api::PokemonScriptApi;
///
/// let engine = ScriptEngine::with_api(&PokemonScriptApi);
/// ```
pub struct PokemonScriptApi;

/// Map a badge constant name (case-insensitive) to its bitfield index (0..7),
/// matching the badge bit order in `pokered_data::map_flags`:
/// BOULDERBADGE=0, CASCADEBADGE=1, THUNDERBADGE=2, RAINBOWBADGE=3,
/// SOULBADGE=4, MARSHBADGE=5, VOLCANOBADGE=6, EARTHBADGE=7.
fn badge_index(name: &str) -> Option<u8> {
    match name.to_ascii_uppercase().as_str() {
        "BOULDERBADGE" => Some(0),
        "CASCADEBADGE" => Some(1),
        "THUNDERBADGE" => Some(2),
        "RAINBOWBADGE" => Some(3),
        "SOULBADGE" => Some(4),
        "MARSHBADGE" => Some(5),
        "VOLCANOBADGE" => Some(6),
        "EARTHBADGE" => Some(7),
        _ => None,
    }
}

impl ScriptApiRegistrar for PokemonScriptApi {
    fn register_api(&self, engine: &mut dotzuki_engine_script::ScriptEngine) {
        engine.register_sync_fn("getPokecenterUsed", |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
            Ok(JsValue::from(view.number("pokecenterUsed") != 0.0))
        });
        for name in ["pokecenterNurseOffer", "pokecenterFarewell"] {
            engine.register_async_fn(name, move |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                let values = args.iter().map(|value| {
                    if let Some(text) = value.as_string() { Ok(serde_json::Value::String(text.to_std_string_lossy())) }
                    else if let Some(value) = value.as_boolean() { Ok(serde_json::Value::Bool(value)) }
                    else { Err(boa_engine::JsNativeError::typ().with_message(format!("{name}: invalid argument type"))) }
                }).collect::<Result<Vec<_>, _>>()?;
                PokemonScriptCommand::from_custom(name, &values)
                    .map(PokemonScriptCommand::into_script_command)
                    .map_err(|error| boa_engine::JsNativeError::typ().with_message(error).into())
            });
        }
        engine.register_async_fn("askFieldYesNo", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            if args.len() != 3 {
                return Err(boa_engine::JsNativeError::typ().with_message("askFieldYesNo: expected exactly three strings").into());
            }
            let values = args.iter().map(|arg| arg.as_string()
                .map(|text| serde_json::Value::String(text.to_std_string_lossy()))
                .ok_or_else(|| boa_engine::JsNativeError::typ().with_message("askFieldYesNo: expected exactly three strings")))
                .collect::<Result<Vec<_>, _>>()?;
            PokemonScriptCommand::from_custom("askFieldYesNo", &values)
                .map(PokemonScriptCommand::into_script_command)
                .map_err(|error| boa_engine::JsNativeError::typ().with_message(error).into())
        });

        engine.register_async_fn(
            "printFieldText",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let text = args.get_or_undefined(0).to_string(ctx)?.to_std_string_lossy();
                let retain_window = if args.get_or_undefined(1).is_undefined() {
                    false
                } else {
                    args.get_or_undefined(1).as_boolean().ok_or_else(|| {
                        boa_engine::JsNativeError::typ()
                            .with_message("printFieldText: retainWindow must be a boolean")
                    })?
                };
                let value = args.get_or_undefined(2);
                let prompt = if value.is_undefined() {
                    crate::script_command::FieldTextPrompt::Done
                } else if let Some(enabled) = value.as_boolean() {
                    if enabled { crate::script_command::FieldTextPrompt::Button }
                    else { crate::script_command::FieldTextPrompt::Done }
                } else if value.as_string().is_some_and(|s| s.to_std_string_lossy() == "prompt") {
                    crate::script_command::FieldTextPrompt::Protected
                } else {
                    return Err(boa_engine::JsNativeError::typ()
                        .with_message("printFieldText: prompt must be false, true, or prompt").into());
                };
                let sound = args.get_or_undefined(3);
                let leading_sound = if sound.is_undefined() { None } else {
                    Some(sound.as_string().ok_or_else(|| boa_engine::JsNativeError::typ()
                        .with_message("printFieldText: leadingSound must be a string"))?.to_std_string_lossy())
                };
                Ok(PokemonScriptCommand::PrintFieldText { text, retain_window, prompt, leading_sound }.into_script_command())
            },
        );

        engine.register_async_fn("bikeShopOffer", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            if args.len() != 7 {
                return Err(boa_engine::JsNativeError::typ().with_message("bikeShopOffer: seven text arguments expected").into());
            }
            let texts = args.iter().map(|value| value.as_string()
                .map(|text| text.to_std_string_lossy())
                .ok_or_else(|| boa_engine::JsNativeError::typ().with_message("bikeShopOffer: text arguments must be strings")))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PokemonScriptCommand::BikeShopOffer { texts }.into_script_command())
        });

        engine.register_async_fn("pokedexRating", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            let error = |message: &'static str| boa_engine::JsNativeError::typ().with_message(message);
            if args.len() != 2 { return Err(error("pokedexRating: text and owned count expected").into()); }
            let text = args[0].as_string().ok_or_else(|| error("pokedexRating: argument 0 must be a string"))?.to_std_string_lossy();
            let owned = args[1].as_number().filter(|n| n.is_finite() && n.fract() == 0. && (0. ..=151.).contains(n))
                .ok_or_else(|| error("pokedexRating: owned count must be an integer from 0 to 151"))?;
            Ok(PokemonScriptCommand::PokedexRating { text, owned: owned as u32 }.into_script_command())
        });

        engine.register_async_fn("jigglypuffSong", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            if args.len() != 2 {
                return Err(boa_engine::JsNativeError::typ().with_message("jigglypuffSong: text and npcId expected").into());
            }
            let string = |index: usize| args[index].as_string().map(|value| value.to_std_string_lossy())
                .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("jigglypuffSong: argument {index} must be a string")));
            Ok(PokemonScriptCommand::JigglypuffSong { text: string(0)?, npc_id: string(1)? }.into_script_command())
        });

        engine.register_async_fn("staticEncounter", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            let error=|message: String| boa_engine::JsNativeError::typ().with_message(message);
            if args.len()!=3 { return Err(error("staticEncounter: text, species and level expected".into()).into()); }
            let text=args[0].as_string().ok_or_else(||error("staticEncounter: text must be a string".into()))?.to_std_string_lossy();
            let species=args[1].as_string().ok_or_else(||error("staticEncounter: species must be a string".into()))?.to_std_string_lossy();
            let level=args[2].as_number().filter(|n|n.is_finite() && (1. ..=100.).contains(n) && *n==*n as u8 as f64).ok_or_else(||error("staticEncounter: level must be an integer from 1 to 100".into()))? as u8;
            let typed=PokemonScriptCommand::from_custom("staticEncounter", &[serde_json::json!(text),serde_json::json!(species),serde_json::json!(level)]).map_err(error)?;
            Ok(typed.into_script_command())
        });

        engine.register_async_fn("fieldCry", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            if args.len() != 2 {
                return Err(boa_engine::JsNativeError::typ().with_message("fieldCry: text and species expected").into());
            }
            let string = |index: usize| args[index].as_string().map(|value| value.to_std_string_lossy())
                .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("fieldCry: argument {index} must be a string")));
            Ok(PokemonScriptCommand::FieldCry { text: string(0)?, species: string(1)? }.into_script_command())
        });

        engine.register_async_fn(
            "fieldCryContinuation",
            |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                if args.len() != 4 {
                    return Err(boa_engine::JsNativeError::typ()
                        .with_message("fieldCryContinuation: text, species, paragraph and continuation expected")
                        .into());
                }
                let string = |index: usize| {
                    args[index]
                        .as_string()
                        .map(|value| value.to_std_string_lossy())
                        .ok_or_else(|| {
                            boa_engine::JsNativeError::typ().with_message(format!(
                                "fieldCryContinuation: argument {index} must be a string"
                            ))
                        })
                };
                Ok(PokemonScriptCommand::FieldCryContinuation {
                    texts: vec![string(0)?, string(2)?, string(3)?],
                    species: string(1)?,
                }
                .into_script_command())
            },
        );
        engine.register_async_fn("printTextCry", |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
            if args.len() != 2 {
                return Err(boa_engine::JsNativeError::typ().with_message("printTextCry: text and species expected").into());
            }
            let string = |index: usize| args[index].as_string().map(|value| value.to_std_string_lossy())
                .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("printTextCry: argument {index} must be a string")));
            Ok(PokemonScriptCommand::PrintTextCry { text: string(0)?, species: string(1)? }.into_script_command())
        });

        // FoundItemText prints, plays GET_ITEM_1, then closes without A.
        engine.register_async_fn(
            "showItemDialogue",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let text = args.get_or_undefined(0).to_string(ctx)?.to_std_string_lossy();
                let sound_id = if args.len() > 1 {
                    Some(args.get_or_undefined(1).to_string(ctx)?.to_std_string_lossy())
                } else { None };
                Ok(PokemonScriptCommand::ShowItemDialogue { text, sound_id }.into_script_command())
            },
        );

        // game.giveItem(itemId: string, quantity: number) -> Promise<void>
        engine.register_async_fn(
            "giveItem",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let item_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let quantity = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::GiveItem { item_id, quantity })
            },
        );

        // game.takeItem(itemId: string, quantity: number) -> Promise<void>
        engine.register_async_fn(
            "takeItem",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let item_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let quantity = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::TakeItem { item_id, quantity })
            },
        );

        // game.givePokemon(species: string, level: number) -> Promise<void>
        engine.register_async_fn(
            "givePokemon",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let species = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let level = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::GiveMonster { species, level })
            },
        );

        // game.startBattle(trainerId: string) -> Promise<string>
        engine.register_async_fn(
            "startBattle",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let trainer_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                Ok(ScriptCommand::StartBattle { trainer_id })
            },
        );

        // game.startBattleSet(trainerId: string, rivalTripletBase: number)
        // -> Promise<string>
        // Like startBattle, but for rival battles the 0-based party index of
        // the triplet's Squirtle slot is explicit (scripts/{Map}.asm
        // StarterTable: Route22 1st = 3, CeruleanCity = 6, PokemonTower2F = 3,
        // Route22 2nd = 9).
        engine.register_async_fn(
            "startBattleSet",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let trainer_id = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let base = args.get_or_undefined(1).to_json(ctx)?;
                let rival_triplet_base = base.as_u64().and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| boa_engine::JsNativeError::typ()
                        .with_message("startBattleSet: rivalTripletBase must be a u8"))?;
                Ok(PokemonScriptCommand::StartBattleSet {
                    trainer_id,
                    rival_triplet_base,
                }.into_script_command())
            },
        );

        // Scene-owned ordinary trainer: explicit localized PrintEndBattleText.
        // The generic startBattle and rival startBattleSet keep their defaults.
        engine.register_async_fn(
            "startBattleWithText",
            |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                let string = |index: usize| {
                    args.get_or_undefined(index).as_string()
                        .map(|value| value.to_std_string_lossy())
                        .ok_or_else(|| boa_engine::JsNativeError::typ()
                            .with_message(format!("startBattleWithText: argument {index} must be a string")))
                };
                Ok(PokemonScriptCommand::StartBattleWithText {
                    trainer_id: string(0)?,
                    end_battle_text: string(1)?,
                }.into_script_command())
            },
        );

        // game.startWildBattle(species: string, level: number) -> Promise<string>
        // Starts a catchable wild/static battle (legendary or route blocker) and
        // resolves to the outcome ("win" | "lose" | "caught" | "fled" | "draw").
        engine.register_async_fn(
            "startWildBattle",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let species = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let level = args.get_or_undefined(1).to_u32(ctx)? as u8;
                Ok(ScriptCommand::StartWildBattle { species, level })
            },
        );

        // game.oldManTutorial() -> Promise<string>
        // The Viridian Old-Man catch tutorial (auto-played, guaranteed-catch demo vs a
        // Lv5 WEEDLE). Resolves to the outcome ("caught").
        engine.register_async_fn(
            "oldManTutorial",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::OldManTutorial.into_script_command())
            },
        );

        // game.tradePokemon(offered, received, nickname) -> Promise<bool>
        // Resolves true if the party held `offered` (removed + `received` added).
        engine.register_async_fn(
            "tradePokemon",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::TradePokemon {
                    offered: args.get_or_undefined(0).to_string(ctx)?.to_std_string_lossy(),
                    received: args.get_or_undefined(1).to_string(ctx)?.to_std_string_lossy(),
                    nickname: args.get_or_undefined(2).to_string(ctx)?.to_std_string_lossy(),
                }.into_script_command())
            },
        );

        // game.showPokedexEntry(species: string) -> Promise<void>
        engine.register_async_fn(
            "showPokedexEntry",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::ShowPokedexEntry {
                    species: args.get_or_undefined(0).to_string(ctx)?.to_std_string_lossy(),
                }.into_script_command())
            },
        );

        // game.giveMoney(amount: number) -> Promise<void>
        engine.register_async_fn(
            "giveMoney",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let amount = args.get_or_undefined(0).to_u32(ctx)?;
                Ok(ScriptCommand::GiveMoney { amount })
            },
        );

        // game.takeMoney(amount: number) -> Promise<void>
        engine.register_async_fn(
            "takeMoney",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let amount = args.get_or_undefined(0).to_u32(ctx)?;
                Ok(ScriptCommand::TakeMoney { amount })
            },
        );

        // game.replaceTileBlock(blockX: number, blockY: number, blockId: number)
        //   -> Promise<void>
        // Swap a map block at runtime (BLOCK coordinates). Used for
        // doors/switches/gates. The change is transient (a map reload rebuilds
        // the block grid), so persistent changes must be re-applied on entry.
        engine.register_async_fn(
            "replaceTileBlock",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::ReplaceTileBlock {
                    x: args.get_or_undefined(0).to_u32(ctx)? as u8,
                    y: args.get_or_undefined(1).to_u32(ctx)? as u8,
                    block_id: args.get_or_undefined(2).to_u32(ctx)? as u8,
                }.into_script_command())
            },
        );

        // game.playCry(species: string) -> Promise<void>
        engine.register_async_fn(
            "playCry",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let species = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                Ok(ScriptCommand::PlayCry { species })
            },
        );

        // game.giveBadge(badge: string | number) -> Promise<void>
        // Accepts a badge constant name (BOULDERBADGE..EARTHBADGE,
        // case-insensitive) or a numeric index 0..7.
        engine.register_async_fn(
            "giveBadge",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let arg0 = args.get_or_undefined(0);
                let badge = if arg0.is_number() {
                    arg0.to_number(ctx)? as u8
                } else {
                    let name = arg0.to_string(ctx)?.to_std_string_lossy();
                    badge_index(&name).ok_or_else(|| {
                        boa_engine::JsNativeError::typ()
                            .with_message(format!("giveBadge: unknown badge '{name}'"))
                    })?
                };
                Ok(ScriptCommand::GiveBadge { badge })
            },
        );

        // game.openSlots(lucky?: boolean) -> Promise<void>
        // Opens the Game Corner slot-machine minigame. The app layer credits
        // and debits the player's coin balance.
        engine.register_async_fn(
            "openSlots",
            |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                let lucky = args.get(0).filter(|value| !value.is_undefined())
                    .map(|value| value.to_boolean());
                Ok(PokemonScriptCommand::OpenSlots { lucky }.into_script_command())
            },
        );

        // game.elevatorMenu(floors: string[]) -> Promise<number>
        // Opens the elevator floor-selection menu; resolves to the chosen floor
        // index (0-based) so the scene can warpTo the matching destination, or
        // -1 if the player cancelled.
        engine.register_async_fn(
            "elevatorMenu",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let floors = args.get_or_undefined(0).to_json(ctx)?;
                let command = PokemonScriptCommand::from_custom("elevatorMenu", &[floors])
                    .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
                Ok(command.into_script_command())
            },
        );

        // game.filterBag(itemIds: string[]) -> Promise<string>
        // Opens a filtered-bag menu showing only the candidate items the player
        // actually carries; resolves to the chosen item's const name, or "" if
        // the player cancelled. Used by the drink/fossil/badge-list menus.
        engine.register_async_fn(
            "filterBag",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let item_ids = args.get_or_undefined(0).to_json(ctx)?;
                let command = PokemonScriptCommand::from_custom("filterBag", &[item_ids])
                    .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
                Ok(command.into_script_command())
            },
        );

        // game.showDiploma() -> Promise<void>
        // Shows the full-screen completed-POKeDEX diploma; the app closes it on
        // A/B. The script continues immediately after.
        engine.register_async_fn(
            "showDiploma",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::ShowDiploma.into_script_command())
            },
        );

        // game.openPC() -> Promise<void>
        // Opens the Pokémon Center PC (engine/menus/pc.asm ActivatePC): the
        // full menu with BILL's PC (#MON storage), the player's item PC, and
        // PROF.OAK's PC (#DEX rating). The script continues immediately.
        engine.register_async_fn(
            "openPC",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::OpenPc.into_script_command())
            },
        );

        // game.openItemPC() -> Promise<void>
        // Opens the bedroom PC (engine/menus/players_pc.asm PlayerPC direct):
        // item storage only, no main menu.
        engine.register_async_fn(
            "openItemPC",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::OpenItemPc.into_script_command())
            },
        );

        // game.openBillsPC() -> Promise<void>
        // Opens Bill's house PC directly (home/map_objects.asm TextScript_BillsPC
        // -> engine/pokemon/bills_pc.asm BillsPC_): "Switch on!" then straight
        // into the #MON storage system, no main menu.
        engine.register_async_fn(
            "openBillsPC",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::OpenBillsPc.into_script_command())
            },
        );

        // game.linkStart() -> Promise<void>
        // The Cable Club "gameboy on the table" (Colosseum / Trade Center,
        // engine/pokemon/bills_pc.asm `CableClubLeftGameboy` /
        // `CableClubRightGameboy`): requests the app-level link flow, which
        // starts the LINK BATTLE (Colosseum) or LINK TRADE (Trade Center)
        // against the connected peer. The script continues immediately; the
        // app shows "Just a moment." (JustAMomentText) and drives the
        // request/accept/decline state machines. Offline (no link session)
        // the request is dropped and only "Just a moment." is shown.
        engine.register_async_fn(
            "linkStart",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::LinkStart.into_script_command())
            },
        );

        // game.enterHallOfFame() -> Promise<void>
        // Records the party in the Hall of Fame and plays the endgame
        // ceremony (engine/movie/hall_of_fame.asm roll call + credits.asm):
        // the app pushes the team onto the SRAM HoF list, runs the movie
        // takeover, saves, and resets to the title screen. The script
        // continues immediately (and simply ends).
        engine.register_async_fn(
            "enterHallOfFame",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::EnterHallOfFame.into_script_command())
            },
        );

        // game.giveCoins(amount: number) -> Promise<void>
        // Credit the player's Game Corner coin balance (capped at 9999).
        engine.register_async_fn(
            "giveCoins",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let amount = args.get_or_undefined(0).to_u32(ctx)?.min(u16::MAX as u32) as u16;
                Ok(PokemonScriptCommand::GiveCoins { amount }.into_script_command())
            },
        );

        // game.takeCoins(amount: number) -> Promise<void>
        engine.register_async_fn(
            "takeCoins",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let amount = args.get_or_undefined(0).to_u32(ctx)?.min(u16::MAX as u32) as u16;
                Ok(PokemonScriptCommand::TakeCoins { amount }.into_script_command())
            },
        );

        // game.depositDaycare(index: number) -> Promise<void>
        // Deposit the party member at the 0-based `index` into the Day Care.
        engine.register_async_fn(
            "depositDaycare",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::DepositDaycare {
                    index: args.get_or_undefined(0).to_u32(ctx)? as u8,
                }.into_script_command())
            },
        );

        // game.withdrawDaycare() -> Promise<void>
        // Withdraw the Day Care Pokémon back into the party at its grown level.
        engine.register_async_fn(
            "withdrawDaycare",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::WithdrawDaycare.into_script_command())
            },
        );

        // ── Game-side re-registrations: these verbs were registered by the
        // core engine before the `Custom` refactor dropped their dedicated
        // variants; games must now register them. JS names unchanged. ────────

        // Wait for the sequencer, rather than guessing a jingle duration.
        engine.register_async_fn(
            "waitMusic",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::WaitMusic.into_script_command())
            },
        );

        // WaitForSoundToFinish uses logical SFX completion even without a device.
        engine.register_async_fn(
            "waitSound",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::WaitSound.into_script_command())
            },
        );

        engine.register_async_fn(
            "finishFieldText",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::FinishFieldText.into_script_command())
            },
        );

        // game.playShipDeparture() -> Promise<void>
        // The S.S. Anne departure cutscene (VermilionDock): the blocking
        // ship-sail animation (smoke puffs + view scroll + erase).
        engine.register_async_fn(
            "playShipDeparture",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::PlayShipDeparture.into_script_command())
            },
        );

        // game.animateHealingMachine() -> Promise<void>
        // Pokémon Center healing-machine animation.
        engine.register_async_fn(
            "animateHealingMachine",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::AnimateHealingMachine.into_script_command())
            },
        );

        // game.openNamingScreen(species: string) -> Promise<string>
        // Opens the naming screen for a newly caught/hatched `species`;
        // resolves to the chosen name.
        engine.register_async_fn(
            "openNamingScreen",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::OpenNamingScreen {
                    species: args.get_or_undefined(0).to_string(ctx)?.to_std_string_lossy(),
                }.into_script_command())
            },
        );

        engine.register_async_fn(
            "silphRocketDeparture",
            |args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                if !args.is_empty() {
                    return Err(boa_engine::JsNativeError::typ()
                        .with_message("silphRocketDeparture: no arguments expected").into());
                }
                Ok(PokemonScriptCommand::SilphRocketDeparture.into_script_command())
            },
        );

        // game.choosePartyPokemon() -> Promise<number>
        // Opens the party menu as a selector (Name Rater); resolves to the
        // chosen 0-based index, or -1 on cancel.
        engine.register_async_fn(
            "choosePartyPokemon",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::ChoosePartyPokemon.into_script_command())
            },
        );

        // game.setPartyNickname(index: number, nickname: string) -> Promise<void>
        // Writes a new nickname onto the party member at `index`.
        engine.register_async_fn(
            "setPartyNickname",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(PokemonScriptCommand::SetPartyNickname {
                    index: args.get_or_undefined(0).to_u32(ctx)? as u8,
                    nickname: args.get_or_undefined(1).to_string(ctx)?.to_std_string_lossy(),
                }.into_script_command())
            },
        );

        // ── Synchronous query APIs (read seeded state; no await needed) ──────

        // game.hasItem(itemConstName: string) -> boolean
        engine.register_sync_fn(
            "hasItem",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let name = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                Ok(JsValue::from(view.set_contains("bag", &name)))
            },
        );

        engine.register_async_fn("showMoneyBox", |args: &[JsValue], ctx: &mut Context| {
            Ok(PokemonScriptCommand::ShowMoneyBox { amount: args.get_or_undefined(0).to_number(ctx)? as i64 }.into_script_command())
        });
        engine.register_async_fn("showCoinBox", |args: &[JsValue], ctx: &mut Context| {
            Ok(PokemonScriptCommand::ShowCoinBox { amount: args.get_or_undefined(0).to_number(ctx)? as i64 }.into_script_command())
        });
        engine.register_async_fn("vendingDelivery", |_: &[JsValue], _: &mut Context| {
            Ok(PokemonScriptCommand::VendingDelivery.into_script_command())
        });
        engine.register_async_fn("walkTogether", |args: &[JsValue], ctx: &mut Context| {
            let values = vec![args.get_or_undefined(0).to_json(ctx)?, args.get_or_undefined(1).to_json(ctx)?, args.get_or_undefined(2).to_json(ctx)?];
            let command = PokemonScriptCommand::from_custom("walkTogether", &values)
                .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
            Ok(command.into_script_command())
        });
        engine.register_async_fn("followNpcPath", |args: &[JsValue], ctx: &mut Context| {
            let values = vec![args.get_or_undefined(0).to_json(ctx)?, args.get_or_undefined(1).to_json(ctx)?];
            let command = PokemonScriptCommand::from_custom("followNpcPath", &values)
                .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
            Ok(command.into_script_command())
        });
        engine.register_async_fn("billPokemonList", |args: &[JsValue], ctx: &mut Context| {
            let texts = args.get_or_undefined(0).to_json(ctx)?;
            let command = PokemonScriptCommand::from_custom("billPokemonList", &[texts])
                .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
            Ok(command.into_script_command())
        });
        for name in ["badgeMenu", "pokemonMenu"] {
            engine.register_async_fn(name, move |args: &[JsValue], ctx: &mut Context| {
                let mut values = vec![args.get_or_undefined(0).to_json(ctx)?, args.get_or_undefined(1).to_json(ctx)?];
                if name == "badgeMenu" { values.push(args.get_or_undefined(2).to_json(ctx)?); }
                let command = PokemonScriptCommand::from_custom(name, &values)
                    .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
                Ok(command.into_script_command())
            });
        }
        engine.register_async_fn("readingMenu", |args: &[JsValue], ctx: &mut Context| {
            let options = args.get_or_undefined(0).to_json(ctx)?;
            let texts = args.get_or_undefined(1).to_json(ctx)?;
            let command = PokemonScriptCommand::from_custom("readingMenu", &[options, texts])
                .map_err(|error| boa_engine::JsNativeError::typ().with_message(error))?;
            Ok(command.into_script_command())
        });

        // game.getMoney() -> number
        engine.register_sync_fn(
            "getMoney",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("money")))
            },
        );

        // game.hasMoney(amount: number) -> boolean
        engine.register_sync_fn(
            "hasMoney",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let needed = args.get_or_undefined(0).to_number(ctx)?;
                Ok(JsValue::from(view.number("money") >= needed))
            },
        );

        // game.getPokedexOwnedCount() -> number
        engine.register_sync_fn(
            "getPokedexOwnedCount",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("pokedexOwned")))
            },
        );

        // game.getPokedexSeenCount() -> number
        engine.register_sync_fn(
            "getPokedexSeenCount",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("pokedexSeen")))
            },
        );

        // game.getPlayerFacing() -> string ("up" | "down" | "left" | "right")
        engine.register_sync_fn(
            "getPlayerFacing",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(boa_engine::js_string!(
                    view.text("playerFacing")
                )))
            },
        );

        // game.getRivalStarter() -> number (0 = Squirtle, 1 = Bulbasaur, 2 = Charmander)
        engine.register_sync_fn(
            "getRivalStarter",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("rivalStarter")))
            },
        );

        // game.getGameVersion() -> number  (0 = Red, 1 = Blue)
        engine.register_sync_fn(
            "getGameVersion",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("gameVersion")))
            },
        );

        // game.getBadgeCount() -> number  (0..8 badges owned)
        engine.register_sync_fn(
            "getBadgeCount",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from((view.number("obtainedBadges") as u8).count_ones()))
            },
        );

        // game.hasBadge(badge: string) -> boolean
        // Accepts a badge constant name (BOULDERBADGE..EARTHBADGE,
        // case-insensitive). Mirrors the bit order used by giveBadge.
        engine.register_sync_fn(
            "hasBadge",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let name = args
                    .get_or_undefined(0)
                    .to_string(ctx)?
                    .to_std_string_lossy();
                let idx = badge_index(&name).ok_or_else(|| {
                    boa_engine::JsNativeError::typ()
                        .with_message(format!("hasBadge: unknown badge '{name}'"))
                })?;
                Ok(JsValue::from(
                    (view.number("obtainedBadges") as u8) & (1 << idx) != 0,
                ))
            },
        );

        // game.getCoins() -> number  (Game Corner coin balance)
        engine.register_sync_fn(
            "getCoins",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("coins")))
            },
        );

        // game.hasCoins(amount: number) -> boolean
        engine.register_sync_fn(
            "hasCoins",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let needed = args.get_or_undefined(0).to_number(ctx)?;
                Ok(JsValue::from(view.number("coins") >= needed))
            },
        );

        // game.isDaycareInUse() -> boolean  (is a Pokémon currently deposited?)
        engine.register_sync_fn(
            "isDaycareInUse",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("daycareInUse") != 0.0))
            },
        );

        engine.register_sync_fn("getDaycareMonSpecies", |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
            Ok(JsValue::from(boa_engine::js_string!(view.text("daycareMonSpecies"))))
        });
        engine.register_sync_fn("getPartyMonSpecies", |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
            let idx = args.get_or_undefined(0).to_u32(ctx)?;
            Ok(JsValue::from(boa_engine::js_string!(view.text(&format!("partySpecies{idx}")))))
        });

        // game.getDaycareMonName() -> string  (nickname/species of the deposited mon)
        engine.register_sync_fn(
            "getDaycareMonName",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(boa_engine::js_string!(
                    view.text("daycareMonName")
                )))
            },
        );

        // game.getDaycareLevelsGrown() -> number  (levels gained since deposit)
        engine.register_sync_fn(
            "getDaycareLevelsGrown",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("daycareLevelsGrown")))
            },
        );

        // game.getDaycareCost() -> number  (yen owed to withdraw: 100*(grown+1))
        engine.register_sync_fn(
            "getDaycareCost",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("daycareCost")))
            },
        );

        // game.getPartyCount() -> number
        engine.register_sync_fn(
            "getPartyCount",
            |_args: &[JsValue], _ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                Ok(JsValue::from(view.number("partyCount")))
            },
        );

        // game.getPartyMonName(index: number) -> string  (nickname/species)
        engine.register_sync_fn(
            "getPartyMonName",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let idx = args.get_or_undefined(0).to_u32(ctx)?;
                Ok(JsValue::from(boa_engine::js_string!(
                    view.text(&format!("partyName{idx}"))
                )))
            },
        );

        engine.register_sync_fn(
            "partyMonCanRename",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let idx = args.get_or_undefined(0).to_u32(ctx)?;
                Ok(JsValue::from(view.number(&format!("partyCanRename{idx}")) != 0.0))
            },
        );

        // game.partyMonKnowsHm(index: number) -> boolean
        engine.register_sync_fn(
            "partyMonKnowsHm",
            |args: &[JsValue], ctx: &mut Context, view: &BridgeView| -> JsResult<JsValue> {
                let idx = args.get_or_undefined(0).to_u32(ctx)?;
                Ok(JsValue::from(view.number(&format!("partyKnowsHm{idx}")) != 0.0))
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::alloc_prelude::*;
    use crate::script_function_catalog::POKERED_SCRIPT_FUNCTIONS;
    use super::PokemonScriptApi;
    use dotzuki_engine_script::{CommandResult, ScriptCommand, ScriptEngine};

    #[test]
    fn boa_silph_departure_await_blocks_and_extra_arguments_reject() {
        use crate::script_command::PokemonScriptCommand;
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine
            .load_script(
                r#"
            export async function departure() {
                await game.silphRocketDeparture();
                await game.showText("after fade");
            }
            export async function invalid() {
                try {
                    await game.silphRocketDeparture(false);
                    await game.showText("INVALID_ARGUMENT_ACCEPTED");
                } catch (error) {
                    await game.showText(error.name + ":" + error.message);
                }
            }
        "#,
            )
            .unwrap();
        let command = PokemonScriptCommand::SilphRocketDeparture.into_script_command();
        assert_eq!(
            engine.call_function("departure", &[]).unwrap(),
            Some(command.clone())
        );
        for _ in 0..3 {
            assert_eq!(engine.tick(), Some(command.clone()));
        }
        assert_eq!(
            engine.signal_done(CommandResult::Void).unwrap(),
            Some(ScriptCommand::ShowText {
                text: "after fade".into()
            })
        );
        assert!(engine.signal_done(CommandResult::Void).unwrap().is_none());
        assert_eq!(
            engine.call_function("invalid", &[]).unwrap(),
            Some(ScriptCommand::ShowText {
                text: "TypeError:silphRocketDeparture: no arguments expected".into()
            })
        );
        assert!(engine.signal_done(CommandResult::Void).unwrap().is_none());
    }

    #[test]
    fn fidelity_menus_and_reward_sound_suspend_with_typed_commands_in_boa() {
        use crate::script_command::PokemonScriptCommand;
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"
            export async function verifyFidelity() {
                await game.badgeMenu(["B", "C", "T", "R", "S", "M", "V", "E", "CANCEL"],
                    ["b", "c", "t", "r", "s", "m", "v", "e"],
                    [true, false, true, false, false, false, false, false]);
                await game.pokemonMenu(["EEVEE", "CANCEL"], ["Eevee"]);
                await game.showItemDialogue("received", "SFX_GET_KEY_ITEM");
            }
        "#).unwrap();
        let expected = [
            PokemonScriptCommand::ReadingMenu { options: vec!["B".into(), "T".into(), "CANCEL".into()], texts: vec!["b".into(), "t".into()] },
            PokemonScriptCommand::PokemonMenu { options: vec!["EEVEE".into(), "CANCEL".into()], species: vec!["Eevee".into()] },
            PokemonScriptCommand::ShowItemDialogue { text: "received".into(), sound_id: Some("SFX_GET_KEY_ITEM".into()) },
        ];
        let mut next = engine.call_function("verifyFidelity", &[]).unwrap();
        for expected in expected {
            let Some(ScriptCommand::Custom { name, args }) = &next else { panic!("missing command: {next:?}"); };
            assert_eq!(PokemonScriptCommand::from_custom(name, args).unwrap(), expected);
            assert_eq!(engine.tick(), next);
            next = engine.signal_done(CommandResult::Void).unwrap();
        }
        assert!(next.is_none());
    }

    #[test]
    fn boa_registrar_implements_every_cataloged_capability() {
        let names = serde_json::to_string(POKERED_SCRIPT_FUNCTIONS).unwrap();
        let source = format!(
            r#"
                export async function verifyCatalog() {{
                    for (const name of {names}) {{
                        if (typeof game[name] !== "function") {{
                            throw new Error(`missing game capability: ${{name}}`);
                        }}
                    }}
                }}
            "#
        );
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(&source).unwrap();
        assert_eq!(engine.call_function("verifyCatalog", &[]).unwrap(), None);
    }

    /// Proves the synchronous query APIs (`hasItem`, `getMoney`) read seeded
    /// state: seeding money=5000 + bag={SILPH_SCOPE} makes the gate take the
    /// "ok" branch.
    #[test]
    fn sync_queries_read_seeded_state() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.seed_number("money", 5000.0);
        engine.seed_set("bag", &["SILPH_SCOPE".to_string()]);

        engine
            .load_script(
                r#"
                export async function f() {
                    if (game.hasItem("SILPH_SCOPE") && game.getMoney() >= 3000) {
                        await game.showText("ok");
                    } else {
                        await game.showText("no");
                    }
                }
            "#,
            )
            .unwrap();

        let cmd = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            cmd,
            Some(ScriptCommand::ShowText {
                text: "ok".to_string()
            })
        );
    }

    /// Inverse: when state does not satisfy the gate, the "no" branch runs.
    #[test]
    fn sync_queries_gate_negative() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.seed_number("money", 100.0);
        // bag deliberately not seeded -> hasItem("SILPH_SCOPE") is false.

        engine
            .load_script(
                r#"
                export async function f() {
                    if (game.hasItem("SILPH_SCOPE") && game.getMoney() >= 3000) {
                        await game.showText("ok");
                    } else {
                        await game.showText("no");
                    }
                }
            "#,
            )
            .unwrap();

        let cmd = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            cmd,
            Some(ScriptCommand::ShowText {
                text: "no".to_string()
            })
        );
    }

    /// Badge queries read the seeded obtained-badges bitmask: with all badges
    /// except EARTH (bit 7) set, getBadgeCount() == 7, hasBadge("BOULDERBADGE")
    /// is true and hasBadge("EARTHBADGE") is false.
    #[test]
    fn badge_queries_read_seeded_bitmask() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        // 0b0111_1111 = BOULDERBADGE..VOLCANOBADGE set, EARTHBADGE clear.
        engine.seed_number("obtainedBadges", 127.0);

        engine
            .load_script(
                r#"
                export async function f() {
                    if (game.getBadgeCount() == 7 && game.hasBadge("BOULDERBADGE") && !game.hasBadge("EARTHBADGE")) {
                        await game.showText("all-but-earth");
                    } else {
                        await game.showText("other");
                    }
                }
            "#,
            )
            .unwrap();

        let cmd = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            cmd,
            Some(ScriptCommand::ShowText {
                text: "all-but-earth".to_string()
            })
        );
    }

    /// Unknown badge names are rejected by hasBadge at runtime (JS TypeError).
    #[test]
    fn has_badge_rejects_unknown_name() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.seed_number("obtainedBadges", 0.0);

        engine
            .load_script(
                r#"
                export async function f() {
                    try {
                        game.hasBadge("NO_SUCH_BADGE");
                        await game.showText("no-error");
                    } catch (e) {
                        await game.showText("error");
                    }
                }
            "#,
            )
            .unwrap();

        let cmd = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            cmd,
            Some(ScriptCommand::ShowText {
                text: "error".to_string()
            }),
            "hasBadge with an unknown name must raise a JS error"
        );
    }

    /// Core guarantee of FIX 2 (battle-result resolution) at the engine level:
    /// `await game.startBattle(...)` yields a `StartBattle` command and then the
    /// script stays SUSPENDED — it does NOT immediately produce the post-battle
    /// `showText`. Only after `signal_done(Text("win"))` does the script resume,
    /// take the win-branch, and yield `ShowText("won")`.
    #[test]
    fn start_battle_suspends_until_outcome_then_resumes() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);

        engine
            .load_script(
                r#"
                export async function f() {
                    let r = await game.startBattle("OPP_BROCK");
                    if (r == "win") {
                        await game.showText("won");
                    } else {
                        await game.showText("lost");
                    }
                }
            "#,
            )
            .unwrap();

        // First yielded command is the battle request.
        let first = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            first,
            Some(ScriptCommand::StartBattle {
                trainer_id: "OPP_BROCK".to_string()
            }),
            "startBattle should be the first yielded command",
        );

        // The script is suspended on the await: ticking must NOT advance it to
        // the post-battle showText. It re-reports the same pending command.
        let still_pending = engine.tick();
        assert_eq!(
            still_pending,
            Some(ScriptCommand::StartBattle {
                trainer_id: "OPP_BROCK".to_string()
            }),
            "script must stay suspended on startBattle until the outcome is delivered",
        );

        // Deliver the battle outcome -> script resumes and runs the win branch.
        let resumed = engine
            .signal_done(CommandResult::Text("win".to_string()))
            .unwrap();
        assert_eq!(
            resumed,
            Some(ScriptCommand::ShowText {
                text: "won".to_string()
            }),
            "resuming with \"win\" must take the win-branch -> showText(\"won\")",
        );
    }

    /// Core guarantee for elevatorMenu: it yields an `elevatorMenu` custom
    /// command and then stays SUSPENDED (like startBattle) until the app
    /// delivers the chosen floor via `signal_done(Number(...))`, after which
    /// the matching branch runs.
    #[test]
    fn elevator_menu_suspends_until_floor_then_resumes() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);

        engine
            .load_script(
                r#"
                export async function f() {
                    let floor = await game.elevatorMenu(["1F", "2F", "3F"]);
                    if (floor == 2) {
                        await game.showText("3F");
                    } else {
                        await game.showText("other");
                    }
                }
            "#,
            )
            .unwrap();

        // First yielded command is the floor-menu request.
        let first = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            first,
            Some(ScriptCommand::Custom {
                name: "elevatorMenu".to_string(),
                args: vec![serde_json::json!(["1F", "2F", "3F"])]
            }),
            "elevatorMenu should be the first yielded command",
        );

        // The script is suspended on the await: ticking must NOT advance it.
        let still_pending = engine.tick();
        assert_eq!(
            still_pending,
            Some(ScriptCommand::Custom {
                name: "elevatorMenu".to_string(),
                args: vec![serde_json::json!(["1F", "2F", "3F"])]
            }),
            "script must stay suspended on elevatorMenu until the floor is delivered",
        );

        // Deliver floor index 2 -> script resumes and runs the matching branch.
        let resumed = engine
            .signal_done(CommandResult::Number(2.0))
            .unwrap();
        assert_eq!(
            resumed,
            Some(ScriptCommand::ShowText {
                text: "3F".to_string()
            }),
            "resuming with floor 2 must take the floor==2 branch -> showText(\"3F\")",
        );
    }

    #[test]
    fn explicit_escort_suspends_boa_until_movement_finishes() {
        use crate::script_command::PokemonScriptCommand;
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"
            export async function f() {
                await game.followNpcPath("OAK", [[12,12], [12,11]]);
                await game.showText("arrived");
            }
        "#).unwrap();
        let command = engine.call_function("f", &[]).unwrap();
        let Some(ScriptCommand::Custom { name, args }) = &command else { panic!("missing escort command"); };
        assert_eq!(PokemonScriptCommand::from_custom(name, args).unwrap(),
            PokemonScriptCommand::FollowNpcPath { npc_id: "OAK".into(), path: vec![(12,12), (12,11)] });
        assert_eq!(engine.tick(), command, "movement must stay awaited");
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(),
            Some(ScriptCommand::ShowText { text: "arrived".into() }));
    }

    #[test]
    fn reading_and_vending_apis_suspend_the_boa_fallback() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"
            export async function f() {
                await game.readingMenu(["SLP", "QUIT"], ["Sleep information"]);
                await game.showMoneyBox(3000);
                await game.vendingDelivery();
                await game.showMoneyBox(-1);
                await game.showCoinBox(50);
                await game.showCoinBox(-1);
            }
        "#).unwrap();
        let mut next = engine.call_function("f", &[]).unwrap();
        for expected in ["readingMenu", "showMoneyBox", "vendingDelivery", "showMoneyBox", "showCoinBox", "showCoinBox"] {
            let Some(ScriptCommand::Custom { name, .. }) = &next else { panic!("missing {expected}: {next:?}"); };
            assert_eq!(name, expected);
            assert_eq!(engine.tick(), next, "await must stay parked until completion");
            next = engine.signal_done(CommandResult::Void).unwrap();
        }
        assert!(next.is_none());
    }

    /// filterBag yields a `filterBag` custom command, suspends until the app
    /// delivers the chosen item name, then resumes on the matching branch.
    #[test]
    fn filter_bag_suspends_until_item_then_resumes() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);

        engine
            .load_script(
                r#"
                export async function f() {
                    let drink = await game.filterBag(["FRESH_WATER", "SODA_POP", "LEMONADE"]);
                    if (drink == "FRESH_WATER") {
                        await game.showText("fresh");
                    } else {
                        await game.showText("other");
                    }
                }
            "#,
            )
            .unwrap();

        let first = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            first,
            Some(ScriptCommand::Custom {
                name: "filterBag".to_string(),
                args: vec![serde_json::json!([
                    "FRESH_WATER",
                    "SODA_POP",
                    "LEMONADE"
                ])]
            }),
            "filterBag should be the first yielded command",
        );

        // Suspended: ticking must not advance the script.
        let still = engine.tick();
        assert_eq!(
            still,
            Some(ScriptCommand::Custom {
                name: "filterBag".to_string(),
                args: vec![serde_json::json!([
                    "FRESH_WATER",
                    "SODA_POP",
                    "LEMONADE"
                ])]
            }),
            "script must stay suspended on filterBag until the item is delivered",
        );

        // Deliver the chosen item -> the matching branch runs.
        let resumed = engine
            .signal_done(CommandResult::Text("FRESH_WATER".to_string()))
            .unwrap();
        assert_eq!(
            resumed,
            Some(ScriptCommand::ShowText {
                text: "fresh".to_string()
            }),
            "resuming with FRESH_WATER must take the matching branch",
        );
    }

    /// showDiploma yields a `showDiploma` custom command (immediate-void effect).
    #[test]
    fn show_diploma_yields_command() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);

        engine
            .load_script(
                r#"
                export async function f() {
                    await game.showDiploma();
                    await game.showText("done");
                }
            "#,
            )
            .unwrap();

        let first = engine.call_function("f", &[]).unwrap();
        assert_eq!(
            first,
            Some(ScriptCommand::Custom {
                name: "showDiploma".to_string(),
                args: vec![]
            })
        );

        // The effect completes immediately (Void); the script continues.
        let resumed = engine.signal_done(CommandResult::Void).unwrap();
        assert_eq!(
            resumed,
            Some(ScriptCommand::ShowText {
                text: "done".to_string()
            })
        );
    }

    #[test]
    fn scoped_field_text_and_wait_sound_suspend_with_typed_commands_in_boa() {
        use crate::script_command::PokemonScriptCommand;
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"
            export async function verifyCryWait() {
                await game.printFieldText("mother", true);
                await game.waitSound();
                await game.printFieldText("ordinary");
            }
        "#).unwrap();
        let expected = [
            PokemonScriptCommand::PrintFieldText { text: "mother".into(), retain_window: true, prompt: crate::script_command::FieldTextPrompt::Done, leading_sound: None },
            PokemonScriptCommand::WaitSound,
            PokemonScriptCommand::PrintFieldText { text: "ordinary".into(), retain_window: false, prompt: crate::script_command::FieldTextPrompt::Done, leading_sound: None },
        ];
        let mut next = engine.call_function("verifyCryWait", &[]).unwrap();
        for expected in expected {
            let Some(ScriptCommand::Custom { name, args }) = &next else { panic!("missing command: {next:?}"); };
            assert_eq!(PokemonScriptCommand::from_custom(name, args).unwrap(), expected);
            assert_eq!(engine.tick(), next);
            next = engine.signal_done(CommandResult::Void).unwrap();
        }
        assert!(next.is_none());
    }

    #[test]
    fn boa_retained_prompt_modes_suspend_and_resume_without_coercing_invalid_modes() {
        use crate::script_command::{FieldTextPrompt, PokemonScriptCommand};
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"
            export async function verifyQuizText() {
                await game.printFieldText("intro", true, "prompt");
                await game.printFieldText("question", true, false);
                await game.printFieldText("correct", true, true, "SFX_GET_ITEM_1");
                await game.finishFieldText();
            }
        "#).unwrap();
        let expected = [
            PokemonScriptCommand::PrintFieldText { text: "intro".into(), retain_window: true, prompt: FieldTextPrompt::Protected, leading_sound: None },
            PokemonScriptCommand::PrintFieldText { text: "question".into(), retain_window: true, prompt: FieldTextPrompt::Done, leading_sound: None },
            PokemonScriptCommand::PrintFieldText { text: "correct".into(), retain_window: true, prompt: FieldTextPrompt::Button, leading_sound: Some("SFX_GET_ITEM_1".into()) },
            PokemonScriptCommand::FinishFieldText,
        ];
        let mut next = engine.call_function("verifyQuizText", &[]).unwrap();
        for expected in expected {
            let Some(ScriptCommand::Custom { name, args }) = &next else { panic!("missing command: {next:?}"); };
            assert_eq!(PokemonScriptCommand::from_custom(name, args).unwrap(), expected);
            assert_eq!(engine.tick(), next, "each async call must suspend until the caller resumes it");
            next = engine.signal_done(CommandResult::Void).unwrap();
        }
        assert!(next.is_none());
        engine.load_script(r#"
            export async function invalidQuizText() {
                try {
                    await game.printFieldText("invalid", true, "true");
                    await game.showText("INVALID_MODE_ACCEPTED");
                } catch (error) {
                    await game.showText(error.name + ":" + error.message);
                }
            }
        "#).unwrap();
        assert_eq!(engine.call_function("invalidQuizText", &[]).unwrap(),
            Some(ScriptCommand::ShowText {
                text: "TypeError:printFieldText: prompt must be false, true, or prompt".into(),
            }));
        assert!(engine.signal_done(CommandResult::Void).unwrap().is_none());
    }
    #[test]
    fn quoted_battle_awaits_the_real_result_and_invalid_payload_rejects_the_promise() {
        use crate::script_command::PokemonScriptCommand;
        for outcome in ["win", "lose", "draw"] {
            let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
            engine.load_script(r#"export async function f() {
                let r = await game.startBattleWithText("OPP_BURGLAR4", "I\nsurrender!");
                await game.showText(r);
            }"#).unwrap();
            let expected = PokemonScriptCommand::StartBattleWithText {
                trainer_id: "OPP_BURGLAR4".into(), end_battle_text: "I\nsurrender!".into(),
            }.into_script_command();
            assert_eq!(engine.call_function("f", &[]).unwrap(), Some(expected.clone()));
            assert_eq!(engine.tick(), Some(expected));
            assert_eq!(engine.signal_done(CommandResult::Text(outcome.into())).unwrap(),
                Some(ScriptCommand::ShowText { text: outcome.into() }));
            assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), None);
        }
        for (arguments, index) in [("7, \"text\"", 0), ("\"ID\"", 1), ("\"ID\", null", 1)] {
            let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
            engine.load_script(&format!(r#"export async function f() {{
                try {{ await game.startBattleWithText({arguments}); await game.showText("INVALID_PAYLOAD_ACCEPTED"); }}
                catch (e) {{ await game.showText(String(e)); }}
            }}"#)).unwrap();
            assert_eq!(engine.call_function("f", &[]).unwrap(), Some(ScriptCommand::ShowText {
                text: format!("TypeError: startBattleWithText: argument {index} must be a string"),
            }));
            assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), None);
        }
    }

}

#[cfg(test)]
mod bike_offer_boa_tests {
    use super::*;
    use dotzuki_engine_script::{ScriptEngine, CommandResult};
    #[test]
    fn bike_offer_boa_await_keeps_the_real_caller_suspended_until_outer_close() {
        let mut engine=ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"export async function offer() {
          await game.bikeShopOffer("first", "last", "question", "refusal", "goodbye", "bicycle", "cancel");
          await game.showText("returned");
        }"#).unwrap();
        let expected=Some(PokemonScriptCommand::BikeShopOffer {texts:["first","last","question","refusal","goodbye","bicycle","cancel"].into_iter().map(str::to_string).collect()}.into_script_command());
        assert_eq!(engine.call_function("offer",&[]).unwrap(),expected);
        assert_eq!(engine.tick(),expected,"idle ticking is not an outer textbox close");
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(),Some(ScriptCommand::ShowText{text:"returned".into()}));
    }
}

#[cfg(test)]
mod field_cry_boa_tests {
    use super::*;
    use dotzuki_engine_script::{ScriptEngine, CommandResult};
    #[test]
    fn field_cry_boa_await_does_not_return_before_outer_close() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"export async function bird() {
          await game.fieldCry("PIDGEY: Kurukkoo!", "PIDGEY");
          await game.showText("returned");
        }"#).unwrap();
        let expected = Some(PokemonScriptCommand::FieldCry {
            text: "PIDGEY: Kurukkoo!".into(), species: "PIDGEY".into(),
        }.into_script_command());
        assert_eq!(engine.call_function("bird", &[]).unwrap(), expected);
        assert_eq!(engine.tick(), expected);
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), Some(ScriptCommand::ShowText { text: "returned".into() }));
    }

    #[test]
    fn field_cry_continuation_boa_await_retains_all_segments_until_outer_close() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine
            .load_script(
                r#"export async function machop() {
          await game.fieldCryContinuation("first", "MACHOP", "paragraph", "tail");
          await game.showText("returned");
        }"#,
            )
            .unwrap();
        let expected = Some(
            PokemonScriptCommand::FieldCryContinuation {
                texts: vec!["first".into(), "paragraph".into(), "tail".into()],
                species: "MACHOP".into(),
            }
            .into_script_command(),
        );
        assert_eq!(engine.call_function("machop", &[]).unwrap(), expected);
        assert_eq!(
            engine.tick(),
            expected,
            "idle ticking cannot complete the continuation"
        );
        assert_eq!(
            engine.signal_done(CommandResult::Void).unwrap(),
            Some(ScriptCommand::ShowText {
                text: "returned".into(),
            })
        );
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), None);
    }

    #[test]
    fn field_cry_continuation_boa_rejects_wrong_arity_and_each_non_string_argument() {
        let mut invalid_calls = Vec::new();
        for count in [0, 1, 2, 3, 5] {
            invalid_calls.push((
                vec![r#""text""#; count].join(", "),
                "fieldCryContinuation: text, species, paragraph and continuation expected"
                    .to_string(),
            ));
        }
        for index in 0..4 {
            for invalid_value in ["false", "17", "null", "[]", "{}"] {
                let mut args = vec![r#""first""#, r#""MACHOP""#, r#""paragraph""#, r#""tail""#];
                args[index] = invalid_value;
                invalid_calls.push((
                    args.join(", "),
                    format!("fieldCryContinuation: argument {index} must be a string"),
                ));
            }
        }
        for (arguments, message) in invalid_calls {
            let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
            engine
                .load_script(&format!(
                    r#"export async function invalid() {{
              try {{
                await game.fieldCryContinuation({arguments});
                await game.showText("INVALID_PAYLOAD_ACCEPTED");
              }} catch (error) {{
                await game.showText(error.name + ":" + error.message);
              }}
            }}"#
                ))
                .unwrap();
            assert_eq!(
                engine.call_function("invalid", &[]).unwrap(),
                Some(ScriptCommand::ShowText {
                    text: format!("TypeError:{message}"),
                }),
                "arguments: {arguments}"
            );
            assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), None);
        }
    }

    #[test]
    fn jigglypuff_song_boa_await_does_not_return_before_do_not_wait_close() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"export async function song() {
          await game.jigglypuffSong("JIGGLYPUFF: Puu pupuu!", "3");
          await game.showText("returned");
        }"#).unwrap();
        let expected = Some(PokemonScriptCommand::JigglypuffSong {
            text: "JIGGLYPUFF: Puu pupuu!".into(), npc_id: "3".into(),
        }.into_script_command());
        assert_eq!(engine.call_function("song", &[]).unwrap(), expected);
        assert_eq!(engine.tick(), expected);
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(),
            Some(ScriptCommand::ShowText { text: "returned".into() }));
    }
    #[test]
    fn print_text_cry_boa_await_returns_only_after_the_shared_outer_close() {
        let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"export async function pet() {
          await game.printTextCry("SEEL: Kyuoo!", "SEEL");
          await game.showText("returned");
        }"#).unwrap();
        let expected = Some(PokemonScriptCommand::PrintTextCry {
            text: "SEEL: Kyuoo!".into(), species: "SEEL".into(),
        }.into_script_command());
        assert_eq!(engine.call_function("pet", &[]).unwrap(), expected);
        assert_eq!(engine.tick(), expected);
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(), Some(ScriptCommand::ShowText { text: "returned".into() }));
    }

    #[test]
    fn print_text_cry_boa_rejects_wrong_arity_and_non_string_payloads() {
        for (arguments, message) in [
            ("", "text and species expected"),
            (r#""text""#, "text and species expected"),
            (r#""text", "SEEL", false"#, "text and species expected"),
            (r#"false, "SEEL""#, "argument 0 must be a string"),
            (r#""text", 1"#, "argument 1 must be a string"),
        ] {
            let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
            engine.load_script(&format!(r#"export async function pet() {{
              try {{ await game.printTextCry({arguments}); await game.showText("INVALID_PAYLOAD_ACCEPTED"); }}
              catch (e) {{ await game.showText(String(e)); }}
            }}"#)).unwrap();
            assert_eq!(engine.call_function("pet", &[]).unwrap(), Some(ScriptCommand::ShowText {
                text: format!("TypeError: printTextCry: {message}"),
            }));
        }
    }

}

#[cfg(test)]
mod static_encounter_protocol_tests {
    use super::*;
    use dotzuki_engine_script::{ScriptEngine,CommandResult};
    #[test]
    fn static_encounter_boa_await_returns_the_actual_battle_outcome() {
        let mut e=ScriptEngine::with_api(&PokemonScriptApi);
        e.load_script(r#"export async function encounter() {
          const result=await game.staticEncounter("Gyaoo!", "ZAPDOS", 50);
          await game.showText(result);
        }"#).unwrap();
        let expected=Some(PokemonScriptCommand::StaticEncounter{text:"Gyaoo!".into(),species:crate::species::Species::Zapdos,level:50}.into_script_command());
        assert_eq!(e.call_function("encounter",&[]).unwrap(),expected);assert_eq!(e.tick(),expected);
        assert_eq!(e.signal_done(CommandResult::Text("caught".into())).unwrap(),Some(ScriptCommand::ShowText{text:"caught".into()}));
        for args in ["",r#""cry", "MEWTWO", 0"#,r#""cry", "MEWTWO", 70.5"#,r#""cry", "BAD", 70"#,r#""cry", "MEWTWO", NaN"#,r#"false, "MEWTWO", 70"#] {
            let mut e=ScriptEngine::with_api(&PokemonScriptApi);
            e.load_script(&format!(r#"export async function bad() {{try {{ await game.staticEncounter({args});await game.showText("accepted");}}catch(error){{await game.showText(error.name);}}}}"#)).unwrap();
            assert_eq!(e.call_function("bad",&[]).unwrap(),Some(ScriptCommand::ShowText{text:"TypeError".into()}));
        }
    }
}

#[cfg(test)]
mod pokedex_rating_protocol_tests {
    use super::*;
    use dotzuki_engine_script::{ScriptEngine,CommandResult};
    #[test]
    fn rating_boa_await_preserves_payload_and_rejects_invalid_types() {
        let mut engine=ScriptEngine::with_api(&PokemonScriptApi);
        engine.load_script(r#"export async function rating() {
          await game.pokedexRating("rating", 151);
          await game.showText("returned");
        }"#).unwrap();
        let expected=Some(PokemonScriptCommand::PokedexRating{text:"rating".into(),owned:151}.into_script_command());
        assert_eq!(engine.call_function("rating",&[]).unwrap(),expected);
        assert_eq!(engine.tick(),expected);
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(),Some(ScriptCommand::ShowText{text:"returned".into()}));
        assert_eq!(engine.signal_done(CommandResult::Void).unwrap(),None);
        for args in ["",r#""text""#,r#""text", -1"#,r#""text", 152"#,r#""text", 0.5"#,r#""text", NaN"#,r#""text", Infinity"#,r#""text", "1""#,r#"false, 1"#,r#""text", 1, 2"#] {
            let mut engine=ScriptEngine::with_api(&PokemonScriptApi);
            engine.load_script(&format!(r#"export async function bad() {{
              try {{ await game.pokedexRating({args}); await game.showText("accepted"); }}
              catch (error) {{ await game.showText(error.name); }}
            }}"#)).unwrap();
            assert_eq!(engine.call_function("bad",&[]).unwrap(),Some(ScriptCommand::ShowText{text:"TypeError".into()}),"args={args}");
        }
    }
}

#[cfg(test)]
mod field_yes_no_protocol_tests {
    use super::*;
    use dotzuki_engine_script::{ScriptEngine, CommandResult};
    #[test]
    fn boa_explicit_yes_no_suspends_assigns_numeric_result_and_rejects_coercion() {
        use crate::script_command::PokemonScriptCommand;
        for answer in [0.0, 1.0] {
            let mut engine = ScriptEngine::with_api(&PokemonScriptApi);
            engine
                .load_script(
                    r#"
                export async function question() {
                    const answer = await game.askFieldYesNo("Question?", "YES", "NO");
                    await game.showText(answer === 0 ? "YES branch" : "NO branch");
                }
                export async function invalid() {
                    try { await game.askFieldYesNo("Q", true, "NO"); }
                    catch (error) { await game.showText(error.name); }
                }
            "#,
                )
                .unwrap();
            let first = engine.call_function("question", &[]).unwrap();
            assert_eq!(
                first,
                Some(
                    PokemonScriptCommand::AskFieldYesNo {
                        question: "Question?".into(),
                        labels: ["YES".into(), "NO".into()],
                    }
                    .into_script_command()
                )
            );
            assert_eq!(engine.tick(), first, "the async owner remains suspended");
            assert_eq!(
                engine.signal_done(CommandResult::Number(answer)).unwrap(),
                Some(ScriptCommand::ShowText {
                    text: if answer == 0.0 {
                        "YES branch"
                    } else {
                        "NO branch"
                    }
                    .into()
                })
            );
            assert!(engine.signal_done(CommandResult::Void).unwrap().is_none());
            assert_eq!(
                engine.call_function("invalid", &[]).unwrap(),
                Some(ScriptCommand::ShowText {
                    text: "TypeError".into()
                })
            );
        }
    }
}

#[cfg(test)]
mod nurse_protocol_tests {
    use super::*;
    use dotzuki_engine_script::{CommandResult, ScriptEngine};
    #[test]
    fn boa_nurse_queries_canonical_seed_and_retains_numeric_await_then_farewell() {
        for used in [false, true] { for answer in [0., 1.] {
            let mut e = ScriptEngine::with_api(&PokemonScriptApi);
            e.seed_number("pokecenterUsed", if used { 1. } else { 0. });
            e.load_script(r#"export async function nurse() {
                const answer = await game.pokecenterNurseOffer(game.getPokecenterUsed(), "Welcome", "We heal", "Heal?", "HEAL", "CANCEL");
                await game.pokecenterFarewell(answer === 0 ? "Healed" : "Declined");
            }"#).unwrap();
            let first = Some(PokemonScriptCommand::PokecenterNurseOffer { used,
                texts: ["Welcome".into(), "We heal".into(), "Heal?".into(), "HEAL".into(), "CANCEL".into()] }.into_script_command());
            assert_eq!(e.call_function("nurse", &[]).unwrap(), first);
            assert_eq!(e.tick(), first);
            assert_eq!(e.signal_done(CommandResult::Number(answer)).unwrap(), Some(PokemonScriptCommand::PokecenterFarewell {
                text: if answer == 0. { "Healed" } else { "Declined" }.into()
            }.into_script_command()));
            assert!(e.signal_done(CommandResult::Void).unwrap().is_none());
        }}
        for (name, args) in [("pokecenterNurseOffer", r#"0, "Welcome", "We heal", "Heal?", "HEAL", "CANCEL""#),
            ("pokecenterNurseOffer", r#"false, "Welcome", "We heal", "Heal?", "HEAL""#),
            ("pokecenterFarewell", "false"), ("pokecenterFarewell", r#""Bye", "extra""#)] {
            let mut e = ScriptEngine::with_api(&PokemonScriptApi);
            e.load_script(&format!(r#"export async function bad() {{
                try {{ await game.{name}({args}); await game.showText("accepted"); }}
                catch (error) {{ await game.showText(error.name); }}
            }}"#)).unwrap();
            assert_eq!(e.call_function("bad", &[]).unwrap(), Some(ScriptCommand::ShowText { text: "TypeError".into() }));
        }
    }
}
