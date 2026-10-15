//! Post-battle settlement → save writeback, shared by every frontend.
//!
//! The battle runs on a CLONE of the player party (`BattleScreen::from_parties`), so
//! when a battle ends the frontend must fold the results back into the save: money +
//! trainer-defeated + blackout (Loss), the mutated party (EXP / level-ups / learned
//! moves / evolutions / HP / status / PP), a caught wild mon (→ party or PC box) + its
//! Pokédex entry, the in-battle bag, and the overworld's derived party/encounter state.
//!
//! Extracted from the native app so the native + TUI frontends share ONE fidelity-
//! sensitive implementation instead of drifting copy-paste. Frontends keep only their
//! own concerns (audio, the `screen == Battle` guard) around the call.

use crate::alloc_prelude::*;
use crate::battle::settlement::EvolutionEvent;
use crate::battle::BattleScreen;
use crate::overworld::OverworldScreen;
use crate::save::SaveData;
use dotzuki_engine::overworld::types::TransportMode;

/// The insertion primitive's actual slot, retained until AskName returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedLocation {
    Party(usize),
    CurrentBox(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturedMonReceipt {
    pub location: CapturedLocation,
    pub species: pokered_data::species::Species,
    /// Tested before the caught species' owned flag is set.
    pub new_pokedex_entry: bool,
}

/// What [`settle_battle_into_save`] produced.
pub struct SettleWriteback {
    /// The outcome string (`"win"` / `"lose"` / `"caught"` / `"fled"` =
    /// Poké-Doll escape / `"ran"` = menu run / `"draw"`) for resuming a
    /// script suspended on `await game.startBattle(...)`, or `None` if there
    /// was no settlement.
    pub outcome: Option<&'static str>,
    /// Level-up evolutions detected at battle end
    /// (`BattleSettlement::evolutions`) — NOT yet applied. The frontend plays
    /// the evolution cutscene (`crate::evolution_screen`) in the overworld,
    /// then applies each confirmed one with
    /// `pokemon::evolution::finalize_evolution` (a B-cancelled evolution
    /// applies nothing and retries on the mon's next level-up, matching
    /// `wCanEvolveFlags` semantics).
    pub pending_evolutions: Vec<EvolutionEvent>,
    pub captured: Option<CapturedMonReceipt>,
}

/// Fold a finished battle's results into `save` + `overworld`.
///
/// The caller is responsible for: guarding on "we are leaving a battle", playing any
/// post-battle music, and calling [`OverworldScreen::resume_script_after_battle`] with
/// the returned outcome (kept out of here so frontends control audio/resume ordering).
pub fn settle_battle_into_save(
    battle: &mut BattleScreen,
    save: &mut SaveData,
    overworld: &mut OverworldScreen,
) -> SettleWriteback {
    use crate::battle::settlement::BattleOutcome;

    let mut battle_outcome: Option<&'static str> = None;
    let mut pending_evolutions: Vec<EvolutionEvent> = Vec::new();
    let mut captured_receipt = None;
    if let Some(ref settlement) = battle.settlement {
        battle_outcome = Some(match settlement.outcome {
            BattleOutcome::Win => "win",
            BattleOutcome::Loss => "lose",
            BattleOutcome::Captured => "caught",
            // Escape kinds are distinct in the original: a Poké DOLL escape
            // never writes wBattleResult (reads as a win to scripts), while a
            // menu run writes $2 (core.asm:1600-1602). "fled" = Doll skip,
            // "ran" = menu run.
            BattleOutcome::Escaped if battle.escaped_via_poke_doll => "fled",
            BattleOutcome::Escaped => "ran",
            _ => "draw",
        });
        pending_evolutions = settlement.evolutions.clone();
        match settlement.outcome {
            BattleOutcome::Win => {
                let gained = settlement.money_gained;
                save.game_data.player_money = (save.game_data.player_money as u32)
                    .saturating_add(gained)
                    .min(999_999);
                if gained > 0 {
                    log::info!("Player won ${}", gained);
                }
                // Mark the trainer NPC as defeated.
                if let Some(npc_index) = battle.trainer_npc_index.take() {
                    // PrintEndBattleText sets ordinary trainers to STAY/NONE:
                    // no further wandering, but unrestricted random facing.
                    // Rivals and Tower7F Rockets retain their departure scripts.
                    use pokered_data::trainer_data::TrainerClass;
                    let stays = battle.map_id != crate::data::maps::MapId::PokemonTower7F as u8
                        && battle.trainer_class.is_some_and(|class| !matches!(class,
                            TrainerClass::Rival1 | TrainerClass::Rival2 | TrainerClass::Rival3));
                    if stays {
                        if let Some(npc) = overworld.npc_states.iter_mut()
                            .find(|npc| npc.npc_index == npc_index) {
                            npc.movement_type = crate::overworld::NpcMovementType::Stationary;
                            npc.range = 0xFF;
                        }
                    }
                    crate::overworld::npc_interaction::mark_trainer_defeated(
                        &mut overworld.npc_states,
                        npc_index,
                    );
                    // EndTrainerBattle (home/trainers.asm:169-186) also sets
                    // the trainer's wEventFlags bit (TrainerFlagAction
                    // FLAG_SET) so CheckForEngagingTrainers skips them after
                    // re-entering the map; our LOS check tests the same
                    // flag. The k-th header belongs to the k-th trainer NPC
                    // in object order.
                    if let Some(map) = crate::data::maps::MapId::from_u8(battle.map_id) {
                        let headers = pokered_data::trainer_headers::get_trainer_headers(map);
                        if let Some(k) = crate::overworld::trainer_engine::trainer_ordinal(
                            &overworld.npc_pokemon_data,
                            npc_index,
                        ) {
                            if let Some(header) = headers.get(k) {
                                overworld.set_event_flag_live(header.event_flag);
                            }
                        }
                    }
                    // The original's per-frame map script re-applies door/exit
                    // blocks the moment the trainer flag flips (EndTrainerBattle
                    // hands back to the map script — BrunosRoom.asm:11-26,
                    // AgathasRoom.asm:11-26). Re-run the port's `@load` (idempotent
                    // by design) so post-battle block writes land immediately
                    // instead of on the next map re-entry.
                    overworld.rerun_map_on_load_script();
                }
            }
            BattleOutcome::Loss => {
                // Oak's Lab Rival1: no blackout in the original (special case).
                let is_oaks_lab_rival = battle.map_id == crate::data::maps::MapId::OaksLab as u8
                    && matches!(
                        battle.trainer_class,
                        Some(pokered_data::trainer_data::TrainerClass::Rival1)
                    );
                // Link defeats return to the Cable Club room after HealParty;
                // they never queue the ordinary city's blackout warp.
                if !is_oaks_lab_rival && !battle.link_mode {
                    let lost = settlement.money_lost;
                    save.game_data.player_money = save.game_data.player_money.saturating_sub(lost);
                    if lost > 0 {
                        log::info!("Player lost ${} on blackout", lost);
                    }
                    // Heal the party and warp back — the original sets
                    // BIT_ESCAPE_WARP and warps to wLastBlackoutMap's FLY
                    // point (special_warps.asm:66-81): outside the Pokémon
                    // Center of the last city healed at. wLastBlackoutMap
                    // defaults to 0 = PALLET_TOWN (wram init), landing in
                    // front of the player's house before any heal.
                    overworld.heal_requested = true;
                    let blackout_map = crate::data::maps::MapId::from_u8(
                        save.game_data.last_blackout_map,
                    )
                    .unwrap_or(crate::data::maps::MapId::PalletTown);
                    let dest = crate::overworld::hm_effects::fly_destination_for_map(blackout_map)
                        .or_else(|| {
                            crate::overworld::hm_effects::fly_destination_for_map(
                                crate::data::maps::MapId::PalletTown,
                            )
                        })
                        .expect("Pallet Town always has a fly point");
                    overworld.pending_warp = Some(crate::overworld::PendingWarp {
                        dest_map: dest.map,
                        dest_x: dest.x,
                        dest_y: dest.y,
                        save_last_map: false,
                        // Blackout (black_out.asm:42-43) RESETS BIT_FLY_WARP
                        // (only BIT_ESCAPE_WARP is set) → the arrival fades in
                        // without EnterMapAnim.
                        arrival_spin: false,
                    });
                    // Start the fade-out in the same breath: the update loop
                    // only commits a queued warp from the BlackScreen fade
                    // phase, and every other pending_warp producer pairs the
                    // queue with its fade. Without this the blackout warp
                    // hangs forever with the fade Idle — and warp triggers
                    // bail while a warp is pending, so the player is
                    // soft-locked on the battle map.
                    overworld.warp_fade_to_white = true;
                    overworld.warp_fade_state = crate::overworld::WarpFadeState::FadingOut {
                        frames_remaining: crate::overworld::WARP_FADE_OUT_WHITE_FRAMES,
                    };
                    // DisplayPlayerBlackedOutText (home/text_script.asm:200-202)
                    // and the loss path (engine/battle/core.asm:1160-1162) reset
                    // BIT_ALWAYS_ON_BIKE — a blackout on the Cycling Road
                    // releases the forced bike and restores walking.
                    overworld.forced_bike.clear();
                    overworld.state.player.transport = TransportMode::Walking;
                } else if is_oaks_lab_rival {
                    log::info!("Oak's Lab Rival1 loss: skipping blackout (original behavior)");
                }
            }
            _ => {}
        }
    }
    battle.settlement = None;

    // Persist items consumed in battle (balls thrown, potions used) back into the bag.
    save.game_data.bag = battle.player_bag.clone();

    // Persist the mutated battle party (EXP / level-ups / learned moves / HP /
    // status / PP) — the battle ran on a clone of the party. Evolutions are
    // NOT in here: they are returned in `pending_evolutions` and applied only
    // after the evolution cutscene confirms them.
    if let Some(ref bs) = battle.battle_state {
        if !bs.player.party.is_empty() {
            if let Ok(p) = crate::pokemon::party::Party::from_pokemon(bs.player.persistent_party()) {
                save.party = p;
            }
        }
    }
    // A caught wild Pokémon joins the party (or the current PC box if the party is full)
    // and enters the Pokédex.
    if let Some(mut caught) = battle.captured_mon.take() {
        // _AddPartyMon / SendNewMonToBox stamp the player's raw ownership
        // fields in both ordinary and Safari captures, preserving all other
        // caught data (including HP, status, DVs and current PP).
        caught.ot_id = save.game_data.player_id;
        caught.ot_name.fill(0x50);
        let name_len = caught.ot_name.len().min(save.player_name.len());
        caught.ot_name[..name_len].copy_from_slice(&save.player_name[..name_len]);
        let species = caught.species;
        let new_pokedex_entry = !save.game_data.pokedex.is_owned(species);
        let location = if save.party.count() < 6 {
            save.party.add(caught).ok().map(CapturedLocation::Party)
        } else {
            let location = save.current_box.insert_new_mon(caught).ok()
                .map(CapturedLocation::CurrentBox);
            save.sync_current_box_to_storage();
            location
        };
        captured_receipt = location.map(|location| CapturedMonReceipt {
            location, species, new_pokedex_entry,
        });
        save.game_data.pokedex.set_seen(species);
        save.game_data.pokedex.set_owned(species);
    }
    // Colosseum heals only after returning from InitOpponent, not on entry.
    if battle.link_mode { save.party.heal_all(); }
    overworld.box_count = save.current_box.count() as u8;
    overworld.party_count = save.party.count() as u8;
    overworld.party_lead_level = save.party.leader_level();
    overworld.set_post_battle_encounter_cooldown();

    SettleWriteback {
        outcome: battle_outcome,
        pending_evolutions,
        captured: captured_receipt,
    }
}

#[cfg(test)]
mod trainer_victory_tests {
    use super::*;
    use crate::battle::settlement::{BattleOutcome, BattleSettlement};
    use crate::overworld::{Direction, NpcMovementType};
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId, trainer_data::TrainerClass};

    #[test]
    fn trainer_victory_sets_stay_none_with_original_exceptions() {
        for (map, class, outcome, expected) in [
            (MapId::PokemonMansion2F, TrainerClass::Burglar, BattleOutcome::Win, true),
            (MapId::PokemonMansion3F, TrainerClass::Burglar, BattleOutcome::Win, true),
            (MapId::Route3, TrainerClass::Youngster, BattleOutcome::Win, true),
            (MapId::OaksLab, TrainerClass::Rival1, BattleOutcome::Win, false),
            (MapId::Route22, TrainerClass::Rival2, BattleOutcome::Win, false),
            (MapId::ChampionsRoom, TrainerClass::Rival3, BattleOutcome::Win, false),
            (MapId::PokemonTower7F, TrainerClass::Rocket, BattleOutcome::Win, false),
            (MapId::PokemonMansion2F, TrainerClass::Burglar, BattleOutcome::Loss, false),
        ] {
            let mut ow = OverworldScreen::new(map, None, PokemonRedData);
            let npc = &mut ow.npc_states[0];
            npc.movement_type = NpcMovementType::Wander;
            npc.range = 0xFC;
            npc.facing = Direction::Left;
            npc.walk_counter = 8;
            npc.delay_counter = 7;
            let index = npc.npc_index;
            let mut battle = BattleScreen::new(true);
            battle.map_id = map as u8;
            battle.trainer_class = Some(class);
            battle.trainer_npc_index = Some(index);
            battle.settlement = Some(BattleSettlement {
                outcome, money_gained: 0, money_lost: 0, payday_bonus: 0,
                exp_entries: vec![], level_ups: vec![], evolutions: vec![],
            });
            settle_battle_into_save(&mut battle, &mut SaveData::new(), &mut ow);
            let npc = &ow.npc_states[0];
            assert_eq!(npc.movement_type, if expected { NpcMovementType::Stationary }
                else { NpcMovementType::Wander }, "{map:?}/{class:?}/{outcome:?}");
            assert_eq!(npc.range, if expected { 0xFF } else { 0xFC });
            // The source changes only its two movement bytes, preserving
            // pending pixels, facing and delay from the interrupted walk.
            assert_eq!((npc.walk_counter, npc.delay_counter, npc.facing),
                (8, 7, Direction::Left));
        }
    }
}
