use boa_engine::{Context, JsArgs, JsResult, JsValue};
use dotzuki_engine_script::{BridgeView, ScriptApiRegistrar, ScriptCommand};

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
                Ok(ScriptCommand::Custom {
                    name: "startBattleSet".to_string(),
                    args: vec![
                        serde_json::Value::String(trainer_id),
                        base,
                    ],
                })
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
                Ok(ScriptCommand::Custom {
                    name: "oldManTutorial".to_string(),
                    args: vec![],
                })
            },
        );

        // game.tradePokemon(offered, received, nickname) -> Promise<bool>
        // Resolves true if the party held `offered` (removed + `received` added).
        engine.register_async_fn(
            "tradePokemon",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "tradePokemon".to_string(),
                    args: vec![
                        args.get_or_undefined(0).to_json(ctx)?,
                        args.get_or_undefined(1).to_json(ctx)?,
                        args.get_or_undefined(2).to_json(ctx)?,
                    ],
                })
            },
        );

        // game.showPokedexEntry(species: string) -> Promise<void>
        engine.register_async_fn(
            "showPokedexEntry",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "showPokedexEntry".to_string(),
                    args: vec![args.get_or_undefined(0).to_json(ctx)?],
                })
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
                Ok(ScriptCommand::Custom {
                    name: "replaceTileBlock".to_string(),
                    args: vec![
                        args.get_or_undefined(0).to_json(ctx)?,
                        args.get_or_undefined(1).to_json(ctx)?,
                        args.get_or_undefined(2).to_json(ctx)?,
                    ],
                })
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
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let lucky = args.get_or_undefined(0).to_json(ctx)?;
                Ok(ScriptCommand::Custom {
                    name: "openSlots".to_string(),
                    args: vec![lucky],
                })
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
                Ok(ScriptCommand::Custom {
                    name: "elevatorMenu".to_string(),
                    args: vec![floors],
                })
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
                Ok(ScriptCommand::Custom {
                    name: "filterBag".to_string(),
                    args: vec![item_ids],
                })
            },
        );

        // game.showDiploma() -> Promise<void>
        // Shows the full-screen completed-POKeDEX diploma; the app closes it on
        // A/B. The script continues immediately after.
        engine.register_async_fn(
            "showDiploma",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "showDiploma".to_string(),
                    args: vec![],
                })
            },
        );

        // game.openPC() -> Promise<void>
        // Opens the Pokémon Center PC (engine/menus/pc.asm ActivatePC): the
        // full menu with BILL's PC (#MON storage), the player's item PC, and
        // PROF.OAK's PC (#DEX rating). The script continues immediately.
        engine.register_async_fn(
            "openPC",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "openPC".to_string(),
                    args: vec![],
                })
            },
        );

        // game.openItemPC() -> Promise<void>
        // Opens the bedroom PC (engine/menus/players_pc.asm PlayerPC direct):
        // item storage only, no main menu.
        engine.register_async_fn(
            "openItemPC",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "openItemPC".to_string(),
                    args: vec![],
                })
            },
        );

        // game.openBillsPC() -> Promise<void>
        // Opens Bill's house PC directly (home/map_objects.asm TextScript_BillsPC
        // -> engine/pokemon/bills_pc.asm BillsPC_): "Switch on!" then straight
        // into the #MON storage system, no main menu.
        engine.register_async_fn(
            "openBillsPC",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "openBillsPC".to_string(),
                    args: vec![],
                })
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
                Ok(ScriptCommand::Custom {
                    name: "linkStart".to_string(),
                    args: vec![],
                })
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
                Ok(ScriptCommand::Custom {
                    name: "enterHallOfFame".to_string(),
                    args: vec![],
                })
            },
        );

        // game.giveCoins(amount: number) -> Promise<void>
        // Credit the player's Game Corner coin balance (capped at 9999).
        engine.register_async_fn(
            "giveCoins",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let amount = args.get_or_undefined(0).to_u32(ctx)?.min(u16::MAX as u32) as u16;
                Ok(ScriptCommand::Custom {
                    name: "giveCoins".to_string(),
                    args: vec![serde_json::json!(amount)],
                })
            },
        );

        // game.takeCoins(amount: number) -> Promise<void>
        engine.register_async_fn(
            "takeCoins",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                let amount = args.get_or_undefined(0).to_u32(ctx)?.min(u16::MAX as u32) as u16;
                Ok(ScriptCommand::Custom {
                    name: "takeCoins".to_string(),
                    args: vec![serde_json::json!(amount)],
                })
            },
        );

        // game.depositDaycare(index: number) -> Promise<void>
        // Deposit the party member at the 0-based `index` into the Day Care.
        engine.register_async_fn(
            "depositDaycare",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "depositDaycare".to_string(),
                    args: vec![args.get_or_undefined(0).to_json(ctx)?],
                })
            },
        );

        // game.withdrawDaycare() -> Promise<void>
        // Withdraw the Day Care Pokémon back into the party at its grown level.
        engine.register_async_fn(
            "withdrawDaycare",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "withdrawDaycare".to_string(),
                    args: vec![],
                })
            },
        );

        // ── Game-side re-registrations: these verbs were registered by the
        // core engine before the `Custom` refactor dropped their dedicated
        // variants; games must now register them. JS names unchanged. ────────

        // game.playShipDeparture() -> Promise<void>
        // The S.S. Anne departure cutscene (VermilionDock): the blocking
        // ship-sail animation (smoke puffs + view scroll + erase).
        engine.register_async_fn(
            "playShipDeparture",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "playShipDeparture".to_string(),
                    args: vec![],
                })
            },
        );

        // game.animateHealingMachine() -> Promise<void>
        // Pokémon Center healing-machine animation.
        engine.register_async_fn(
            "animateHealingMachine",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "animateHealingMachine".to_string(),
                    args: vec![],
                })
            },
        );

        // game.openNamingScreen(species: string) -> Promise<string>
        // Opens the naming screen for a newly caught/hatched `species`;
        // resolves to the chosen name.
        engine.register_async_fn(
            "openNamingScreen",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "openNamingScreen".to_string(),
                    args: vec![args.get_or_undefined(0).to_json(ctx)?],
                })
            },
        );

        // game.choosePartyPokemon() -> Promise<number>
        // Opens the party menu as a selector (Name Rater); resolves to the
        // chosen 0-based index, or -1 on cancel.
        engine.register_async_fn(
            "choosePartyPokemon",
            |_args: &[JsValue], _ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "choosePartyPokemon".to_string(),
                    args: vec![],
                })
            },
        );

        // game.setPartyNickname(index: number, nickname: string) -> Promise<void>
        // Writes a new nickname onto the party member at `index`.
        engine.register_async_fn(
            "setPartyNickname",
            |args: &[JsValue], ctx: &mut Context| -> JsResult<ScriptCommand> {
                Ok(ScriptCommand::Custom {
                    name: "setPartyNickname".to_string(),
                    args: vec![
                        args.get_or_undefined(0).to_json(ctx)?,
                        args.get_or_undefined(1).to_json(ctx)?,
                    ],
                })
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
    use super::PokemonScriptApi;
    use dotzuki_engine_script::{CommandResult, ScriptCommand, ScriptEngine};

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
}
