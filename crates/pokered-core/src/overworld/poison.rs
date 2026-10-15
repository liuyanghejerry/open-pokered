//! Out-of-battle poison damage — a faithful port of
//! `engine/events/poison.asm` (`ApplyOutOfBattlePoisonDamage`).
//!
//! Every fourth overworld step, each poisoned party member with HP left loses
//! 1 HP; a mon whose HP reaches 0 faints with its own text box. If any party
//! member is still poisoned, the poison jingle plays. If NO party member is
//! alive afterwards, the player blacks out — full heal and a warp to the fly
//! point of `wLastBlackoutMap`, exactly like the battle-loss blackout.
//!
//! The party lives in the save layer, so the overworld only *requests* the
//! tick (`OverworldGameDataRequest::PoisonStep` every 4th step); the frontend
//! consumer calls [`apply_out_of_battle_poison_damage`].

use crate::alloc_prelude::*;
use crate::battle::state::StatusCondition;
use crate::overworld::screen::{OverworldAudioRequest, OverworldScreen};
use crate::save::SaveData;

/// The suspended source loop stores only its next slot and presentation phase.
/// Pokemon remain owned by `SaveData.party` throughout a poison tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FieldPoisonFlow {
    Damage { next_index: u8 },
    Faint { next_index: u8 },
    Flash { frames_remaining: u8 },
    Blackout,
}

/// The door target is already in pending_warp; retain only its sound while
/// the source ApplyOutOfBattlePoisonDamage caller precedes CheckWarps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PoisonStepWarpSound {
    GoInside,
    GoOutside,
}

/// Resume PlayMapChangeSound/fade only after the party's poison caller returns.
pub fn resume_step_warp_after_poison(overworld: &mut OverworldScreen) {
    if overworld.safari_eject_pending.is_some() {
        if overworld.poison_step_warp_sound.take().is_some() {
            overworld.pending_warp = None;
        }
        return;
    }
    if overworld.poison_flow.is_some() {
        return;
    }
    if let Some(sound) = overworld.poison_step_warp_sound.take() {
        if overworld.pending_warp.is_some() {
            // Queue after SFX_POISONED, matching PlaySound before the door's
            // PlayMapChangeSound. One-frame sfx_event cannot retain this call.
            overworld.audio_requests.push(OverworldAudioRequest::PlaySound {
                sound_id: match sound {
                    PoisonStepWarpSound::GoInside => "SFX_GO_INSIDE",
                    PoisonStepWarpSound::GoOutside => "SFX_GO_OUTSIDE",
                }.to_string(),
            });
            overworld.warp_fade_state = super::WarpFadeState::FadingOut {
                frames_remaining: super::WARP_FADE_OUT_FRAMES,
            };
        }
    }
}

impl FieldPoisonFlow {
    pub fn bg_flash_active(self) -> bool {
        matches!(
            self,
            Self::Flash {
                frames_remaining: 1..=4
            }
        )
    }
}

/// Enter `ApplyOutOfBattlePoisonDamage` after a completed fourth field step.
/// The first fainted Pokemon suspends the loop on its own DisplayTextID call.
pub fn apply_out_of_battle_poison_damage(save: &mut SaveData, overworld: &mut OverworldScreen) {
    if save.party.is_empty() || overworld.poison_flow.is_some() {
        return;
    }
    overworld.poison_flow = Some(FieldPoisonFlow::Damage { next_index: 0 });
    resume_out_of_battle_poison_damage(save, overworld);
}

/// Resume once per overworld update, before entering a newly requested tick.
/// Existing dialogue owns the physical A/B edges and HoldTextDisplayOpen.
pub fn resume_out_of_battle_poison_damage(save: &mut SaveData, overworld: &mut OverworldScreen) {
    let Some(flow) = overworld.poison_flow else {
        return;
    };
    let next_index = match flow {
        FieldPoisonFlow::Damage { next_index } => next_index,
        FieldPoisonFlow::Faint { next_index } => {
            if overworld.pending_dialogue.is_some() {
                return;
            }
            next_index
        }
        FieldPoisonFlow::Flash { frames_remaining } => {
            if frames_remaining > 1 {
                overworld.poison_flow = Some(FieldPoisonFlow::Flash {
                    frames_remaining: frames_remaining - 1,
                });
                return;
            }
            // ChangeBGPalColor0_4Frames restores BGP before PlaySound.
            overworld
                .audio_requests
                .push(OverworldAudioRequest::PlaySound {
                    sound_id: "SFX_POISONED".to_string(),
                });
            finish_poison_tick(save, overworld);
            return;
        }
        FieldPoisonFlow::Blackout => {
            if overworld.pending_dialogue.is_some() {
                return;
            }
            overworld.poison_flow = None;
            field_blackout(save, overworld);
            return;
        }
    };

    for index in usize::from(next_index)..save.party.count() {
        let mon = save.party.get_mut(index).expect("party slot exists");
        if mon.status != StatusCondition::Poison || mon.hp == 0 {
            continue;
        }
        mon.hp -= 1;
        if mon.hp == 0 {
            mon.status = StatusCondition::None;
            let mut buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
            let text = format!("{} fainted!", mon.display_name(&mut buf));
            show_poison_dialogue(overworld, &text);
            overworld.poison_flow = Some(FieldPoisonFlow::Faint {
                next_index: index as u8 + 1,
            });
            return;
        }
    }

    // The source counts the PSN status bit, including an already-fainted
    // poisoned slot. It does not add an HP guard to this separate pass.
    if save
        .party
        .iter()
        .any(|mon| mon.status == StatusCondition::Poison)
    {
        overworld.poison_flow = Some(FieldPoisonFlow::Flash {
            frames_remaining: 4,
        });
    } else {
        finish_poison_tick(save, overworld);
    }
}

fn show_poison_dialogue(overworld: &mut OverworldScreen, message: &str) {
    let text = overworld.localize_message(message);
    let mut dialogue = crate::overworld::screen::BedroomDialogue::from_message(&text);
    dialogue.begin_field_display(true);
    overworld.pending_dialogue = Some(dialogue);
}

fn finish_poison_tick(save: &mut SaveData, overworld: &mut OverworldScreen) {
    if save.party.iter().any(|mon| mon.hp > 0) {
        overworld.poison_flow = None;
    } else {
        // The source jumps to HandleBlackOut before calling NewBattle. The
        // native step has already rolled an encounter; it must not survive
        // this blackout and start after the party is healed or warped.
        overworld.pending_wild_encounter = None;
        // HandleBlackOut replaces CheckWarps: the original door and its sound
        // must never resume after the player is healed at the blackout point.
        if overworld.poison_step_warp_sound.take().is_some() {
            overworld.pending_warp = None;
        }
        // TEXT_BLACKED_OUT is a separate blocking DisplayTextID call.
        let message = format!("{}\nblacked out!", overworld.player_name);
        show_poison_dialogue(overworld, &message);
        overworld.poison_flow = Some(FieldPoisonFlow::Blackout);
    }
}

/// The blackout shared with the battle-loss path: full heal, warp to the fly
/// point of `wLastBlackoutMap` (Pallet Town before any heal), fade to white,
/// release the Cycling-Road forced bike. Mirrors `BattleOutcome::Loss` in
/// `battle/settlement/writeback.rs`: HandleBlackOut calls
/// ResetStatusAndHalveMoneyOnBlackout for field and battle blackouts alike.
fn field_blackout(save: &mut SaveData, overworld: &mut OverworldScreen) {
    save.game_data.player_money /= 2;
    overworld.heal_requested = true;
    let blackout_map = crate::data::maps::MapId::from_u8(save.game_data.last_blackout_map)
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
        // black_out.asm:42-43 resets BIT_FLY_WARP — no EnterMapAnim on arrival.
        arrival_spin: false,
    });
    overworld.warp_fade_to_white = true;
    overworld.warp_fade_state = crate::overworld::WarpFadeState::FadingOut {
        frames_remaining: crate::overworld::WARP_FADE_OUT_WHITE_FRAMES,
    };
    overworld.forced_bike.clear();
    overworld.state.player.transport = dotzuki_engine::overworld::types::TransportMode::Walking;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::Pokemon;
    use crate::overworld::screen::WarpFadeState;
    use pokered_data::impl_traits::PokemonRedData;
    use pokered_data::maps::MapId;
    use pokered_data::species::Species;

    fn mon(species: Species, level: u8) -> Pokemon {
        crate::pokemon::stats::create_pokemon(species, level, [0x9A, 0x78]).expect("valid")
    }

    fn poisoned(mut m: Pokemon) -> Pokemon {
        m.status = StatusCondition::Poison;
        m
    }

    fn save_with(mons: Vec<Pokemon>) -> SaveData {
        let mut save = SaveData::new();
        for m in mons {
            let _ = save.party.add(m);
        }
        save
    }

    fn finish_tick(save: &mut SaveData, ow: &mut OverworldScreen) {
        for _ in 0..20 {
            if ow.poison_flow.is_none() {
                return;
            }
            if matches!(
                ow.poison_flow,
                Some(FieldPoisonFlow::Faint { .. } | FieldPoisonFlow::Blackout)
            ) {
                ow.pending_dialogue = None; // Explicit source DisplayTextID return.
            }
            resume_out_of_battle_poison_damage(save, ow);
        }
        panic!("poison caller did not complete");
    }

    #[test]
    fn empty_party_before_starter_never_blacks_out() {
        let mut save = SaveData::new();
        save.game_data.player_money = 3000;
        let mut ow = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        assert!(ow.pending_warp.is_none());
        assert!(!ow.heal_requested);
        assert!(ow.pending_dialogue.is_none());
        assert_eq!(save.game_data.player_money, 3000);
    }

    /// Every poisoned, alive mon loses exactly 1 HP per tick (poison.asm
    /// `.applyDamageLoop`); healthy and fainted mons are untouched.
    #[test]
    fn poison_tick_damages_only_poisoned_alive_mons() {
        let mut healthy = mon(Species::Pidgey, 10);
        healthy.status = StatusCondition::Burn;
        let save_hp = healthy.hp;
        let mut save = save_with(vec![poisoned(mon(Species::Rattata, 10)), healthy]);
        save.party.get_mut(0).unwrap().hp = 20;
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);

        assert_eq!(save.party.get(0).unwrap().hp, 19, "poisoned mon takes 1");
        assert_eq!(
            save.party.get(1).unwrap().hp,
            save_hp,
            "burned mon untouched"
        );
        assert!(ow.pending_dialogue.is_none(), "nobody fainted");
    }

    /// A poisoned mon whose HP reaches 0 faints with its text box (TEXT_MON_
    /// FAINTED) but stays in the party at 0 HP.
    #[test]
    fn poison_tick_faints_at_one_hp_and_shows_text() {
        let mut m = poisoned(mon(Species::Rattata, 10));
        m.hp = 1;
        let mut save = save_with(vec![m]);
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);

        assert_eq!(save.party.get(0).unwrap().hp, 0);
        assert!(ow.pending_dialogue.is_some(), "shows '<MON> fainted!'");
    }

    #[test]
    fn field_poison_clears_status_only_when_this_tick_causes_fainting() {
        let mut newly_fainted = poisoned(mon(Species::Rattata, 10));
        newly_fainted.hp = 1;
        let mut already_fainted = poisoned(mon(Species::Pidgey, 10));
        already_fainted.hp = 0;
        let mut surviving_poison = poisoned(mon(Species::Bulbasaur, 10));
        surviving_poison.hp = 3;
        let healthy = mon(Species::Charmander, 10);
        let mut save = save_with(vec![
            newly_fainted,
            already_fainted,
            surviving_poison,
            healthy,
        ]);
        let before = save.party.to_vec();
        save.game_data.player_money = 3001;
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        finish_tick(&mut save, &mut ow);

        let mut expected = before.clone();
        expected[0].hp = 0;
        expected[0].status = StatusCondition::None;
        expected[2].hp = 2;
        assert_eq!(
            save.party.to_vec(),
            expected,
            "PP and all other party fields stay intact"
        );
        assert_eq!(save.game_data.player_money, 3001);
        assert!(!ow.heal_requested);
        assert!(ow.pending_warp.is_none());
        assert!(
            ow.pending_dialogue.is_none(),
            "faint ACK completed before the flash"
        );
        assert!(ow.audio_requests.iter().any(|request| matches!(request,
            OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_POISONED")));
    }

    /// Any still-poisoned living mon → the SFX_POISONED jingle plays
    /// (poison.asm `.skipPoisonEffectAndSound` inverse).
    #[test]
    fn poison_tick_plays_jingle_while_poison_remains() {
        let mut save = save_with(vec![poisoned(mon(Species::Rattata, 10))]);
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        finish_tick(&mut save, &mut ow);
        assert!(ow.audio_requests.iter().any(|r| matches!(
            r,
            OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_POISONED"
        )));
    }

    /// No poison left → no jingle (poison.asm `.skipPoisonEffectAndSound`).
    #[test]
    fn no_poison_no_jingle() {
        let mut save = save_with(vec![mon(Species::Rattata, 10)]);
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        assert!(ow.audio_requests.is_empty());
        assert!(ow.pending_warp.is_none(), "no blackout");
    }

    /// A party wiped by the tick blacks out: full heal queued, warp to the
    /// last-healed map's fly point with a white fade — the battle-loss flow
    /// (black_out.asm), including the halved money.
    #[test]
    fn full_party_faint_blacks_out_to_last_center() {
        let mut a = poisoned(mon(Species::Rattata, 10));
        a.hp = 1;
        let mut b = poisoned(mon(Species::Pidgey, 10));
        b.hp = 1;
        let mut save = save_with(vec![a, b]);
        save.game_data.last_blackout_map = MapId::CeruleanCity as u8;
        save.game_data.player_money = 3000;
        let mut ow = OverworldScreen::new(MapId::Route10, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        finish_tick(&mut save, &mut ow);

        assert!(ow.heal_requested, "full heal queued");
        let warp = ow.pending_warp.expect("blackout warp queued");
        assert_eq!(warp.dest_map, MapId::CeruleanCity);
        assert!(!warp.arrival_spin, "blackout arrivals skip EnterMapAnim");
        assert!(matches!(
            ow.warp_fade_state,
            WarpFadeState::FadingOut { .. }
        ));
        assert_eq!(
            save.game_data.player_money, 1500,
            "field blackout halves money like a battle loss"
        );
    }

    #[test]
    fn field_blackout_halves_money_rounding_down() {
        for money in [0, 1, 3, 3001, 999999] {
            let mut m = poisoned(mon(Species::Rattata, 10));
            m.hp = 1;
            let mut save = save_with(vec![m]);
            save.game_data.player_money = money;
            let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
            apply_out_of_battle_poison_damage(&mut save, &mut ow);
            finish_tick(&mut save, &mut ow);
            assert_eq!(save.game_data.player_money, money / 2);
        }
    }

    /// Fainted-but-not-poisoned party members do not trigger a blackout on
    /// their own; the party must be entirely down.
    #[test]
    fn partial_survivors_do_not_black_out() {
        let mut a = poisoned(mon(Species::Rattata, 10));
        a.hp = 1;
        let b = mon(Species::Pidgey, 10);
        let mut save = save_with(vec![a, b]);
        save.game_data.player_money = 3001;
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);

        assert!(save.party.get(0).unwrap().hp == 0);
        assert!(ow.pending_warp.is_none(), "one survivor → no blackout");
        assert_eq!(
            save.game_data.player_money, 3001,
            "no penalty with a survivor"
        );
    }
    #[test]
    fn each_faint_suspends_before_the_next_party_slot_and_blackout() {
        let mut first = poisoned(mon(Species::Rattata, 12));
        first.hp = 1;
        let mut second = first.clone();
        second.set_nickname("SECOND");
        let mut save = save_with(vec![first, second]);
        save.game_data.player_money = 3001;
        let mut ow = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(save.party.get(0).unwrap().hp, 0);
        assert_eq!(
            save.party.get(1).unwrap().hp,
            1,
            "next slot is untouched until ACK"
        );
        assert!(!ow.heal_requested && ow.pending_warp.is_none());
        assert_eq!(save.game_data.player_money, 3001);
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(
            save.party.get(1).unwrap().hp,
            1,
            "waiting may not apply damage twice"
        );
        ow.pending_dialogue = None;
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(save.party.get(1).unwrap().hp, 0);
        assert!(ow.pending_dialogue.as_ref().unwrap().pages()[0]
            .line1
            .contains("SECOND"));
        assert!(!ow.heal_requested && ow.pending_warp.is_none());
        ow.pending_dialogue = None;
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(ow.poison_flow, Some(FieldPoisonFlow::Blackout));
        assert_eq!(save.game_data.player_money, 3001);
        assert!(!ow.heal_requested && ow.pending_warp.is_none());
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(
            save.game_data.player_money, 3001,
            "blackout text has its own ACK"
        );
        ow.pending_dialogue = None;
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert!(ow.heal_requested && ow.pending_warp.is_some());
        assert_eq!(save.game_data.player_money, 1500);
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(
            save.game_data.player_money, 1500,
            "settlement executes once"
        );
    }

    #[test]
    fn mixed_faint_then_four_flash_frames_then_sound() {
        let mut first = poisoned(mon(Species::Rattata, 12));
        first.hp = 1;
        let mut second = poisoned(mon(Species::Rattata, 12));
        second.hp = 3;
        let mut save = save_with(vec![first, second]);
        let mut ow = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(save.party.get(1).unwrap().hp, 3);
        assert!(ow.audio_requests.is_empty());
        ow.pending_dialogue = None;
        resume_out_of_battle_poison_damage(&mut save, &mut ow);
        assert_eq!(save.party.get(1).unwrap().hp, 2);
        for remaining in [4, 3, 2, 1] {
            assert_eq!(
                ow.poison_flow,
                Some(FieldPoisonFlow::Flash {
                    frames_remaining: remaining
                })
            );
            assert!(ow.audio_requests.is_empty());
            resume_out_of_battle_poison_damage(&mut save, &mut ow);
        }
        assert!(ow.poison_flow.is_none());
        assert_eq!(ow.audio_requests.len(), 1);
        assert!(
            matches!(&ow.audio_requests[0], OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_POISONED")
        );
        assert!(!ow.heal_requested && ow.pending_warp.is_none());
    }

    #[test]
    fn source_counts_poison_status_even_on_an_already_fainted_member() {
        let mut first = poisoned(mon(Species::Rattata, 12));
        first.hp = 0;
        let healthy = mon(Species::Pidgey, 12);
        let mut save = save_with(vec![first, healthy]);
        let before = save.party.to_vec();
        let mut ow = OverworldScreen::new(MapId::RedsHouse2F, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
        assert!(ow.pending_dialogue.is_none());
        assert_eq!(
            ow.poison_flow,
            Some(FieldPoisonFlow::Flash {
                frames_remaining: 4
            })
        );
        finish_tick(&mut save, &mut ow);
        assert_eq!(save.party.to_vec(), before);
        assert_eq!(ow.audio_requests.len(), 1);
    }
}
