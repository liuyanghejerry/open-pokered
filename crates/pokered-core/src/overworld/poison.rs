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

use crate::battle::state::StatusCondition;
use crate::overworld::screen::{OverworldScreen, OverworldAudioRequest};
use crate::save::SaveData;

/// Run one poison-damage tick against the party. Called by the frontend on
/// `OverworldGameDataRequest::PoisonStep`.
///
/// Mirrors the asm ordering: damage (+ per-mon faint text) first, then the
/// poisoned jingle, then the blackout check.
pub fn apply_out_of_battle_poison_damage(save: &mut SaveData, overworld: &mut OverworldScreen) {
    // Damage pass: one HP per poisoned, still-alive mon; collect faint texts.
    let mut faint_texts: Vec<String> = Vec::new();
    for mon in save.party.iter_mut() {
        if mon.status != StatusCondition::Poison || mon.hp == 0 {
            continue;
        }
        mon.hp -= 1;
        if mon.hp == 0 {
            let mut buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
            faint_texts.push(format!("{} fainted!", mon.display_name(&mut buf)));
        }
    }

    if !faint_texts.is_empty() {
        // The original prints one TEXT_MON_FAINTED box per fainted mon; the
        // port folds them into a single dialogue (multi-faint in one tick is
        // rare) so the steps keep flowing afterwards.
        overworld.pending_dialogue = Some(crate::overworld::screen::BedroomDialogue::from_message(
            &faint_texts.join("\n"),
        ));
    }

    // Any party member still poisoned? → the white→dark-gray palette blink and
    // SFX_POISONED. The 4-frame palette blink is a renderer nicety and is not
    // reproduced; the jingle is.
    let any_poisoned = save
        .party
        .iter()
        .any(|m| m.status == StatusCondition::Poison && m.hp > 0);
    if any_poisoned {
        overworld
            .audio_requests
            .push(OverworldAudioRequest::PlaySound {
                sound_id: "SFX_POISONED".to_string(),
            });
    }

    // AnyPartyAlive → blackout when the whole party is down (poisoned or not).
    let any_alive = save.party.iter().any(|m| m.hp > 0);
    if !any_alive {
        field_blackout(save, overworld);
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
    let blackout_map =
        crate::data::maps::MapId::from_u8(save.game_data.last_blackout_map)
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

    /// Any still-poisoned living mon → the SFX_POISONED jingle plays
    /// (poison.asm `.skipPoisonEffectAndSound` inverse).
    #[test]
    fn poison_tick_plays_jingle_while_poison_remains() {
        let mut save = save_with(vec![poisoned(mon(Species::Rattata, 10))]);
        let mut ow = OverworldScreen::new(MapId::Route1, None, PokemonRedData);
        apply_out_of_battle_poison_damage(&mut save, &mut ow);
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

        assert!(ow.heal_requested, "full heal queued");
        let warp = ow.pending_warp.expect("blackout warp queued");
        assert_eq!(warp.dest_map, MapId::CeruleanCity);
        assert!(!warp.arrival_spin, "blackout arrivals skip EnterMapAnim");
        assert!(matches!(ow.warp_fade_state, WarpFadeState::FadingOut { .. }));
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
        assert_eq!(save.game_data.player_money, 3001, "no penalty with a survivor");
    }
}
