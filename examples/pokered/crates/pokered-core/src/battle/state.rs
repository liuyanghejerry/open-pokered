use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use pokered_data::types::PokemonType;
use serde::{Deserialize, Serialize};

use super::stat_stages::StatStages;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleType {
    Wild,
    Trainer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Player,
    Enemy,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        }
    }
}

/// Non-volatile status. Only one active at a time.
/// Sleep counter: 1-7, decremented each turn mon tries to act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StatusCondition {
    #[default]
    None,
    Sleep(u8),
    Poison,
    Burn,
    Freeze,
    Paralysis,
}

impl StatusCondition {
    pub fn is_none(&self) -> bool {
        matches!(self, StatusCondition::None)
    }

    pub fn is_sleep(&self) -> bool {
        matches!(self, StatusCondition::Sleep(_))
    }

    pub fn is_frozen(&self) -> bool {
        matches!(self, StatusCondition::Freeze)
    }
}

/// wPlayerBattleStatus1 / wEnemyBattleStatus1 bit flags.
pub mod status1 {
    pub const STORING_ENERGY: u8 = 1 << 0; // Bide
    pub const THRASHING_ABOUT: u8 = 1 << 1; // Thrash/PetalDance
    pub const MULTI_HIT: u8 = 1 << 2; // DoubleKick, FuryAttack
    pub const FLINCHED: u8 = 1 << 3;
    pub const CHARGING_UP: u8 = 1 << 4; // SolarBeam/Fly/Dig charge
    pub const USING_TRAPPING_MOVE: u8 = 1 << 5; // Wrap/Bind/FireSpin/Clamp
    pub const INVULNERABLE: u8 = 1 << 6; // Fly/Dig semi-invuln
    pub const CONFUSED: u8 = 1 << 7;
}

/// wPlayerBattleStatus2 / wEnemyBattleStatus2 bit flags.
pub mod status2 {
    pub const USING_X_ACCURACY: u8 = 1 << 0;
    pub const PROTECTED_BY_MIST: u8 = 1 << 1;
    pub const GETTING_PUMPED: u8 = 1 << 2; // Focus Energy (bugged)
    pub const HAS_SUBSTITUTE_UP: u8 = 1 << 4;
    pub const NEEDS_TO_RECHARGE: u8 = 1 << 5; // Hyper Beam
    pub const USING_RAGE: u8 = 1 << 6;
    pub const SEEDED: u8 = 1 << 7; // Leech Seed
}

/// wPlayerBattleStatus3 / wEnemyBattleStatus3 bit flags.
pub mod status3 {
    pub const BADLY_POISONED: u8 = 1 << 0; // Toxic
    pub const HAS_LIGHT_SCREEN_UP: u8 = 1 << 1;
    pub const HAS_REFLECT_UP: u8 = 1 << 2;
    pub const TRANSFORMED: u8 = 1 << 3;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pokemon {
    pub species: Species,
    /// Optional nickname. None means use species name.
    /// Max 10 characters (NAME_LENGTH - 1 in original).
    pub nickname: Option<String>,
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub attack: u16,
    pub defense: u16,
    pub speed: u16,
    pub special: u16,
    pub type1: PokemonType,
    pub type2: PokemonType,
    pub moves: [MoveId; 4],
    pub pp: [u8; 4],
    /// Number of PP Ups applied to each move slot (0-3 each).
    pub pp_ups: [u8; 4],
    pub status: StatusCondition,
    /// Gen1 DV bytes: [atk_def, spd_spc]. Each byte packs two 4-bit IVs.
    /// High nybble = Atk/Spd IV, Low nybble = Def/Spc IV.
    /// HP IV is derived: bit3=Atk&1, bit2=Def&1, bit1=Spd&1, bit0=Spc&1.
    pub dv_bytes: [u8; 2],
    /// Stat experience (EVs) accumulated. [hp, atk, def, spd, spc].
    pub stat_exp: [u16; 5],
    pub total_exp: u32,
    pub is_traded: bool,
    /// Original-trainer ID (MON_OTID in the party/box struct). Drives traded-mon
    /// obedience (`CheckForDisobedience`): the mon is traded iff `ot_id != 0 &&
    /// ot_id != player_id` — `0` is "unknown" (mon created before OT IDs were
    /// tracked, or by a code path that never set it) and is treated as own.
    #[serde(default)]
    pub ot_id: u16,
    /// Original-trainer name (decoded text; `None` = unknown/blank). The party/box
    /// OT-name table in SRAM round-trips through this.
    #[serde(default)]
    pub ot_name: Option<String>,
}

impl Pokemon {
    pub fn display_name(&self) -> String {
        self.nickname
            .clone()
            .unwrap_or_else(|| format!("{:?}", self.species).to_uppercase())
    }

    pub fn set_nickname(&mut self, nickname: String) {
        self.nickname = if nickname.len() <= 10 && !nickname.is_empty() {
            Some(nickname)
        } else if nickname.is_empty() {
            None
        } else {
            Some(nickname.chars().take(10).collect())
        };
    }

    pub fn clear_nickname(&mut self) {
        self.nickname = None;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattlerState {
    pub active_pokemon_index: usize,
    pub party: Vec<Pokemon>,
    pub stat_stages: StatStages,
    pub battle_status1: u8,
    pub battle_status2: u8,
    pub battle_status3: u8,
    pub substitute_hp: u8,
    pub confused_turns_left: u8,
    pub toxic_counter: u8,
    pub disabled_move: u8,
    pub disabled_turns_left: u8,
    pub num_attacks_left: u8,
    pub num_hits: u8,
    pub bide_accumulated_damage: u16,
    pub selected_move: MoveId,
    pub selected_move_index: u8,
    pub player_used_move: bool,
    pub unmodified_attack: u16,
    pub unmodified_defense: u16,
    pub unmodified_speed: u16,
    pub unmodified_special: u16,
    pub last_move_used: MoveId,
    /// Conversion (变身术) type override — the active mon's battle-only type1/type2
    /// after a Conversion. `None` ⇒ use the species-derived types. BATTLE-ONLY: reset
    /// by [`reset_volatile_status`](Self::reset_volatile_status) (so it clears on
    /// switch/battle end) and never copied into the persistent save (the writeback
    /// only clones `party`), so a Conversion can never permanently retype a mon.
    /// `#[serde(default)]` keeps old snapshots loadable.
    #[serde(default)]
    pub conversion_type1: Option<PokemonType>,
    #[serde(default)]
    pub conversion_type2: Option<PokemonType>,
    /// Gen-1 badge stat boosts (`ApplyBadgeStatBoosts`): the active mon's
    /// badge-boosted UNSTAGED stats `[atk, def, spd, spc]` — the `wBattleMon*`
    /// working copy. Persisted across turns because the stat-up glitch
    /// re-applies (and compounds) the boosts mid-battle; `None` = not yet
    /// initialised (lazy-init on first use = the send-out boost). PLAYER side
    /// only. See [`super::badge_boosts`].
    #[serde(default)]
    pub badge_boosted_stats: Option<[u16; 4]>,
}

impl BattlerState {
    pub fn active_mon(&self) -> &Pokemon {
        &self.party[self.active_pokemon_index]
    }

    pub fn active_mon_mut(&mut self) -> &mut Pokemon {
        &mut self.party[self.active_pokemon_index]
    }

    pub fn has_status1(&self, flag: u8) -> bool {
        self.battle_status1 & flag != 0
    }

    pub fn set_status1(&mut self, flag: u8) {
        self.battle_status1 |= flag;
    }

    pub fn clear_status1(&mut self, flag: u8) {
        self.battle_status1 &= !flag;
    }

    pub fn has_status2(&self, flag: u8) -> bool {
        self.battle_status2 & flag != 0
    }

    pub fn set_status2(&mut self, flag: u8) {
        self.battle_status2 |= flag;
    }

    pub fn clear_status2(&mut self, flag: u8) {
        self.battle_status2 &= !flag;
    }

    pub fn has_status3(&self, flag: u8) -> bool {
        self.battle_status3 & flag != 0
    }

    pub fn set_status3(&mut self, flag: u8) {
        self.battle_status3 |= flag;
    }

    pub fn clear_status3(&mut self, flag: u8) {
        self.battle_status3 &= !flag;
    }

    pub fn reset_volatile_status(&mut self) {
        self.battle_status1 = 0;
        self.battle_status2 = 0;
        self.battle_status3 = 0;
        self.stat_stages.reset();
        self.substitute_hp = 0;
        self.confused_turns_left = 0;
        self.toxic_counter = 0;
        self.disabled_move = 0;
        self.disabled_turns_left = 0;
        self.num_attacks_left = 0;
        self.num_hits = 0;
        self.bide_accumulated_damage = 0;
        self.player_used_move = false;
        self.last_move_used = MoveId::None;
        // Conversion ends on switch-out (Gen-1 clears the battle type change).
        self.conversion_type1 = None;
        self.conversion_type2 = None;
        // Badge boosts live on the per-mon working copy (`wBattleMon*`): a fresh
        // mon re-applies them from scratch at send-out (core.asm:1659), so the
        // accumulated stat-up-glitch rounds reset here.
        self.badge_boosted_stats = None;
    }

    pub fn refresh_unmodified_stats(&mut self) {
        let mon = &self.party[self.active_pokemon_index];
        self.unmodified_attack = mon.attack;
        self.unmodified_defense = mon.defense;
        self.unmodified_speed = mon.speed;
        self.unmodified_special = mon.special;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleState {
    pub battle_type: BattleType,
    pub player: BattlerState,
    pub enemy: BattlerState,
    pub whose_turn: Side,
    pub move_missed: bool,
    /// 0=normal, 1=crit, 2=OHKO success, 0xFF=OHKO fail
    pub critical_or_ohko: u8,
    pub damage: u16,
    pub num_run_attempts: u8,
    pub escaped: bool,
    pub party_fought_flags: [bool; 6],
    pub party_gain_exp_flags: [bool; 6],
    /// `wCanEvolveFlags` — set for a party mon each time it levels up in
    /// battle (`FlagAction` in the level-up path, engine/battle/experience.asm:257),
    /// cleared at battle init (engine/battle/init_battle_variables.asm:22).
    /// `EvolutionAfterBattle` only attempts level-up evolutions for flagged
    /// mons, which is what makes a B-cancelled evolution retry on the NEXT
    /// level-up instead of after every battle.
    #[serde(default)]
    pub party_leveled_up_flags: [bool; 6],
    pub total_payday_money: u32,
    pub is_battle_over: bool,
    /// `wObtainedBadges` for this battle — drives the badge stat boosts
    /// ([`super::badge_boosts`]) and traded-mon obedience thresholds
    /// ([`super::obedience`]). Synced from the frontend at battle start.
    #[serde(default)]
    pub player_badges: u8,
    /// `wPlayerID` — the player's trainer ID, compared against a party mon's
    /// `ot_id` by `CheckForDisobedience`.
    #[serde(default)]
    pub player_id: u16,
}

impl BattleState {
    pub fn attacker(&self) -> &BattlerState {
        match self.whose_turn {
            Side::Player => &self.player,
            Side::Enemy => &self.enemy,
        }
    }

    pub fn defender(&self) -> &BattlerState {
        match self.whose_turn {
            Side::Player => &self.enemy,
            Side::Enemy => &self.player,
        }
    }

    pub fn attacker_mut(&mut self) -> &mut BattlerState {
        match self.whose_turn {
            Side::Player => &mut self.player,
            Side::Enemy => &mut self.enemy,
        }
    }

    pub fn defender_mut(&mut self) -> &mut BattlerState {
        match self.whose_turn {
            Side::Player => &mut self.enemy,
            Side::Enemy => &mut self.player,
        }
    }

    pub fn side(&self, side: Side) -> &BattlerState {
        match side {
            Side::Player => &self.player,
            Side::Enemy => &self.enemy,
        }
    }

    pub fn side_mut(&mut self, side: Side) -> &mut BattlerState {
        match side {
            Side::Player => &mut self.player,
            Side::Enemy => &mut self.enemy,
        }
    }
}

pub fn new_battler_state(party: Vec<Pokemon>) -> BattlerState {
    let mon = &party[0];
    let attack = mon.attack;
    let defense = mon.defense;
    let speed = mon.speed;
    let special = mon.special;
    BattlerState {
        active_pokemon_index: 0,
        party,
        stat_stages: StatStages::default(),
        battle_status1: 0,
        battle_status2: 0,
        battle_status3: 0,
        substitute_hp: 0,
        confused_turns_left: 0,
        toxic_counter: 0,
        disabled_move: 0,
        disabled_turns_left: 0,
        num_attacks_left: 0,
        num_hits: 0,
        bide_accumulated_damage: 0,
        selected_move: MoveId::None,
        selected_move_index: 0,
        player_used_move: false,
        unmodified_attack: attack,
        unmodified_defense: defense,
        unmodified_speed: speed,
        unmodified_special: special,
        last_move_used: MoveId::None,
        conversion_type1: None,
        conversion_type2: None,
        badge_boosted_stats: None,
    }
}

pub fn new_battle_state(
    battle_type: BattleType,
    player_party: Vec<Pokemon>,
    enemy_party: Vec<Pokemon>,
) -> BattleState {
    BattleState {
        battle_type,
        player: new_battler_state(player_party),
        enemy: new_battler_state(enemy_party),
        whose_turn: Side::Player,
        move_missed: false,
        critical_or_ohko: 0,
        damage: 0,
        num_run_attempts: 0,
        escaped: false,
        party_fought_flags: [false; 6],
        party_gain_exp_flags: [false; 6],
        party_leveled_up_flags: [false; 6],
        total_payday_money: 0,
        is_battle_over: false,
        player_badges: 0,
        player_id: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_pokemon() -> Pokemon {
        Pokemon {
            species: Species::Pikachu,
            nickname: None,
            level: 25,
            hp: 55,
            max_hp: 55,
            attack: 55,
            defense: 30,
            speed: 90,
            special: 50,
            type1: PokemonType::Electric,
            type2: PokemonType::Electric,
            moves: [
                MoveId::Thundershock,
                MoveId::QuickAttack,
                MoveId::ThunderWave,
                MoveId::None,
            ],
            pp: [30, 30, 20, 0],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    #[test]
    fn side_opposite() {
        assert_eq!(Side::Player.opposite(), Side::Enemy);
        assert_eq!(Side::Enemy.opposite(), Side::Player);
    }

    /// reset_volatile_status clears the Conversion type override (so a Conversion ends
    /// on switch-out, where this is called), alongside the other battle volatiles.
    #[test]
    fn reset_volatile_status_clears_conversion_override() {
        let mut b = new_battler_state(vec![make_test_pokemon()]);
        b.conversion_type1 = Some(PokemonType::Ghost);
        b.conversion_type2 = Some(PokemonType::Poison);
        b.disabled_move = 2;
        b.disabled_turns_left = 3;
        b.reset_volatile_status();
        assert_eq!(b.conversion_type1, None, "Conversion override cleared on reset");
        assert_eq!(b.conversion_type2, None);
        assert_eq!(b.disabled_move, 0, "Disable cleared on reset too");
        assert_eq!(b.disabled_turns_left, 0);
    }

    #[test]
    fn status_condition_checks() {
        assert!(StatusCondition::None.is_none());
        assert!(!StatusCondition::Poison.is_none());
        assert!(StatusCondition::Sleep(3).is_sleep());
        assert!(!StatusCondition::Burn.is_sleep());
        assert!(StatusCondition::Freeze.is_frozen());
    }

    #[test]
    fn battler_status_flag_operations() {
        let party = vec![make_test_pokemon()];
        let mut battler = new_battler_state(party);

        assert!(!battler.has_status1(status1::CONFUSED));
        battler.set_status1(status1::CONFUSED);
        assert!(battler.has_status1(status1::CONFUSED));
        battler.clear_status1(status1::CONFUSED);
        assert!(!battler.has_status1(status1::CONFUSED));

        battler.set_status2(status2::SEEDED | status2::USING_RAGE);
        assert!(battler.has_status2(status2::SEEDED));
        assert!(battler.has_status2(status2::USING_RAGE));
    }

    #[test]
    fn battler_active_mon() {
        let party = vec![make_test_pokemon()];
        let battler = new_battler_state(party);
        assert_eq!(battler.active_mon().species, Species::Pikachu);
        assert_eq!(battler.active_mon().level, 25);
    }

    #[test]
    fn battler_reset_volatile() {
        let party = vec![make_test_pokemon()];
        let mut battler = new_battler_state(party);
        battler.set_status1(status1::CONFUSED | status1::FLINCHED);
        battler.set_status2(status2::SEEDED);
        battler.set_status3(status3::BADLY_POISONED);
        battler.confused_turns_left = 3;
        battler.toxic_counter = 5;
        battler.substitute_hp = 20;

        battler.reset_volatile_status();

        assert_eq!(battler.battle_status1, 0);
        assert_eq!(battler.battle_status2, 0);
        assert_eq!(battler.battle_status3, 0);
        assert_eq!(battler.confused_turns_left, 0);
        assert_eq!(battler.toxic_counter, 0);
        assert_eq!(battler.substitute_hp, 0);
    }

    #[test]
    fn battle_state_attacker_defender() {
        let player_party = vec![make_test_pokemon()];
        let enemy_party = vec![make_test_pokemon()];
        let mut state: BattleState = new_battle_state(BattleType::Wild, player_party, enemy_party);

        state.whose_turn = Side::Player;
        assert_eq!(state.attacker().active_mon().species, Species::Pikachu);

        state.whose_turn = Side::Enemy;
        assert_eq!(state.attacker().active_mon().species, Species::Pikachu);
    }

    #[test]
    fn new_battle_state_defaults() {
        let player_party = vec![make_test_pokemon()];
        let enemy_party = vec![make_test_pokemon()];
        let state: BattleState = new_battle_state(BattleType::Trainer, player_party, enemy_party);

        assert_eq!(state.battle_type, BattleType::Trainer);
        assert!(!state.is_battle_over);
        assert!(!state.escaped);
        assert_eq!(state.damage, 0);
        assert_eq!(state.num_run_attempts, 0);
        assert_eq!(state.player.unmodified_speed, 90);
        assert_eq!(state.enemy.unmodified_attack, 55);
    }
}
