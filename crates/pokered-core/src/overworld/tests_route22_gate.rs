use super::OverworldScreen;
use crate::{
    hash_compat::HashMap,
    save::{game_data::ROUTE22_GATE_SCRIPT_INDEX, SaveData},
};
use pokered_data::{impl_traits::PokemonRedData, maps::MapId};

#[test]
fn route22_permission_loads_from_original_script_byte_and_ignores_stale_sidecar() {
    for script in [0, 1, 2] {
        let mut save = SaveData::new();
        save.game_data.game_progress_flags[ROUTE22_GATE_SCRIPT_INDEX] = script;
        let mut old_sidecar = HashMap::default();
        old_sidecar.insert("SCRIPT_ROUTE22GATE_NOOP".into(), script != 2);
        let mut screen = OverworldScreen::new(MapId::Route22Gate, None, PokemonRedData);
        screen.restore_loaded_save_flags(&save, Some(old_sidecar));
        assert_eq!(
            screen.unified_flags().get_flag("SCRIPT_ROUTE22GATE_NOOP"),
            script == 2
        );
    }
}

#[test]
fn route22_permission_writes_only_its_original_saved_script_slot() {
    for admitted in [false, true] {
        let mut data = SaveData::new().game_data;
        data.game_progress_flags.fill(0xa5);
        let mut screen = OverworldScreen::new(MapId::Route22Gate, None, PokemonRedData);
        screen.restore_system_save_state(&data);
        screen.set_flag_live("SCRIPT_ROUTE22GATE_NOOP", admitted);
        let mut expected = data.clone();
        screen.write_system_save_state(&mut expected);
        // The existing bedroom mapper also normalizes its own script byte.
        data.game_progress_flags[crate::save::game_data::REDS_HOUSE_2F_SCRIPT_INDEX] = 1;
        data.game_progress_flags[ROUTE22_GATE_SCRIPT_INDEX] = if admitted { 2 } else { 0 };
        assert_eq!(expected.game_progress_flags, data.game_progress_flags);
    }
}
