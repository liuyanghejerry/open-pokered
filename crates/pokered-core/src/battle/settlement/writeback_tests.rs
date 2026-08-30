//! Tests for [`settle_battle_into_save`] — the shared post-battle → save writeback.

use super::{settle_battle_into_save, BattleOutcome, BattleSettlement};
use crate::battle::BattleScreen;
use crate::overworld::screen::OverworldScreen;
use crate::pokemon::party::Party;
use crate::pokemon::stats::create_pokemon_with_moves;
use crate::save::SaveData;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::trainer_data::TrainerClass;

fn mon(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
    create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
}

fn settlement(outcome: BattleOutcome, gained: u32, lost: u32) -> BattleSettlement {
    BattleSettlement {
        outcome,
        money_gained: gained,
        money_lost: lost,
        payday_bonus: 0,
        exp_entries: vec![],
        level_ups: vec![],
        evolutions: vec![],
    }
}

fn overworld() -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(MapId::Route1, None, PokemonRedData)
}

/// A trainer win adds the prize money and returns "win".
#[test]
fn win_adds_prize_money() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Rattata, 8)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Youngster));
    battle.settlement = Some(settlement(BattleOutcome::Win, 200, 0));
    let mut save = SaveData::new();
    save.game_data.player_money = 1000;
    let mut ow = overworld();

    let outcome = settle_battle_into_save(&mut battle, &mut save, &mut ow).outcome;
    assert_eq!(outcome, Some("win"));
    assert_eq!(save.game_data.player_money, 1200, "prize money added");
}

/// A loss subtracts money, heals + warps to the last Poké Center, and returns "lose".
#[test]
fn loss_triggers_blackout() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Onix, 12)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Brock));
    battle.map_id = MapId::PewterCity as u8; // not Oak's Lab
    battle.settlement = Some(settlement(BattleOutcome::Loss, 0, 500));
    let mut save = SaveData::new();
    save.game_data.player_money = 1000;
    let mut ow = overworld();

    let outcome = settle_battle_into_save(&mut battle, &mut save, &mut ow).outcome;
    assert_eq!(outcome, Some("lose"));
    assert_eq!(save.game_data.player_money, 500, "blackout money penalty applied");
    assert!(ow.heal_requested, "party heal requested on blackout");
    assert!(ow.pending_warp.is_some(), "warped to the last Poké Center");
    assert!(
        matches!(
            ow.warp_fade_state,
            crate::overworld::WarpFadeState::FadingOut { .. }
        ),
        "blackout must start the fade-out — the update loop only commits a queued \
         warp from the BlackScreen phase; queueing without the fade soft-locks \
         (warp triggers bail while pending_warp.is_some())"
    );
}

/// The blackout warp lands at `wLastBlackoutMap`'s FLY point — outside the
/// Pokémon Center of the last city healed at (special_warps.asm:66-81).
#[test]
fn blackout_warps_to_last_blackout_maps_fly_point() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Onix, 12)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Brock));
    battle.map_id = MapId::PewterGym as u8;
    battle.settlement = Some(settlement(BattleOutcome::Loss, 0, 0));
    let mut save = SaveData::new();
    save.game_data.last_blackout_map = MapId::PewterCity as u8;
    let mut ow = overworld();

    settle_battle_into_save(&mut battle, &mut save, &mut ow);
    let warp = ow.pending_warp.expect("blackout warp");
    assert_eq!(warp.dest_map, MapId::PewterCity);
    assert_eq!((warp.dest_x, warp.dest_y), (13, 26), "Pewter fly point");
    assert!(!warp.save_last_map);
}

/// Before any heal, `wLastBlackoutMap` is 0 = PALLET_TOWN (wram init): the
/// blackout warp lands in front of the player's house.
#[test]
fn blackout_defaults_to_pallet_town() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Onix, 12)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Brock));
    battle.map_id = MapId::PewterGym as u8;
    battle.settlement = Some(settlement(BattleOutcome::Loss, 0, 0));
    let mut save = SaveData::new();
    assert_eq!(save.game_data.last_blackout_map, 0);
    let mut ow = overworld();

    settle_battle_into_save(&mut battle, &mut save, &mut ow);
    let warp = ow.pending_warp.expect("blackout warp");
    assert_eq!(warp.dest_map, MapId::PalletTown);
    assert_eq!((warp.dest_x, warp.dest_y), (5, 6), "Pallet Town fly point");
}

/// The blackout warp actually commits when the overworld update loop runs:
/// fade-out → BlackScreen → commit → fade-in, landing at the fly point.
/// Regression test for the soft-lock where the queued warp never fired.
#[test]
fn blackout_warp_commits_through_the_update_loop() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Onix, 12)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Brock));
    battle.map_id = MapId::PewterGym as u8;
    battle.settlement = Some(settlement(BattleOutcome::Loss, 0, 0));
    let mut save = SaveData::new();
    let mut ow = overworld();

    settle_battle_into_save(&mut battle, &mut save, &mut ow);
    let input = crate::overworld::OverworldInput::new(
        false, false, false, false, false, false, false, false,
    );
    for _ in 0..(24 + 1 + 24 + 20) {
        ow.update_frame(input);
    }
    assert!(ow.pending_warp.is_none(), "blackout warp committed");
    assert!(matches!(
        ow.warp_fade_state,
        crate::overworld::WarpFadeState::Idle
    ));
    assert_eq!(ow.state.current_map, MapId::PalletTown);
    assert_eq!(
        (ow.state.player.x, ow.state.player.y),
        (5, 6),
        "Pallet Town fly point"
    );
}

/// The Oak's-Lab Rival1 loss is the original's no-blackout special case.
#[test]
fn oaks_lab_rival1_loss_skips_blackout() {
    let player = vec![mon(Species::Charmander, 5)];
    let enemy = vec![mon(Species::Squirtle, 5)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Rival1));
    battle.map_id = MapId::OaksLab as u8;
    battle.settlement = Some(settlement(BattleOutcome::Loss, 0, 500));
    let mut save = SaveData::new();
    save.game_data.player_money = 1000;
    let mut ow = overworld();

    let outcome = settle_battle_into_save(&mut battle, &mut save, &mut ow).outcome;
    assert_eq!(outcome, Some("lose"));
    assert_eq!(save.game_data.player_money, 1000, "no money penalty at Oak's Lab vs Rival1");
    assert!(!ow.heal_requested, "no blackout heal");
    assert!(ow.pending_warp.is_none(), "no blackout warp");
}

/// A caught wild mon (party not full) joins the party and enters the Pokédex.
#[test]
fn capture_adds_to_party_and_pokedex() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Pidgey, 5)];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.settlement = Some(settlement(BattleOutcome::Captured, 0, 0));
    battle.captured_mon = Some(mon(Species::Pidgey, 5));
    let mut save = SaveData::new();
    save.party = Party::from_pokemon(vec![mon(Species::Charmander, 10)]).unwrap();
    let mut ow = overworld();

    let outcome = settle_battle_into_save(&mut battle, &mut save, &mut ow).outcome;
    assert_eq!(outcome, Some("caught"));
    assert_eq!(save.party.count(), 2, "caught mon joined the party");
    assert!(save.game_data.pokedex.is_owned(Species::Pidgey), "Pokédex owned set");
    assert_eq!(ow.party_count, 2, "overworld party_count synced");
}

/// A catch with a full party goes to the current PC box. The BATTLE party (which the
/// writeback persists as the save party) must be full — it is the source of truth.
#[test]
fn capture_full_party_goes_to_box() {
    let player: Vec<_> = (0..6).map(|_| mon(Species::Rattata, 5)).collect();
    let enemy = vec![mon(Species::Pidgey, 5)];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.settlement = Some(settlement(BattleOutcome::Captured, 0, 0));
    battle.captured_mon = Some(mon(Species::Pidgey, 5));
    let mut save = SaveData::new();
    let mut ow = overworld();

    settle_battle_into_save(&mut battle, &mut save, &mut ow);
    assert_eq!(save.party.count(), 6, "party stays full");
    assert_eq!(save.current_box.count(), 1, "caught mon deposited to the PC box");
    assert!(save.game_data.pokedex.is_owned(Species::Pidgey));
}

/// The mutated battle party (levels/HP/…) is written back to the save.
#[test]
fn party_mutations_persist() {
    let player = vec![mon(Species::Charmander, 5)];
    let enemy = vec![mon(Species::Rattata, 3)];
    let mut battle = BattleScreen::from_parties(false, &player, &enemy, Some(TrainerClass::Youngster));
    battle.settlement = Some(settlement(BattleOutcome::Win, 100, 0));
    // Simulate an in-battle level-up on the clone.
    battle.battle_state.as_mut().unwrap().player.party[0].level = 12;
    let mut save = SaveData::new();
    save.party = Party::from_pokemon(vec![mon(Species::Charmander, 5)]).unwrap();
    let mut ow = overworld();

    settle_battle_into_save(&mut battle, &mut save, &mut ow);
    assert_eq!(save.party.leader_level(), 12, "the battle level-up persisted to the save");
    assert_eq!(ow.party_lead_level, 12, "overworld lead level synced");
}

/// The in-battle bag is written back (NOT wiped) — the regression the pre-battle
/// `player_bag` copy-in guards against.
#[test]
fn bag_is_written_back_not_wiped() {
    let player = vec![mon(Species::Charmander, 10)];
    let enemy = vec![mon(Species::Rattata, 8)];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    battle.settlement = Some(settlement(BattleOutcome::Escaped, 0, 0));
    // The battle carried a bag with 3 Potions (as the frontend's pre-battle copy-in does).
    battle.player_bag.add_item(ItemId::Potion, 3).unwrap();
    let mut save = SaveData::new();
    let mut ow = overworld();

    let outcome = settle_battle_into_save(&mut battle, &mut save, &mut ow).outcome;
    assert_eq!(outcome, Some("fled"));
    assert!(save.game_data.bag.has_item(ItemId::Potion, 3), "battle bag written back, not wiped");
}

/// A battle win queues detected level-up evolutions for the post-battle
/// cutscene instead of applying them silently: the writeback returns them in
/// `pending_evolutions` and the save party keeps the pre-evolution species.
#[test]
fn win_queues_pending_evolutions() {
    let player = vec![mon(Species::Bulbasaur, 16)];
    let enemy = vec![mon(Species::Rattata, 3)];
    let mut battle = BattleScreen::from_parties(true, &player, &enemy, None);
    let mut s = settlement(BattleOutcome::Win, 0, 0);
    s.evolutions.push(crate::battle::settlement::EvolutionEvent {
        party_index: 0,
        old_species: Species::Bulbasaur,
        new_species: Species::Ivysaur,
    });
    battle.settlement = Some(s);
    let mut save = SaveData::new();
    save.party = Party::from_pokemon(vec![mon(Species::Bulbasaur, 15)]).unwrap();
    let mut ow = overworld();

    let wb = settle_battle_into_save(&mut battle, &mut save, &mut ow);
    assert_eq!(wb.outcome, Some("win"));
    assert_eq!(wb.pending_evolutions.len(), 1);
    assert_eq!(wb.pending_evolutions[0].new_species, Species::Ivysaur);
    // The party is written back UN-evolved; the cutscene applies the swap
    // (or not, on a B-cancel).
    assert_eq!(save.party.get(0).unwrap().species, Species::Bulbasaur);
    assert!(!save.game_data.pokedex.is_owned(Species::Ivysaur));
}
