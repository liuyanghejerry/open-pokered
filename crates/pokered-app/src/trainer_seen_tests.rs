use super::*;
use pokered_core::battle::BattlePhase;
use pokered_core::pokemon::pokedex::Pokedex;
use pokered_data::{species::Species, trainer_data::{make_trainer_id, TrainerClass}};

fn on_game_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024)
        .spawn(test).unwrap().join().unwrap();
}

fn trainer_game() -> PokemonGame {
    let mut g = PokemonGame::new_with_options(
        GameVersion::Red, None, None, None, false, None, false, true,
        #[cfg(feature = "debug-server")]
        None,
    );
    g.save_data = SaveData::new();
    let mon = pokered_core::pokemon::stats::create_pokemon(
        Species::Bulbasaur, 30, [0x9A, 0x78],
    ).unwrap();
    g.save_data.party.add(mon).unwrap();
    g.save_data.game_data.pokedex.set_owned(Species::Bulbasaur);
    g.start_trainer_battle(&make_trainer_id(TrainerClass::Youngster, 1), None);
    g.state.screen = GameScreen::Battle;
    g
}

#[test]
fn trainer_seen_registers_only_the_loaded_first_enemy() {
    on_game_stack(|| {
        let mut g = trainer_game();
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Rattata));
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        assert!(!g.save_data.game_data.pokedex.is_owned(Species::Rattata));
        assert!(!g.save_data.game_data.pokedex.is_owned(Species::Ekans));
        for _ in 0..120 { g.update(&InputState::new()); }
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
    });
}

#[test]
fn trainer_seen_registers_replacement_only_after_the_real_send_next_handler() {
    on_game_stack(|| {
        let mut g = trainer_game();
        g.battle.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
        g.battle.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        // The battle's normal post-text handler loads the next living slot.
        g.battle.post_text_transition();
        assert_eq!(g.battle.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 1);
        g.update(&InputState::new());
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        assert!(!g.save_data.game_data.pokedex.is_owned(Species::Ekans));
        assert_eq!(g.battle.battle_state.as_ref().unwrap().enemy.party[1].hp,
            g.battle.battle_state.as_ref().unwrap().enemy.party[1].max_hp);
    });
}

#[test]
fn trainer_seen_uses_original_party_identity_during_transform() {
    on_game_stack(|| {
        let mut g = trainer_game();
        g.save_data.game_data.pokedex = Pokedex::new();
        let enemy = &mut g.battle.battle_state.as_mut().unwrap().enemy;
        enemy.preserve_original_identity();
        enemy.active_mon_mut().species = Species::Pikachu;
        // A restored/just-loaded enemy must use the canonical original identity.
        g.battle.loaded_enemy_slots = 1;
        g.register_loaded_trainer_species_seen();
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Rattata));
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Pikachu));
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
    });
}

#[test]
fn trainer_seen_first_slot_zero_is_registered_again_for_each_new_battle() {
    on_game_stack(|| {
        let mut g = trainer_game();
        assert_eq!(g.battle.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 0);
        for class in [TrainerClass::Youngster, TrainerClass::BugCatcher, TrainerClass::Youngster] {
            g.save_data.game_data.pokedex = Pokedex::new();
            g.start_trainer_battle(&make_trainer_id(class, 1), None);
            let enemy = &g.battle.battle_state.as_ref().unwrap().enemy;
            assert_eq!(enemy.active_pokemon_index, 0);
            let species = enemy.party_mon(0).species;
            assert!(g.save_data.game_data.pokedex.is_seen(species));
            assert_eq!(g.save_data.game_data.pokedex.seen_count(), 1);
            assert_eq!(g.save_data.game_data.pokedex.owned_count(), 0);
        }
    });
}

#[test]
fn trainer_seen_tracks_real_ai_switches_back_to_an_already_seen_slot() {
    on_game_stack(|| {
        let mut g = trainer_game();
        let mut mon = pokered_core::pokemon::stats::create_pokemon(
            Species::Bulbasaur, 100, [0x9A, 0x78],
        ).unwrap();
        mon.moves = [pokered_data::moves::MoveId::Growl; 4];
        mon.pp = [40; 4];
        g.save_data.party = Default::default();
        g.save_data.party.add(mon).unwrap();
        g.start_trainer_battle(&make_trainer_id(TrainerClass::Juggler, 4), None);
        // Exercise real AI switches without coupling this Pokédex assertion to
        // damage-variance draws or the opponent knocking the player out.
        for enemy in &mut g.battle.battle_state.as_mut().unwrap().enemy.party {
            enemy.moves = [pokered_data::moves::MoveId::Growl; 4];
            enemy.pp = [40; 4];
        }
        g.battle.rng = pokered_core::battle::pokered_rules::runtime::StdBattleRng::from_seed(42);
        g.state.config.battle_animation = false;
        let mut slots = vec![0];
        for frame in 0..6000 {
            let mut input = InputState::new();
            if frame % 2 == 0 { input.press(GbButton::A); }
            g.update(&input);
            let enemy = &g.battle.battle_state.as_ref().unwrap().enemy;
            let active = enemy.active_pokemon_index;
            if active != *slots.last().unwrap() {
                assert!(enemy.party.iter().all(|mon| mon.hp > 0), "AI switch, not KO replacement");
                assert!(g.save_data.game_data.pokedex.is_seen(enemy.party_mon(active).species));
                slots.push(active);
                if slots.len() == 3 { break; }
            }
        }
        assert_eq!(slots, vec![0, 1, 0]);
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Drowzee));
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Hypno));
        assert!(!g.save_data.game_data.pokedex.is_owned(Species::Drowzee));
        assert!(!g.save_data.game_data.pokedex.is_owned(Species::Hypno));
    });
}

#[test]
fn trainer_seen_snapshot_restore_rebuilds_slot_zero_and_replacement_registration() {
    on_game_stack(|| {
        let mut g = trainer_game();
        for slot in [0, 1] {
            if slot == 1 {
                g.battle.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
                g.battle.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
                g.battle.post_text_transition();
                g.update(&InputState::new());
            }
            let mut snapshot = g.agent_save_state().unwrap();
            // A legacy snapshot has no queued-load field. Reconstruct only
            // its canonical current enemy, including the first slot zero.
            snapshot.save_data.game_data.pokedex = Pokedex::new();
            let mut legacy = serde_json::to_value(snapshot).unwrap();
            legacy["battle"].as_object_mut().unwrap().remove("loaded_enemy_slots");
            let snapshot = serde_json::from_value(legacy).unwrap();
            g.agent_restore_state(&snapshot).unwrap();
            assert_eq!(g.save_data.game_data.pokedex.seen_count(), 0);
            g.update(&InputState::new());
            let species = if slot == 0 { Species::Rattata } else { Species::Ekans };
            assert!(g.save_data.game_data.pokedex.is_seen(species));
            assert_eq!(g.save_data.game_data.pokedex.seen_count(), 1);
        }
        g.save_data.game_data.pokedex = Pokedex::new();
        g.battle.loaded_enemy_slots = 1 << 1;
        g.agent_save_state_slot(7).unwrap();
        g.register_loaded_trainer_species_seen();
        g.agent_restore_state_slot(7).unwrap();
        g.update(&InputState::new());
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Ekans));
    });
}

#[test]
fn trainer_seen_ordinary_load_and_soft_reset_abandon_old_load_notifications() {
    on_game_stack(|| {
        let dir = std::env::temp_dir().join(format!("pokered-trainer-seen-906-{}", std::process::id()));
        assert!(!dir.exists());
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("ordinary.sav");
        let mut g = trainer_game();
        g.save_to_path(&path).unwrap();
        g.battle.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
        g.battle.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
        g.battle.post_text_transition();
        assert_ne!(g.battle.loaded_enemy_slots, 0);
        g.load_from_path(&path).unwrap();
        assert_eq!(g.battle.loaded_enemy_slots, 0);
        g.update(&InputState::new());
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        g.battle.loaded_enemy_slots = 1 << 1;
        g.save_path = Some(dir.join("absent.sav"));
        g.soft_reset();
        assert_eq!(g.battle.loaded_enemy_slots, 0);
        assert_eq!(g.state.screen, GameScreen::TitleScreen);
        std::fs::remove_dir_all(dir).unwrap();
    });
}

#[test]
fn trainer_seen_does_not_add_link_or_wild_seen_flags() {
    on_game_stack(|| {
        let mut g = trainer_game();
        for (wild, link) in [(false, true), (true, false)] {
            g.save_data.game_data.pokedex = Pokedex::new();
            g.battle.is_wild = wild;
            g.battle.link_mode = link;
            g.battle.loaded_enemy_slots = 1;
            g.register_loaded_trainer_species_seen();
            assert_eq!(g.battle.loaded_enemy_slots, 0);
            assert_eq!(g.save_data.game_data.pokedex.seen_count(), 0);
            assert_eq!(g.save_data.game_data.pokedex.owned_count(), 0);
        }
    });
}

#[test]
fn trainer_seen_snapshot_preserves_multiple_actual_loads_before_frontend_consumption() {
    on_game_stack(|| {
        let mut g = trainer_game();
        let make = |species| pokered_core::pokemon::stats::create_pokemon(
            species, 11, [0x9A, 0x78],
        ).unwrap();
        g.battle = pokered_core::battle::BattleScreen::from_parties(
            false,
            &[make(Species::Bulbasaur)],
            &[make(Species::Rattata), make(Species::Ekans), make(Species::Sandshrew)],
            Some(TrainerClass::Youngster),
        );
        g.register_loaded_trainer_species_seen();
        for slot in [0, 1] {
            g.battle.battle_state.as_mut().unwrap().enemy.party[slot].hp = 0;
            g.battle.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
            g.battle.post_text_transition();
        }
        assert_eq!(g.battle.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 2);
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Sandshrew));
        let value = serde_json::to_string(&g.agent_save_state().unwrap()).unwrap();
        g.register_loaded_trainer_species_seen();
        g.agent_restore_state(&serde_json::from_str(&value).unwrap()).unwrap();
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        g.update(&InputState::new());
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Ekans));
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Sandshrew));
        assert_eq!(g.battle.loaded_enemy_slots, 0);
    });
}
