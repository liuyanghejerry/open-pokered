//! Real ball/menu completion must deliver the capture result to map scripts.
//! These are core caller regressions, not ordinary Game/SAVE recordings.

use super::settlement::{settle_battle_into_save, BattleOutcome, CapturedLocation};
use super::{BattleInput, BattlePhase, BattleScreen};
use crate::alloc_prelude::*;
use crate::game_state::{GameScreen, ScreenAction};
use crate::overworld::{Direction, OverworldInput, OverworldScreen};
use crate::pokemon::stats::{create_pokemon, create_pokemon_with_moves};
use crate::save::SaveData;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;

fn a() -> BattleInput {
    BattleInput {
        a: true,
        ..BattleInput::none()
    }
}

fn field_input(a: bool) -> OverworldInput {
    OverworldInput::new(false, false, false, false, a, false, false, false)
}

fn finish_battle(battle: &mut BattleScreen) {
    for frame in 0..1000 {
        let action = battle.update_frame(if frame % 2 == 0 {
            a()
        } else {
            BattleInput::none()
        });
        if matches!(action, ScreenAction::Transition(GameScreen::Overworld)) {
            assert!(battle.settlement.is_some());
            return;
        }
    }
    panic!("battle did not finish: {:?}", battle.phase);
}

fn throw_first_ball(battle: &mut BattleScreen) {
    assert_eq!(battle.phase, BattlePhase::PlayerMenu);
    battle.update_frame(BattleInput {
        down: true,
        ..BattleInput::none()
    });
    battle.update_frame(a());
    assert_eq!(battle.phase, BattlePhase::BagSelect);
    battle.update_frame(a());
}

fn awakened_snorlax(map: MapId, boxed: bool) -> (OverworldScreen, SaveData, BattleScreen) {
    let mut save = SaveData::new();
    let player = create_pokemon(Species::Bulbasaur, 6, [0x9a, 0x78]).unwrap();
    for _ in 0..if boxed { 6 } else { 1 } {
        save.party.add(player).unwrap();
    }
    save.game_data.bag.add_item(ItemId::MasterBall, 2).unwrap();
    save.game_data.bag.add_item(ItemId::PokeFlute, 1).unwrap();
    if boxed {
        save.current_box.deposit(player).unwrap();
        save.sync_current_box_to_storage();
    }

    let mut overworld = OverworldScreen::new(map, None, PokemonRedData);
    let (x, y, facing) = if map == MapId::Route12 {
        (10, 63, Direction::Up)
    } else {
        (27, 10, Direction::Left)
    };
    overworld.state.player.x = x;
    overworld.state.player.y = y;
    overworld.run_on_load();
    for _ in 0..100 {
        overworld.update_frame(field_input(false));
    }
    overworld.seed_script_query_state(0, &["POKE_FLUTE".to_string()], 0, 0, 0, 0, &[], 0, 0, 0);
    overworld.use_field_item(ItemId::PokeFlute, MapId::PalletTown);
    overworld.state.player.facing = facing;
    overworld.update_frame(field_input(true));
    for frame in 0..300 {
        if overworld.pending_wild_encounter.is_some() {
            break;
        }
        if let Some(dialogue) = overworld.pending_dialogue.as_mut() {
            dialogue.skip_to_full_page();
        }
        overworld.update_frame(field_input(frame % 2 == 1));
    }
    let encounter = overworld
        .pending_wild_encounter
        .take()
        .expect("flute starts the actual static scene battle");
    assert_eq!(encounter.species, Species::Snorlax);
    assert_eq!(encounter.level, 30);
    assert!(overworld.script_awaiting_battle);
    let enemy = create_pokemon(encounter.species, encounter.level, [0x9a, 0x78]).unwrap();
    let mut battle = BattleScreen::from_parties(true, &save.party.to_vec(), &[enemy], None);
    battle.player_bag = save.game_data.bag.clone();
    battle.player_box_full = save.current_box.is_full();
    battle.map_id = map as u8;
    // Start at the public battle menu; rendering/intro duration is outside this test.
    battle.phase = BattlePhase::PlayerMenu;
    (overworld, save, battle)
}

#[test]
fn real_snorlax_capture_settles_and_resumes_without_mountains_text() {
    for (map, boxed, flag, slot) in [
        (MapId::Route12, false, "EVENT_BEAT_ROUTE12_SNORLAX", 0),
        (MapId::Route16, true, "EVENT_BEAT_ROUTE16_SNORLAX", 6),
    ] {
        let (mut overworld, mut save, mut battle) = awakened_snorlax(map, boxed);
        let old_party = save.party.clone();
        let old_box = save.current_box.clone();
        assert!(
            !overworld.npc_states[slot].visible,
            "Snorlax hides before battle"
        );
        throw_first_ball(&mut battle);
        assert_eq!(
            battle.captured_mon.as_ref().unwrap().species,
            Species::Snorlax
        );
        assert!(
            battle.settlement.is_none(),
            "text must finish before settlement"
        );
        finish_battle(&mut battle);
        assert_eq!(
            battle.settlement.as_ref().unwrap().outcome,
            BattleOutcome::Captured
        );
        let result = settle_battle_into_save(&mut battle, &mut save, &mut overworld);
        assert_eq!(result.outcome, Some("caught"));
        assert_eq!(
            result.captured.unwrap().location,
            if boxed {
                CapturedLocation::CurrentBox(0)
            } else {
                CapturedLocation::Party(1)
            }
        );
        for i in 0..old_party.count() {
            assert_eq!(save.party.get(i), old_party.get(i));
        }
        if boxed {
            assert_eq!(save.current_box.get(1), old_box.get(0));
        }
        assert!(save.game_data.pokedex.is_owned(Species::Snorlax));
        assert_eq!(
            save.game_data.bag.items(),
            &[(ItemId::MasterBall, 1), (ItemId::PokeFlute, 1)]
        );
        assert!(
            battle.captured_mon.is_none(),
            "only writeback takes the captured mon"
        );

        // Use the real returned outcome, never an injected "caught" or settlement.
        overworld.resume_script_after_battle(result.outcome.unwrap());
        for _ in 0..60 {
            overworld.update_frame(field_input(false));
            assert!(
                overworld.pending_dialogue.is_none(),
                "a catch must not say Snorlax returned to the mountains"
            );
        }
        assert_eq!(overworld.script_flags().get(flag), Some(&true));
        assert!(!overworld.npc_states[slot].visible);
        overworld.warp_to_map(MapId::PalletTown, 5, 6);
        overworld.warp_to_map(
            map,
            if boxed { 27 } else { 10 },
            if boxed { 10 } else { 63 },
        );
        assert!(
            !overworld.npc_states[slot].visible,
            "removal survives map re-entry"
        );
    }
}

#[test]
fn safari_ball_capture_uses_the_same_captured_settlement() {
    let player = create_pokemon(Species::Bulbasaur, 6, [0x9a, 0x78]).unwrap();
    let mut enemy = create_pokemon(Species::NidoranF, 22, [0x9a, 0x78]).unwrap();
    // A constructed weakened state makes the real formula certain for every
    // accepted Safari Rand1/Rand2; this is not an ordinary Safari spawn claim.
    enemy.hp = 1;
    let mut battle = BattleScreen::from_parties(true, &[player], &[enemy], None);
    battle.is_safari = true;
    battle.safari = Some(super::safari::SafariState::new(235, 30));
    battle.phase = BattlePhase::PlayerMenu;
    battle.update_frame(a()); // BALL, through the actual Safari menu caller.
    assert!(battle.captured_mon.is_some());
    assert_eq!(battle.safari.as_ref().unwrap().balls, 29);
    finish_battle(&mut battle);
    assert_eq!(
        battle.settlement.as_ref().unwrap().outcome,
        BattleOutcome::Captured
    );
    let mut save = SaveData::new();
    save.party.add(player).unwrap();
    let mut overworld = OverworldScreen::new(MapId::SafariZoneCenter, None, PokemonRedData);
    let result = settle_battle_into_save(&mut battle, &mut save, &mut overworld);
    assert_eq!(result.outcome, Some("caught"));
    assert_eq!(save.party.get(1).unwrap().species, Species::NidoranF);
}

#[test]
fn full_box_refusal_cannot_publish_a_capture_or_complete_the_static_event() {
    let (overworld, mut save, mut battle) = awakened_snorlax(MapId::Route12, true);
    let old = create_pokemon(Species::Bulbasaur, 6, [0x9a, 0x78]).unwrap();
    while !save.current_box.is_full() {
        save.current_box.deposit(old).unwrap();
    }
    battle.player_box_full = save.current_box.is_full();
    let protected = serde_json::to_value(battle.battle_state.as_ref().unwrap()).unwrap();
    let bag = battle.player_bag.clone();
    throw_first_ball(&mut battle);
    assert!(battle.current_message.as_deref().unwrap().contains("BOX"));
    for frame in 0..100 {
        if battle.phase == BattlePhase::BagSelect {
            break;
        }
        assert!(matches!(
            battle.update_frame(if frame % 2 == 0 {
                a()
            } else {
                BattleInput::none()
            }),
            ScreenAction::Continue
        ));
    }
    assert_eq!(battle.phase, BattlePhase::BagSelect);
    assert!(battle.settlement.is_none());
    assert!(battle.captured_mon.is_none());
    assert_eq!(battle.player_bag.items(), bag.items());
    assert_eq!(
        serde_json::to_value(battle.battle_state.as_ref().unwrap()).unwrap(),
        protected
    );
    assert!(overworld.script_awaiting_battle);
    assert_ne!(
        overworld.script_flags().get("EVENT_BEAT_ROUTE12_SNORLAX"),
        Some(&true)
    );
    assert!(!overworld.npc_states[0].visible);
}

#[test]
fn non_capture_fight_run_blackout_and_demo_keep_their_outcomes() {
    let strong = create_pokemon_with_moves(
        Species::Mewtwo,
        100,
        [0xff, 0xff],
        [MoveId::Swift, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    let mut weak = create_pokemon_with_moves(
        Species::Rattata,
        5,
        [0x9a, 0x78],
        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
    )
    .unwrap();
    weak.hp = 1;
    for kind in ["win", "ran", "lose", "demo"] {
        let (player, enemy) = if kind == "lose" {
            (weak, strong)
        } else {
            (strong, weak)
        };
        let mut battle = BattleScreen::from_parties(true, &[player], &[enemy], None);
        battle.phase = BattlePhase::PlayerMenu;
        if kind == "ran" {
            battle.update_frame(BattleInput {
                right: true,
                ..BattleInput::none()
            });
            battle.update_frame(BattleInput {
                down: true,
                ..BattleInput::none()
            });
            battle.update_frame(a());
        } else if kind == "demo" {
            battle.is_old_man = true;
            battle.update_frame(BattleInput::none());
        } else {
            battle.update_frame(a()); // FIGHT
            battle.update_frame(a()); // actual Swift / enemy's faster Swift
        }
        finish_battle(&mut battle);
        assert!(battle.captured_mon.is_none(), "{kind}");
        let mut save = SaveData::new();
        save.party.add(player).unwrap();
        let mut overworld = OverworldScreen::new(MapId::Route12, None, PokemonRedData);
        let result = settle_battle_into_save(&mut battle, &mut save, &mut overworld);
        assert_eq!(
            result.outcome,
            Some(if kind == "demo" { "win" } else { kind }),
            "{kind}"
        );
        assert!(result.captured.is_none(), "{kind}");
        assert_eq!(overworld.pending_warp.is_some(), kind == "lose", "{kind}");
    }
}
