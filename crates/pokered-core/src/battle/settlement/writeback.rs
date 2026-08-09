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

use crate::battle::settlement::EvolutionEvent;
use crate::battle::BattleScreen;
use crate::overworld::OverworldScreen;
use crate::save::SaveData;
use jrpg_engine::overworld::types::TransportMode;

/// What [`settle_battle_into_save`] produced.
pub struct SettleWriteback {
    /// The outcome string (`"win"` / `"lose"` / `"caught"` / `"fled"` /
    /// `"draw"`) for resuming a script suspended on `await
    /// game.startBattle(...)`, or `None` if there was no settlement.
    pub outcome: Option<&'static str>,
    /// Level-up evolutions detected at battle end
    /// (`BattleSettlement::evolutions`) — NOT yet applied. The frontend plays
    /// the evolution cutscene (`crate::evolution_screen`) in the overworld,
    /// then applies each confirmed one with
    /// `pokemon::evolution::finalize_evolution` (a B-cancelled evolution
    /// applies nothing and retries on the mon's next level-up, matching
    /// `wCanEvolveFlags` semantics).
    pub pending_evolutions: Vec<EvolutionEvent>,
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
    if let Some(ref settlement) = battle.settlement {
        battle_outcome = Some(match settlement.outcome {
            BattleOutcome::Win => "win",
            BattleOutcome::Loss => "lose",
            BattleOutcome::Captured => "caught",
            BattleOutcome::Escaped => "fled",
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
                    crate::overworld::npc_interaction::mark_trainer_defeated(
                        &mut overworld.npc_states,
                        npc_index,
                    );
                }
            }
            BattleOutcome::Loss => {
                // Oak's Lab Rival1: no blackout in the original (special case).
                let is_oaks_lab_rival = battle.map_id == crate::data::maps::MapId::OaksLab as u8
                    && matches!(
                        battle.trainer_class,
                        Some(pokered_data::trainer_data::TrainerClass::Rival1)
                    );
                if !is_oaks_lab_rival {
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
                    // DisplayPlayerBlackedOutText (home/text_script.asm:200-202)
                    // and the loss path (engine/battle/core.asm:1160-1162) reset
                    // BIT_ALWAYS_ON_BIKE — a blackout on the Cycling Road
                    // releases the forced bike and restores walking.
                    overworld.forced_bike.clear();
                    overworld.state.player.transport = TransportMode::Walking;
                } else {
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
            if let Ok(p) = crate::pokemon::party::Party::from_pokemon(bs.player.party.clone()) {
                save.party = p;
            }
        }
    }
    // A caught wild Pokémon joins the party (or the current PC box if the party is full)
    // and enters the Pokédex.
    if let Some(caught) = battle.captured_mon.take() {
        let species = caught.species;
        if save.party.count() < 6 {
            let _ = save.party.add(caught);
        } else {
            let _ = save.current_box.deposit(caught);
        }
        save.game_data.pokedex.set_seen(species);
        save.game_data.pokedex.set_owned(species);
    }
    overworld.party_count = save.party.count() as u8;
    overworld.party_lead_level = save.party.leader_level();
    overworld.set_post_battle_encounter_cooldown();

    SettleWriteback {
        outcome: battle_outcome,
        pending_evolutions,
    }
}
