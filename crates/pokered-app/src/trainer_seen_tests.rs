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
        g.register_active_trainer_species_seen();
        assert!(g.save_data.game_data.pokedex.is_seen(Species::Rattata));
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Pikachu));
        assert!(!g.save_data.game_data.pokedex.is_seen(Species::Ekans));
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
            g.register_active_trainer_species_seen();
            assert_eq!(g.save_data.game_data.pokedex.seen_count(), 0);
            assert_eq!(g.save_data.game_data.pokedex.owned_count(), 0);
        }
    });
}
