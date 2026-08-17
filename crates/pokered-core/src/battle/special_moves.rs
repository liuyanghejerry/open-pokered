//! ReadTrainer's special-move pass (engine/battle/read_trainer_party.asm
//! `.AddLoneMove` / `.AddTeamMove` / `.ChampionRival`).
//!
//! Gen-1 quirks encoded here:
//! * `wGymLeaderNo` and `wLoneAttackNo` are the SAME byte (ram/wram.asm:1264-1265
//!   union alias) — the gym script's 1-8 leader number doubles as the LoneMoves
//!   table index, so a gym leader's party is identifiable from the class alone.
//! * The special move always overwrites MOVE SLOT 3 (index 2) of the target
//!   party member (`wEnemyMon1Moves + 2` / `wEnemyMon5Moves + 2`).
//! * The champion rival's starter counter-move is keyed on `wRivalStarter`
//!   (STARTER1..3): Grass starter ⇒ rival Fire ⇒ MEGA_DRAIN, and so on — the
//!   mapping below inverts from the rival's actual final starter species, which
//!   the port knows directly.

use crate::battle::state::Pokemon;
use crate::pokemon::move_learning::get_move_max_pp;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::trainer_data::{gym_leader_no, lone_move, team_move, TrainerClass};

fn set_slot3(mon: &mut Pokemon, mv: MoveId) {
    mon.moves[2] = mv;
    mon.pp[2] = get_move_max_pp(mv);
}

/// Apply LoneMoves / TeamMoves / the champion-rival moves to an already-built
/// enemy trainer party (in class order — the first LoneMove match wins, exactly
/// like the asm's early returns).
pub fn apply_trainer_special_moves(class: TrainerClass, party: &mut [Pokemon]) {
    // LoneMove: gym leaders (wGymLeaderNo = the LoneMoves index).
    if let Some(no) = gym_leader_no(class) {
        if let Some((idx, mv)) = lone_move(no) {
            if let Some(mon) = party.get_mut(idx) {
                set_slot3(mon, mv);
            }
            return;
        }
    }
    // TeamMove: Elite Four — the 5th member's slot 3.
    if let Some(mv) = team_move(class) {
        if let Some(mon) = party.get_mut(4) {
            set_slot3(mon, mv);
        }
        return;
    }
    // Champion rival (RIVAL3): Pidgeot (1st) gets SKY_ATTACK, and the rival's
    // starter (6th) gets the counter-move keyed to the player's starter.
    if class == TrainerClass::Rival3 {
        if let Some(mon) = party.first_mut() {
            set_slot3(mon, MoveId::SkyAttack);
        }
        if let Some(mon) = party.get_mut(5) {
            let counter = match mon.species {
                Species::Venusaur => MoveId::FireBlast,  // player took Squirtle ⇒ rival Grass
                Species::Charizard => MoveId::Blizzard,  // player took Bulbasaur ⇒ rival Fire
                Species::Blastoise => MoveId::MegaDrain, // player took Charmander ⇒ rival Water
                _ => MoveId::Blizzard,
            };
            set_slot3(mon, counter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;

    fn party_of(species_levels: &[(Species, u8)]) -> Vec<Pokemon> {
        species_levels
            .iter()
            .map(|(sp, lv)| {
                create_pokemon_with_moves(*sp, *lv, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
            })
            .collect()
    }

    #[test]
    fn brock_first_mon_gets_bide() {
        let mut p = party_of(&[(Species::Geodude, 12), (Species::Onix, 14)]);
        apply_trainer_special_moves(TrainerClass::Brock, &mut p);
        assert_eq!(p[0].moves[2], MoveId::Bide, "Brock's Geodude: BIDE in slot 3");
        assert_eq!(p[1].moves[2], MoveId::None, "the rest untouched");
    }

    #[test]
    fn lance_fifth_mon_gets_barrier() {
        let mut p = party_of(&[
            (Species::Gyarados, 56),
            (Species::Dragonair, 54),
            (Species::Dragonair, 54),
            (Species::Aerodactyl, 58),
            (Species::Dragonite, 60),
        ]);
        apply_trainer_special_moves(TrainerClass::Lance, &mut p);
        assert_eq!(p[4].moves[2], MoveId::Barrier, "Lance's 5th mon: BARRIER");
        // PP follows the move.
        assert_eq!(p[4].pp[2], get_move_max_pp(MoveId::Barrier));
    }

    #[test]
    fn champion_rival_gets_sky_attack_and_counter() {
        let mut p = party_of(&[
            (Species::Pidgeot, 61),
            (Species::Alakazam, 59),
            (Species::Rhydon, 59),
            (Species::Arcanine, 61),
            (Species::Exeggutor, 61),
            (Species::Venusaur, 63), // rival took Grass (player took Squirtle)
        ]);
        apply_trainer_special_moves(TrainerClass::Rival3, &mut p);
        assert_eq!(p[0].moves[2], MoveId::SkyAttack, "Pidgeot: SKY ATTACK");
        assert_eq!(p[5].moves[2], MoveId::FireBlast, "Grass starter ⇒ FIRE_BLAST");
    }

    #[test]
    fn ordinary_trainer_unchanged() {
        let mut p = party_of(&[(Species::Rattata, 20), (Species::Ekans, 20)]);
        apply_trainer_special_moves(TrainerClass::Youngster, &mut p);
        assert_eq!(p[0].moves, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
    }
}
