pub mod accuracy;
pub mod badge_boosts;
pub mod capture;
pub mod safari;
pub mod damage;
pub mod effects;
pub mod escape;
pub mod experience;
pub mod link_battle_driver;
pub mod menu;
pub mod move_execution;
pub mod obedience;
pub mod residual;
pub mod settlement;
pub mod stat_stages;
pub mod state;
pub mod status_checks;
pub mod trainer_ai;
pub mod turn;
pub mod turn_order;
pub mod types;
pub mod wild;

use jrpg_engine::battle::rng::BattleRng as _;

#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod link_battle_driver_tests;
#[cfg(test)]
mod catch_tests;

/// Reusable effect-stack **parity harness** (strangler slice 1) — additive,
/// test-only, does NOT touch the production turn loop. The byte-stream shim,
/// the `ScriptedRng` builder, the consumed-count predictor, the `BattleState`
/// differential oracle, and the standing crit-before-accuracy draw-order guard
/// live here so later slices (2–7) add scenarios without rebuilding the
/// plumbing (design doc `06-battle-engine-effect-stack-design.md` §4.1 + §7).
#[cfg(test)]
pub(crate) mod stack_parity;

/// Effect-stack battle engine POC — slice-1 scenarios; additive, test-only,
/// does NOT touch the production turn loop. Re-points its parity tests onto the
/// reusable [`stack_parity`] harness (design doc
/// `06-battle-engine-effect-stack-design.md`).
#[cfg(test)]
mod stack_poc;

/// Effect-stack battle engine — slice-2 scenarios: the Gen-1 `BeforeMove` status
/// gate (sleep/freeze/confusion/paralysis) as ordered effect handlers, proven at
/// parity with the legacy `check_status_conditions`/`execute_turn` oracle.
/// Additive, test-only; builds on the reusable [`stack_parity`] harness (design
/// doc `06-battle-engine-effect-stack-design.md` §6 bugs #8/#10/#12/#13, slice 2
/// of the §7 plan).
#[cfg(test)]
mod stack_slice2;

/// Effect-stack battle engine — slice-3 scenarios: the Gen-1 crit → accuracy →
/// damage pipeline as effect handlers (real `crit_chance`/`is_high_crit_move`/
/// `accuracy_check`/`calculate_damage`), proven at parity with the legacy
/// `execute_turn` oracle. Additive, test-only; builds on the reusable
/// [`stack_parity`] harness (design doc
/// `06-battle-engine-effect-stack-design.md` §6 bugs #1/#2/#3/#4/#5/#29, slice 3
/// of the §7 plan).
#[cfg(test)]
mod stack_slice3;

/// Effect-stack battle engine — slice-4 scenarios: **Substitute + partial-trap as
/// cross-battler handlers** — the FIRST real exercise of the engine's `pair_mut`
/// (the Substitute interceptor reads the attacker and writes the defender's
/// volatile through the cross-side raw-pointer borrow), proven at parity with the
/// legacy `execute_move` Substitute absorb/break and `check_status_conditions`
/// trapped gate. Additive, test-only; builds on the reusable [`stack_parity`]
/// harness (design doc `06-battle-engine-effect-stack-design.md` §3 pair_mut +
/// `EffectStateKind::{Substitute,Trapping}`, §6 #28/#16, slice 4 of the §7 plan).
#[cfg(test)]
mod stack_slice4;

/// Effect-stack battle engine — slice-5 scenarios: **Gen-1 end-of-turn residuals
/// as effect handlers** (burn/poison flat `/16`, Toxic uncapped ramp via an arena
/// counter, Leech Seed cross-battler drain+heal via the genuinely load-bearing
/// `pair_mut`), fired per-mover in ASM order (status damage then leech) with the
/// first-mover-faint short-circuit, proven at parity with the legacy
/// `apply_all_residual` / `execute_turn` oracle. Additive, test-only; builds on the
/// reusable [`stack_parity`] harness (design doc
/// `06-battle-engine-effect-stack-design.md` §3.3 Toxic bug #6, §6 #6/#7, §2
/// per-mover interleave, slice 5 of the §7 plan).
#[cfg(test)]
mod stack_slice5;

/// Effect-stack battle engine — slice-6 scenarios: **multi-turn lock-in
/// (Thrash/Fly/Hyper-Beam-recharge) + Counter + Bide as effect-stack handlers**.
/// Lock-in volatiles live in the arena (cross-turn) and hijack the per-turn chosen
/// action via the engine's generic, defaulted `EffectProvider::forced_action`
/// seam (`MoveGate::ForcedAction`-shaped); Counter reflects 2× the physical damage
/// taken via the now-load-bearing `pair_mut`; Bide accumulates damage taken across
/// turns and unleashes ×2. Resolves the design §9 open question (last_damage +
/// locked state placement). Additive, test-only; builds on the reusable
/// [`stack_parity`] harness (design doc `06-battle-engine-effect-stack-design.md`
/// §3.1 `LockedMove`/`TwoTurn`/`Bide`, §6 #14/#15/#17/#18/#20, §9, slice 6 of the
/// §7 plan).
#[cfg(test)]
mod stack_slice6;

/// Effect-stack battle engine — slice-7 scenarios: **representative Gen-1
/// SECONDARY / SPECIAL effects as `DamagingHit` (the design §1.1
/// `AfterMoveSecondary`) handlers**, one per category (status-on-hit, stat-drop-
/// on-hit, flinch, recoil, drain, plus the Haze global effect), each re-homing the
/// matching `apply_move_effect` legacy fn and proven at parity vs the legacy
/// `execute_turn` oracle. Recoil/drain read `MoveContext.last_damage` (validating
/// slice 6's same-action placement); the side-effect roll draw order + boundary
/// (threshold-1 fires / threshold does not) + `consumed()` are pinned. Engine
/// UNTOUCHED — reuses the existing `DamagingHit` post-damage event. Additive,
/// test-only; builds on the reusable [`stack_parity`] harness (design doc
/// `06-battle-engine-effect-stack-design.md` §1.1 `AfterMoveSecondary`, §6
/// secondary/special entries, slice 7 of the §7 plan).
#[cfg(test)]
mod stack_slice7;

/// **P0 — the production RNG shim + two-mover + AI differential** (migration
/// blueprint `15` §4/§5 P0). The make-or-break prerequisite: it proves the AI
/// `pick_enemy_move` draw — which the production live loop
/// `BattleScreen::execute_turn_with_move` draws FIRST (`mod.rs:1766`), STRICTLY
/// BEFORE the `generate_turn_randoms` pre-roll (`mod.rs:1784`) — is a clean RNG
/// PREFIX (NOT a mid-`TurnRandoms` interleave). The harness re-homes the REAL AI
/// code (`move_choice_layers`/`choose_moves`/`pick_move`) over a shared
/// `BattleRng`, lays the AI byte(s) at the front of ONE byte vector, drives both
/// the legacy `execute_turn` oracle AND the `StackDriver` on it, and asserts
/// IDENTICAL resulting `BattleState` + identical `consumed()` across ≥20
/// scenarios (incl. the AI pick flipping turn order). Additive, test-only;
/// production battle behavior UNTOUCHED. Pins: order byte drawn first exactly
/// once even on a tie; crit drawn before accuracy even with the AI prefix.
#[cfg(test)]
mod stack_p0_ai;

/// **P1 — the pokered `RulesProvider` + bucket-A moves authored as RON data**
/// (migration blueprint `15` §2/§5 P1). Stands up a game-side `PokeredRules`
/// provider (over the REAL `Species`/`MoveId`/`StatIndex`/`PokemonType`/
/// `StatusCondition`) that drives the bucket-A Gen-1 effects through the engine's
/// `StackDriver` using effects authored in `pokered_rules/rules.ron` (loaded via
/// the game-agnostic `jrpg-rules` loader, dual-mode baked + hot-reload).
/// `calculate_damage` stays the single damage authority (precomputed into
/// `ctx.mv.damage`); `DealMoveDamage`/`ApplyTypeChart` are the declarative
/// markers (the chart already rides the damage authority, so `ApplyTypeChart`
/// folds neutral). Additive + DIFFERENTIAL-ONLY: the legacy `apply_move_effect`/
/// `execute_turn` dispatcher stays the production oracle, untouched; this module
/// only proves IDENTICAL `BattleState` + identical `rng.consumed()` vs legacy on
/// real Gen-1 numbers (STAB super-effective / resisted / neutral, crit, 1/256
/// miss, +1/+2 self-Boost with the −6..+6 clamp, self-heal). Bucket-A moves
/// authored as DATA vs DEFERRED (Drain/Recoil/PayDay need a `lastDamage` /
/// coin-award primitive) are reported in the module doc.
///
/// PRODUCTIONIZED (P6 flip): no longer `#[cfg(test)]` — the provider + `runtime`
/// drive the live battle loop. The differential tests inside self-gate.
mod pokered_rules;

// ── BattleScreen (frame-loop adapter) ─────────────────────────────

use crate::battle::experience::gain::{calc_exp_gain, gain_experience};
use crate::battle::settlement::money::{calc_prize_money, calc_total_winnings};
use crate::battle::settlement::settle::settle_battle;
use crate::battle::settlement::{BattleOutcome, BattleSettlement};
use crate::game_state::{BattleStyle, GameScreen, ScreenAction};
use crate::items::inventory::Inventory;
use crate::main_menu::MenuInput;
use effects::EffectRandoms;
use escape::{try_run_from_battle, RunResult};
use menu::{
    BagMenuResult, BagMenuState, BattleMenuAction, BattleMenuInput, BattleMenuState, ItemCategory,
    MoveMenuResult, MoveMenuState, MoveSlot, PartySubMenuAction, PartySubMenuState,
};
use move_execution::MoveRandoms;
use pokered_data::items::ItemId;
use pokered_data::move_data::MoveData;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::trainer_data::TrainerClass;
use state::{BattleState, BattleType, StatusCondition};

/// Per-slot pokeball indicator status for battle HUD display.
///
/// Matches the original PickPokeball logic in draw_hud_pokeball_gfx.asm:
/// - Normal: healthy mon in party
/// - StatusAilment: mon has a non-volatile status condition (poisoned, paralyzed, etc.)
/// - Fainted: mon's current HP is 0
/// - Empty: party slot is unused (beyond party count)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokeballSlotStatus {
    Normal,
    StatusAilment,
    Fainted,
    Empty,
}

impl Default for PokeballSlotStatus {
    fn default() -> Self {
        Self::Empty
    }
}
use trainer_ai::ai_action::{execute_ai_action, AiAction};
use trainer_ai::move_choice::choose_moves;
use trainer_ai::move_choice_layers;
use trainer_ai::trainer_ai_config;

/// Battle transition type (screen wipe effect) matching ASM's BattleTransitions table.
/// Selected via 3 bits: trainer flag + stronger enemy flag + dungeon map flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleTransition {
    /// %000 — wild, not stronger, not dungeon: double half-circle wipe
    DoubleCircle,
    /// %001 — trainer, not stronger, not dungeon: spiral (inward for weaker, outward for stronger)
    Spiral { outward: bool },
    /// %010 — wild, stronger, not dungeon: single circle wipe
    Circle,
    /// %011 — trainer, stronger, not dungeon: spiral
    SpiralTrainerStronger,
    /// %100 — wild, not stronger, dungeon: horizontal stripes
    HorizontalStripes,
    /// %101 — trainer, not stronger, dungeon: shrink to center
    Shrink,
    /// %110 — wild, stronger, dungeon: vertical stripes
    VerticalStripes,
    /// %111 — trainer, stronger, dungeon: split apart
    Split,
}

impl BattleTransition {
    /// Select the transition based on 3 bits: is_trainer, is_stronger, is_dungeon.
    /// Matches GetBattleTransitionID_* logic from engine/battle/battle_transitions.asm.
    pub fn select(is_trainer: bool, enemy_level: u8, player_first_alive_level: u8, map_id: u8) -> Self {
        let is_stronger = enemy_level >= player_first_alive_level.saturating_add(3);
        let is_dungeon = Self::is_dungeon_map(map_id);

        match (is_trainer, is_stronger, is_dungeon) {
            (false, false, false) => Self::DoubleCircle,
            (true,  false, false) => Self::Spiral { outward: false },
            (false, true,  false) => Self::Circle,
            (true,  true,  false) => Self::SpiralTrainerStronger,
            (false, false, true)  => Self::HorizontalStripes,
            (true,  false, true)  => Self::Shrink,
            (false, true,  true)  => Self::VerticalStripes,
            (true,  true,  true)  => Self::Split,
        }
    }

    /// Check if the given map ID is a dungeon map.
    /// Matches DungeonMaps1/DungeonMaps2 from data/maps/dungeon_maps.asm.
    fn is_dungeon_map(map_id: u8) -> bool {
        // DungeonMaps1 — exact matches
        const DUNGEON_MAPS_1: &[u8] = &[
            0x01, // VIRIDIAN_FOREST
            0x1E, // ROCK_TUNNEL_1F
            0x20, // SEAFOAM_ISLANDS_1F
            0x21, // ROCK_TUNNEL_B1F
        ];
        if DUNGEON_MAPS_1.iter().any(|&m| m == map_id) {
            return true;
        }
        // DungeonMaps2 — range checks
        const DUNGEON_RANGES: &[(u8, u8)] = &[
            (0x04, 0x06), // MT_MOON_1F .. MT_MOON_B2F
            (0x10, 0x1B), // SS_ANNE_1F .. HALL_OF_FAME (includes VICTORY_ROAD_1F, LANCES_ROOM)
            (0x2C, 0x2D), // LAVENDER_POKECENTER .. LAVENDER_CUBONE_HOUSE (POKEMON_TOWER area)
            (0x32, 0x3E), // SILPH_CO_2F .. CERULEAN_CAVE_1F (includes POKEMON_MANSION, SAFARI_ZONE)
        ];
        DUNGEON_RANGES.iter().any(|(lo, hi)| map_id >= *lo && map_id <= *hi)
    }
}

impl Default for BattleTransition {
    fn default() -> Self {
        Self::DoubleCircle
    }
}

/// Duration of the FlashScreen strobe in frames: 12 palette steps held 2
/// frames each (`ld c, 2 / DelayFrames`), the whole sequence repeated 3 times
/// (`ld b, $3`) — engine/battle/battle_transitions.asm:329-358.
pub const TRANSITION_FLASH_FRAMES: u16 = 12 * 2 * 3;

/// First intro phase for a given transition. In the original, ONLY the
/// Circle and DoubleCircle transitions (wild, non-dungeon) run the
/// FlashScreen palette strobe BEFORE the wipe — BattleTransition_Circle and
/// BattleTransition_DoubleCircle call BattleTransition_FlashScreen first; the
/// spiral/stripes/shrink/split transitions do not.
fn intro_start_phase(transition: BattleTransition) -> BattlePhase {
    if matches!(
        transition,
        BattleTransition::Circle | BattleTransition::DoubleCircle
    ) {
        BattlePhase::Intro {
            phase: IntroPhase::TransitionFlash,
            wait_frames: TRANSITION_FLASH_FRAMES,
        }
    } else {
        BattlePhase::Intro {
            phase: IntroPhase::BattleTransitionWipe(transition),
            wait_frames: 120,
        }
    }
}

/// Sub-phase of the battle intro sequence, matching the original game's flow.
///
/// Wild battles:
/// 0. FlashScreen palette strobe — ONLY for the Circle/DoubleCircle
///    transitions (wild, non-dungeon), which call BattleTransition_FlashScreen
///    before the wipe (12 palette steps × 2 frames × 3 repeats = 72 frames)
/// 1. Battle screen wipe transition (selected by 3-bit flags)
/// 2. Silhouettes slide in (inverted palette, sliding from right, 72 frames)
/// 3. Wild Pokémon reveal (palette normalizes, "Wild X appeared!")
/// 4. Player sends out first Pokémon ("Go! X!" with Poké Ball throw animation)
///
/// Trainer battles (matches engine/battle/core.asm):
/// 0. Battle screen wipe transition (selected by 3-bit flags) — no FlashScreen
///    (the spiral/stripes/shrink/split transitions do not call it)
/// 1. Silhouettes slide in (player back sprite + enemy trainer sprite)
/// 2. Trainer reveal — "X wants to fight!" text shown (enemy trainer pic visible)
/// 3. Trainer sends out Pokémon — trainer pic slides off, enemy Pokémon appears
/// 4. Player sends out first Pokémon — "Go! X!" with Poké Ball throw animation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroPhase {
    /// Battle screen wipe transition (8 variants based on trainer/wild + level + dungeon)
    BattleTransitionWipe(BattleTransition),
    /// FlashScreen palette strobe (72 frames = 12 steps × 2 frames × 3 repeats,
    /// `ld b, $3` in BattleTransition_FlashScreen). Only runs before the
    /// Circle/DoubleCircle wipes, exactly like the original.
    TransitionFlash,
    /// Silhouettes sliding onto screen (72 frames, matching ASM's $70→$00 SCX slide)
    SilhouetteSlide,
    /// Wild Pokémon revealed — "Wild X appeared!" text shown (30 frames).
    /// A Pokémon-Tower GHOST battle (no SILPH SCOPE) shows "Enemy GHOST appeared!"
    /// here instead (engine/battle/common_text.asm's `.noSilphScope`).
    WildReveal,
    /// No-scope ghost battle only: the second intro line, "Darn! The GHOST
    /// can't be ID'd!" (GhostCantBeIDdText), printed right after
    /// "Enemy GHOST appeared!" and before the player send-out.
    GhostCantID,
    /// Ghost-Marowak battle WITH the SILPH SCOPE only: "SILPH SCOPE unveiled
    /// the GHOST's identity!" (UnveiledGhostText) while the frontend runs the
    /// MarowakAnim reveal (engine/battle/ghost_marowak_anim.asm); then the
    /// intro loops back to WildReveal as a normal "Wild MAROWAK appeared!".
    GhostUnveil,
    /// Trainer revealed — enemy trainer sprite visible, "X wants to fight!" text (30 frames)
    TrainerReveal,
    /// Trainer sends out Pokémon — trainer pic slides off, enemy Pokémon slides in (30 frames)
    TrainerSendOut,
    /// Player sends out first Pokémon — "Go! X!" with Poké Ball throw animation
    PlayerSendOut,
}

/// Sub-phases of trainer battle victory sequence.
/// Matches ASM's TrainerBattleVictory flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryPhase {
    /// Print "Trainer defeated!" text and play victory music
    DefeatedText,
    /// Trainer pic scrolls back onto screen from right (ScrollTrainerPicAfterBattle)
    TrainerPicScrollIn,
    /// Wait 40 frames before end battle text
    WaitFrames,
    /// Print end battle text and show money won
    EndBattleText,
}

/// High-level battle phase (frame-loop granularity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattlePhase {
    Intro {
        /// Current sub-phase of the intro sequence.
        phase: IntroPhase,
        /// Frames remaining in the current sub-phase.
        wait_frames: u16,
    },
    /// Player picks FIGHT / BAG / POKéMON / RUN.
    PlayerMenu,
    /// Player picks a move from the move list.
    MoveSelect,
    /// Player selects an item from their bag.
    BagSelect,
    /// Player selects a party member to use item on (for healing/status items).
    ItemTargetSelect { item_id: ItemId },
    /// Displaying sequential text messages (turn results, status, etc.).
    /// Advances on A press. After all messages → next phase.
    ShowingText {
        messages: Vec<String>,
        current: usize,
        /// Frames to auto-wait before accepting input (brief pause).
        wait_frames: u16,
        /// Phase to transition to after all messages are shown.
        next_phase: Box<BattlePhase>,
    },
    /// Player chooses which party member to switch to.
    PartySelect,
    /// Player selected a party member, showing SWITCH/STATS/CANCEL menu.
    PartySubMenu { selected_index: usize },
    /// Viewing a party member's stats screen.
    PartyStats { pokemon_index: usize },
    /// Enemy trainer sends out next Pokémon after one faints.
    EnemySendingNext { wait_frames: u16 },
    /// SHIFT battle style only (original `BIT_BATTLE_SHIFT` clear, trainer
    /// battle, player party ≥ 2 — engine/battle/core.asm `ReplaceFaintedEnemyMon`):
    /// after an enemy TRAINER's mon faints and before the next is sent out, the
    /// prompt "Enemy X is about to use Y! Will <PLAYER> change #MON?"
    /// (`TrainerAboutToUseText`, data/text/text_2.asm) waits on a YES/NO answer
    /// (cursor defaults to NO, like the original's `wCurrentMenuItem = 1`).
    ShiftPrompt,
    /// Player answered YES to the shift prompt: pick the Pokémon to switch in
    /// (free switch — the fainted enemy cannot attack). B proceeds without
    /// switching, exactly like the original party menu here.
    ShiftSwitchSelect,
    /// Player must choose replacement after their Pokémon faints.
    PlayerFaintSwitch,
    /// Link battle: the local action was chosen and sent; the screen waits
    /// for the remote player's action (the driver resolves via
    /// `resolve_link_turn` once both actions are exchanged). Input is ignored
    /// — the local player cannot undo a committed link action.
    LinkWaiting,
    /// Trainer battle victory sequence (ASM: TrainerBattleVictory flow).
    /// 1. DefeatedText → 2. TrainerPicScrollIn → 3. WaitFrames → 4. EndBattleText → exit
    TrainerVictory {
        phase: VictoryPhase,
        wait_frames: u16,
        player_won: bool,
    },
    /// Wild battle or other battle over — simple wait then exit.
    BattleOver {
        won: bool,
        escaped: bool,
        wait_frames: u16,
    },
}

/// Input forwarded to the battle screen each frame.
#[derive(Debug, Clone, Copy)]
pub struct BattleInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
}

impl BattleInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            a: false,
            b: false,
        }
    }
}

use status_checks::CannotMoveReason;

const BATTLE_TEXT_LINE_WIDTH: usize = 18;
const BATTLE_TEXT_LINES_PER_PAGE: usize = 2;
const BATTLE_TEXT_PAGE_WAIT_FRAMES: u16 = 10;

fn hard_wrap_word(word: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![];
    }
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }

    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + width).min(chars.len());
        out.push(chars[start..end].iter().collect());
        start = end;
    }
    out
}

fn wrap_battle_text_lines(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();

    for raw_line in text.split('\n') {
        if raw_line.trim().is_empty() {
            out.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            let parts = hard_wrap_word(word, width);
            for part in parts {
                if current.is_empty() {
                    current.push_str(&part);
                    continue;
                }

                let candidate_len = current.chars().count() + 1 + part.chars().count();
                if candidate_len <= width {
                    current.push(' ');
                    current.push_str(&part);
                } else {
                    out.push(current);
                    current = part;
                }
            }
        }

        if !current.is_empty() {
            out.push(current);
        }
    }

    if out.is_empty() {
        out.push(String::new());
    }

    out
}

fn paginate_battle_text(text: &str) -> Vec<String> {
    let lines = wrap_battle_text_lines(text, BATTLE_TEXT_LINE_WIDTH);
    let mut pages = Vec::new();

    for chunk in lines.chunks(BATTLE_TEXT_LINES_PER_PAGE) {
        pages.push(chunk.join("\n"));
    }

    if pages.is_empty() {
        pages.push(String::new());
    }

    pages
}

/// Convert a PascalCase MoveId Debug name to game-style uppercase with spaces.
/// e.g. "QuickAttack" → "QUICK ATTACK", "Thunderbolt" → "THUNDERBOLT",
///      "HiJumpKick" → "HI JUMP KICK", "ThunderWave" → "THUNDER WAVE"
fn move_display_name(move_id: MoveId) -> String {
    let raw = format!("{:?}", move_id);
    let mut result = String::with_capacity(raw.len() + 4);
    for (i, c) in raw.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            // Insert space before uppercase letter unless previous char was also uppercase
            let prev = raw.as_bytes()[i - 1] as char;
            if prev.is_lowercase() {
                result.push(' ');
            }
        }
        result.push(c);
    }
    result.to_uppercase()
}

/// True when the mon is locked into re-issuing its previously-selected move (charge
/// mid-flight, rampage) — the menu is ignored and the continuation re-uses
/// `selected_move`. Mirrors `PokeredRules::forced_action`; extended as Bide / trapping
/// / Rage land.
fn move_is_locked(p: &state::BattlerState) -> bool {
    use crate::battle::state::{status1, status2};
    p.has_status1(status1::CHARGING_UP)
        || p.has_status1(status1::THRASHING_ABOUT)
        || p.has_status1(status1::USING_TRAPPING_MOVE)
        || p.has_status1(status1::STORING_ENERGY)
        || p.has_status2(status2::USING_RAGE)
}

/// The battler's last-used move IFF it still occupies a move slot with PP > 0 — the
/// exact guard `apply_disable` applies (`moves[i] == last && pp[i] > 0`), else
/// `MoveId::None`. Disable's `LAST_MOVE_LIVE` is primed with THIS (not the raw
/// `last_move_used`), so `disable_install` fails to a no-op on an out-of-PP last move
/// just like the oracle — the engine battler carries no PP, so the guard lives here
/// where the legacy party PP is reachable.
fn disable_target_last_move(b: &state::BattlerState) -> MoveId {
    let last = b.last_move_used;
    if last == MoveId::None {
        return MoveId::None;
    }
    let mon = b.active_mon();
    for i in 0..mon.moves.len().min(mon.pp.len()) {
        if mon.moves[i] == last && mon.pp[i] > 0 {
            return last;
        }
    }
    MoveId::None
}

/// The move Metronome picks, REJECTION-SAMPLED from the rng stream exactly like
/// the original `MetronomePickMove` (core.asm:5013-5037): draw `BattleRandom`
/// until it is a valid move id — reject 0, reject ≥ STRUGGLE (0xA5; ids above
/// are not moves), and reject METRONOME (0x76) itself. Uniform over the 163
/// legal ids (the old `(byte % 163) + 1` oversampled low ids ~2×). The retry is
/// BOUNDED so a degenerate rng can't spin forever; on exhaustion it falls back
/// to id 1 (still a legal, non-Metronome move).
fn metronome_pick(rng: &mut dyn jrpg_engine::battle::rng::BattleRng) -> MoveId {
    const METRONOME: u8 = 0x76;
    const STRUGGLE: u8 = 0xA5;
    let mut byte = rng.next_u8();
    let mut tries = 0;
    while (byte == 0 || byte >= STRUGGLE || byte == METRONOME) && tries < 64 {
        byte = rng.next_u8();
        tries += 1;
    }
    let move_val = if byte == 0 || byte >= STRUGGLE || byte == METRONOME {
        1
    } else {
        byte
    };
    pokered_data::moves::move_id_from_u8(move_val)
}

/// Resolve a "call another move" effect (Metronome / Mirror Move) into the move that
/// actually executes, following nested calls (Metronome→Mirror Move→…) up to a small
/// bound. Returns `(resolved_move, narration_label, failed)`; `failed` is true only
/// for Mirror Move with no foe last move → the caller resolves `BattleAction::Nothing`.
/// This flattens Gen-1's "call another move" into a pre-driver substitution.
/// The Metronome pick draws from `rng` — in a link battle this must be the
/// shared stream so both sides pick the same move (`BattleRandom`).
fn resolve_called_move(
    id: MoveId,
    foe_last_move: MoveId,
    rng: &mut dyn jrpg_engine::battle::rng::BattleRng,
) -> (MoveId, Option<&'static str>, bool) {
    use pokered_data::moves::MoveEffect;
    let mut cur = id;
    let mut label = None;
    for _ in 0..4 {
        match MoveData::get(cur).map(|m| m.effect) {
            Some(MoveEffect::MetronomeEffect) => {
                label = Some("METRONOME");
                cur = metronome_pick(rng);
            }
            Some(MoveEffect::MirrorMoveEffect) => {
                label = Some("MIRROR MOVE");
                if foe_last_move == MoveId::None {
                    return (cur, label, true);
                }
                cur = foe_last_move;
            }
            _ => break,
        }
    }
    (cur, label, false)
}

/// The Gen-1 charge-turn narration for a two-turn move ("MON flew up high!" etc.).
fn charge_message(move_id: MoveId) -> &'static str {
    match move_id {
        MoveId::Fly => "flew up high!",
        MoveId::Dig => "dug a hole!",
        MoveId::Solarbeam => "took in sunlight!",
        MoveId::RazorWind => "made a whirlwind!",
        MoveId::SkullBash => "lowered its head!",
        MoveId::SkyAttack => "is glowing!",
        _ => "began charging!",
    }
}

fn format_cannot_move(reason: &CannotMoveReason) -> &'static str {
    match reason {
        CannotMoveReason::Asleep => "is fast asleep!",
        CannotMoveReason::WokeUpButLostTurn => "woke up!",
        CannotMoveReason::Frozen => "is frozen solid!",
        CannotMoveReason::TrappedByEnemy => "can't move!",
        CannotMoveReason::Flinched => "flinched!",
        CannotMoveReason::MustRecharge => "must recharge!",
        CannotMoveReason::ConfusedSelfHit => "hurt itself in confusion!",
        CannotMoveReason::MoveDisabled => "is disabled!",
        CannotMoveReason::FullyParalyzed => "is fully paralyzed!",
    }
}

/// `HandleSelfConfusionDamage` (core.asm:3672) for the disobedience "won't
/// obey!" outcome: a typeless 40-power physical self-hit using the mon's own
/// Attack vs own Defense — no crit, always hits, max roll. Mirrors the stack
/// engine's confusion self-hit (`confusion_self_hit_damage`), including its
/// stage-0 convention; the stats are the badge-boosted working copy
/// (`wBattleMon*`) when present.
fn disobedience_self_hit_damage(bs: &BattleState) -> u16 {
    use crate::battle::damage::{calculate_damage, DamageParams};
    use pokered_data::types::PokemonType;
    let mon = bs.player.active_mon();
    let (atk, def) = match bs.player.badge_boosted_stats {
        Some(b) => (b[0], b[1].max(1)),
        None => (mon.attack, mon.defense.max(1)),
    };
    let params = DamageParams {
        attacker_level: mon.level,
        move_power: 40,
        move_type: PokemonType::Normal,
        move_id: MoveId::None,
        attack_stat: atk,
        defense_stat: def,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Normal,
        attacker_type2: PokemonType::Normal,
        defender_type1: PokemonType::Normal,
        defender_type2: PokemonType::Normal,
        is_critical: false,
        random_value: 255,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };
    calculate_damage(&params).damage
}

/// HP-bar drain/refill animation state (original `engine/gfx/hp_bar.asm`
/// `UpdateHPBar` / `UpdateHPBar_AnimateHPBar`, hp_bar.asm:48-163).
///
/// In Gen 1 the HUD HP bar does not jump to the new value: the engine loops,
/// stepping HP and re-drawing the bar one *pixel* per **2 frames**
/// (`UpdateHPBar_AnimateHPBar`: "animates the HP bar going up or down for (a)
/// ticks (two waiting frames each)", hp_bar.asm:140-163). The bar is 48 px
/// (`; 48 * bc (hp bar is 48 pixels long)`, hp_bar.asm:15-16). The original
/// runs this synchronously and *silently* (hp_bar.asm contains no SFX
/// reference — the audible feedback is the move animation's own SFX).
///
/// Here the animation is asynchronous: `BattleScreen.player_hp` / `enemy_hp`
/// are the *displayed* values, tweening toward the real HP at the original
/// rate, and text/message advancement waits for the tween to finish (see the
/// `ShowingText` arm of `update_frame`). Bar color thresholds
/// (`GetHealthBarColor`, home/palettes.asm:44-57: green ≥ 27 px, yellow
/// ≥ 10 px) are computed downstream from the displayed HP, so they follow the
/// animation automatically — as does `low_health_alarm` (the original's alarm
/// is likewise driven by the *displayed* bar color, core.asm:1860-1873).
#[derive(Debug, Clone, Copy, Default)]
pub struct HpBarAnim {
    player: HpBarSide,
    enemy: HpBarSide,
    /// 2-frame divider: the bar steps on every other frame.
    tick: bool,
    /// Set when a *drain* (HP loss) begins. The frontend polls
    /// [`BattleScreen::take_hp_drain_sfx_pending`] and plays `SfxId::Damage`.
    /// (Deviation: the Gen-1 drain is silent; the task spec asks for the
    /// damage SFX, fired once per drain — not per frame.)
    drain_sfx_pending: bool,
}

#[derive(Debug, Clone, Copy)]
struct HpBarSide {
    /// Real HP the bar is tweening toward.
    target: u16,
    max_hp: u16,
    species: Species,
    /// Snapped at least once (battle start / send-out): no animation.
    initialized: bool,
    /// Tween in progress.
    active: bool,
}

impl Default for HpBarSide {
    fn default() -> Self {
        Self {
            target: 0,
            max_hp: 0,
            species: Species::None,
            initialized: false,
            active: false,
        }
    }
}

impl HpBarAnim {
    /// HP points per animation step ≈ 1 bar pixel: `round(max_hp / 48)`,
    /// min 1 (bar = 48 px, hp_bar.asm:15-16). For `max_hp <= 48` this is
    /// exactly 1 HP = 1 px per 2 frames, matching the original; for higher
    /// max HP the original also moves ~1 px per 2 frames (it races through
    /// the HP points between pixel boundaries with no delay on the enemy
    /// side).
    fn step_size(max_hp: u16) -> u16 {
        ((max_hp as u32 + 24) / 48).max(1) as u16
    }

    /// True while either side's bar is still draining/refilling.
    pub fn is_active(&self) -> bool {
        self.player.active || self.enemy.active
    }

    /// Register the real HP for one side. Snaps instantly (no animation) on
    /// first sync or when a different mon is shown (species/max-HP change —
    /// e.g. send-out after a faint); otherwise starts a tween.
    fn set_target(&mut self, side: BattleSide, hp: u16, max_hp: u16, species: Species, display: &mut u16) {
        let s = match side {
            BattleSide::Player => &mut self.player,
            BattleSide::Enemy => &mut self.enemy,
        };
        if !s.initialized || s.species != species || s.max_hp != max_hp {
            // Battle start / new mon sent out: original draws the full bar
            // immediately (`DrawHUDsAndHPBars`), no drain.
            *display = hp;
            s.target = hp;
            s.max_hp = max_hp;
            s.species = species;
            s.initialized = true;
            s.active = false;
            return;
        }
        if hp != s.target {
            if hp < *display {
                self.drain_sfx_pending = true;
            }
            s.target = hp;
        }
        s.active = *display != s.target;
    }

    /// Advance the tween by one frame (1 px-step per 2 frames).
    fn tick(&mut self, player_display: &mut u16, enemy_display: &mut u16) {
        self.tick = !self.tick;
        if !self.tick {
            return;
        }
        for (s, display) in [
            (&mut self.player, player_display),
            (&mut self.enemy, enemy_display),
        ] {
            if !s.active {
                continue;
            }
            let step = Self::step_size(s.max_hp);
            if *display < s.target {
                *display = (*display + step).min(s.target);
            } else {
                *display = display.saturating_sub(step).max(s.target);
            }
            if *display == s.target {
                s.active = false;
            }
        }
    }
}

/// Which side of the battle an HP bar belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BattleSide {
    Player,
    Enemy,
}

/// Result of a thrown ball, mirroring the original `wPokeBallAnimData`
/// values (engine/items/item_effects.asm `ItemUseBall`):
/// `$43` caught / `$20` missed (0 shakes) / `$61`-`$63` broke free after
/// N shakes / `$10` ghost dodge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BallAnimOutcome {
    /// `$43`: caught — toss, poof, hide mon pic, 3 shakes.
    Caught,
    /// `$20` (0 shakes: the ball misses) or `$61`-`$63`: the mon breaks
    /// free after N shakes — the ball reopens (poof) and the mon reappears
    /// (SHOWPIC_ANIM).
    BrokeFree,
    /// `$10`: an unidentified GHOST dodges the ball — toss only.
    Dodged,
}

/// Non-move battle animation requests (data/moves/animations.asm ids $A6+),
/// queued by core flows for the frontend to stage through its battle
/// animation player. Core is I/O-free, so the frontend drains these each
/// frame via [`BattleScreen::take_anim_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleAnimEvent {
    /// A ball was thrown at the wild mon (`ItemUseBall` → `TossBallAnimation`,
    /// engine/battle/animations.asm:2581). Carries the computed shake count
    /// (`wNumShakes`) and outcome so the frontend can stage
    /// toss → poof → hide-pic → shakes (→ poof → show-pic on breakout).
    Ball {
        ball: ItemId,
        /// `wNumShakes` (0-3); 0 = the ball missed entirely. Always 3 for
        /// [`BallAnimOutcome::Caught`] (the original's `$43`).
        shakes: u8,
        outcome: BallAnimOutcome,
    },
    /// XSTATITEM_ANIM on the player's mon — a successful X-stat item use
    /// (`ItemUseXStat` → `StatModifierUpEffect`, engine/items/item_effects.asm:1657).
    XStatItem,
}

#[derive(Clone)]
pub struct BattleScreen {
    pub phase: BattlePhase,
    pub battle_menu: BattleMenuState,
    pub party_submenu: Option<PartySubMenuState>,
    pub bag_menu: Option<BagMenuState>,
    pub is_wild: bool,
    pub trainer_class: Option<TrainerClass>,
    pub trainer_name: Option<String>,
    pub player_bag: Inventory,

    // Display fields (synced from battle_state after every action)
    pub enemy_species: Species,
    pub enemy_level: u8,
    pub enemy_hp: u16,
    pub enemy_max_hp: u16,
    pub enemy_status: StatusCondition,
    pub player_species: Species,
    pub player_level: u8,
    pub player_hp: u16,
    pub player_max_hp: u16,
    pub player_status: StatusCondition,
    pub player_party_size: usize,
    pub enemy_party_size: usize,
    pub player_pokeball_status: [PokeballSlotStatus; 6],
    pub enemy_pokeball_status: [PokeballSlotStatus; 6],
    pub show_player_pokeballs: bool,
    pub show_enemy_pokeballs: bool,

    // Real battle engine state
    pub battle_state: Option<BattleState>,
    pub move_menu: Option<MoveMenuState>,
    pub current_message: Option<String>,
    pub party_cursor: usize,
    /// Settlement result computed when battle ends (money, evolutions, etc.)
    pub settlement: Option<BattleSettlement>,
    /// Player's money at battle start (used for blackout penalty)
    pub player_money: u32,
    /// NPC index of the trainer being battled (for marking defeated after win)
    pub trainer_npc_index: Option<u8>,
    /// The trainer's one-shot victory quip, shown after the "defeated!" line
    /// and before the prize-money text (original `PrintEndBattleText`). Set for
    /// sight/talk trainer battles that carry converted `endBattleText`.
    pub end_battle_text: Option<String>,
    /// A wild Pokémon that was successfully caught this battle, captured with
    /// its current battle state (HP/status/level/moves). The app layer moves it
    /// into the party (or a PC box if full) and registers it in the Pokédex.
    pub captured_mon: Option<state::Pokemon>,
    /// Current map ID (for dungeon transition detection)
    pub map_id: u8,
    /// Selected battle transition type
    pub battle_transition: BattleTransition,
    /// Remaining trainer-AI action budget for the active enemy mon (Gen-1 `wAICount`).
    /// Seeded per trainer class on send-out; decremented when the AI uses an item /
    /// X-item / Guard Spec / switch; 0 for wild battles and generic AI (no-op).
    pub enemy_ai_count: u8,
    /// A Pokémon-Tower wild encounter met WITHOUT the Silph Scope: shown as an
    /// unidentified "GHOST" (name + sprite override) and uncatchable, until the Scope
    /// reveals it. Set by the app at battle start; default `false`.
    pub is_ghost: bool,
    /// The scripted RESTLESS_SOUL (Marowak) battle on Pokémon Tower 6F fought
    /// WITH the Silph Scope (original: `cp RESTLESS_SOUL` in
    /// engine/battle/common_text.asm's PrintBeginningBattleText, where
    /// `RESTLESS_SOUL EQU MAROWAK`): the intro plays the "SILPH SCOPE unveiled
    /// the GHOST's identity!" text + the MarowakAnim reveal, then the battle
    /// proceeds as a normal Marowak fight. Set by the app at battle start;
    /// default `false`. Mutually exclusive with [`is_ghost`](Self::is_ghost).
    pub ghost_marowak_reveal: bool,
    /// Internal intro bookkeeping for [`ghost_marowak_reveal`](Self::ghost_marowak_reveal):
    /// set once the GhostUnveil intro phase completes, so the second WildReveal
    /// shows "Wild MAROWAK appeared!" (and the Marowak sprite) instead of the
    /// ghost text/sprite. Default `false`.
    pub ghost_marowak_unveiled: bool,
    /// A Safari Zone encounter: the FIGHT menu is replaced by BALL/BAIT/ROCK/RUN and no
    /// attacking happens. Set by the app when a wild battle starts in the Safari Zone
    /// during an active Safari Game. Default `false`.
    pub is_safari: bool,
    /// Live Safari mechanics (catch-rate + bait/anger counters + ball count) when
    /// [`is_safari`](Self::is_safari); `None` for a normal battle.
    pub safari: Option<safari::SafariState>,
    /// The BALL/BAIT/ROCK/RUN cursor (used only in Safari battles).
    pub safari_menu: menu::SafariBattleMenuState,
    /// The Viridian Old-Man CATCH TUTORIAL (Gen-1 `BATTLE_TYPE_OLD_MAN`): the player is
    /// shown as "OLD MAN" and the battle AUTO-PLAYS a scripted, guaranteed catch of the
    /// wild WEEDLE (which is a demo — not kept). Default `false`.
    pub is_old_man: bool,
    /// A fishing-rod encounter (Gen-1 `wMoveMissed = 1`, set by `RodResponse` on a
    /// bite — item_effects.asm:1872-1873): `PrintBeginningBattleText`
    /// (engine/battle/common_text.asm:13-18) shows "The hooked X attacked!"
    /// (HookedMonAttackedText) instead of "Wild X appeared!". Set by the app
    /// when the wild battle came from a rod bite; default `false`.
    pub hooked: bool,
    /// Set when the POKé FLUTE wakes at least one sleeping Pokémon: the original
    /// plays `Music_PokeFluteInBattle` only when `wWereAnyMonsAsleep` != 0
    /// (engine/items/item_effects.asm:1728-1739). Core is I/O-free, so the app
    /// layer polls this with [`Self::take_poke_flute_sfx_pending`] and plays
    /// `AudioManager::play_flute_in_battle`.
    pub poke_flute_sfx_pending: bool,
    /// Pending non-move battle animation requests (ball throws, X-stat
    /// items — see [`BattleAnimEvent`]). Queued by the ball / X-stat flows;
    /// drained by the frontend every frame via [`Self::take_anim_event`]
    /// (same polling pattern as [`Self::poke_flute_sfx_pending`]).
    pub pending_anim_events: std::collections::VecDeque<BattleAnimEvent>,
    /// HP-bar drain/refill animation (original `UpdateHPBar`,
    /// engine/gfx/hp_bar.asm): [`Self::player_hp`]/[`Self::enemy_hp`] are the
    /// *displayed* values; this tracks the real-HP targets and steps the
    /// display 1 bar pixel per 2 frames while message advancement waits.
    pub hp_bar_anim: HpBarAnim,
    /// `wOptions` BIT_BATTLE_SHIFT (inverted): `Shift` prompts "Will you change
    /// #MON?" before an enemy trainer sends out their next mon after a faint;
    /// `Set` never prompts. Pushed from the game config by the frontend every
    /// frame (like `battle_vfx.animations_enabled`). Default `Shift`.
    pub battle_style: BattleStyle,
    /// Player's name, used by the SHIFT prompt text ("Will <PLAYER> change
    /// #MON?" — `TrainerAboutToUseText`). Set by the frontend at battle start;
    /// falls back to "you" when unset.
    pub player_name: Option<String>,
    /// YES/NO cursor for [`BattlePhase::ShiftPrompt`] (`true` = YES). Reset to
    /// `false` (NO) each time the prompt opens, matching the original's
    /// `ld a, 1; ld [wCurrentMenuItem], a` (cursor on NO).
    pub shift_prompt_yes: bool,
    /// Party index chosen in [`BattlePhase::ShiftSwitchSelect`], applied after
    /// the enemy's next mon has been sent out (original `ReplaceFaintedEnemyMon`
    /// sends the enemy out first, then runs `SwitchPlayerMon`).
    pub pending_shift_switch: Option<usize>,
    /// `wObtainedBadges` — drives the badge stat boosts
    /// ([`badge_boosts`](crate::battle::badge_boosts)) and the traded-mon
    /// obedience thresholds ([`obedience`](crate::battle::obedience)). Set by
    /// the frontend at battle start (synced into `battle_state` each turn).
    /// Default `0` (no badges).
    pub player_badges: u8,
    /// `wPlayerID` — the player's trainer ID: traded-mon obedience compares it
    /// against a party mon's `ot_id`, and a caught wild mon is stamped with it.
    /// Default `0`.
    pub player_id: u16,

    // ── Link-battle mode (driven by `crate::battle::link_battle_driver`) ──
    /// When set, the screen defers each turn's execution until both sides'
    /// actions have been exchanged (the driver sends the local action and
    /// calls [`Self::resolve_link_turn`] with the remote's), draws every
    /// battle random byte from the shared [`LinkRng`] stream (`BattleRandom`,
    /// engine/battle/core.asm:6543), never consults the trainer AI (remote
    /// actions come from the wire — `TrainerAI` returns early for
    /// `LINK_STATE_BATTLING`, engine/battle/trainer_ai.asm:296-298), and
    /// blocks the bag ("Items can't be used here.",
    /// engine/battle/core.asm:2171-2179).
    pub link_mode: bool,
    /// Shared link-battle RNG stream (the host's random-number list, consumed
    /// by both sides). `None` outside link battles → the normal local RNG.
    pub link_rng: Option<crate::link::rng::LinkRng>,
    /// The local player's action chosen via the battle menus, awaiting the
    /// remote action. Set when `link_mode` and a menu action was picked; the
    /// driver sends it over the wire and calls [`Self::resolve_link_turn`]
    /// once the remote action arrives.
    pub link_pending_local_action: Option<crate::link::protocol::LinkAction>,
    /// Link-mode enemy-move override: the remote player's raw wire move index
    /// replaces the AI pick for the current turn (set by `resolve_link_turn`
    /// before re-entering `execute_turn_with_move`).
    pub link_enemy_move_override: Option<(MoveId, u8)>,
    /// Link-mode: the remote's action was a switch / no action — the enemy
    /// skips its move this turn (mirrors `enemy_ai_fired`'s Nothing action).
    pub link_enemy_skips_turn: bool,
    /// Link-mode: narration lines (e.g. the remote's switch "sent out" text)
    /// prepended to the next `execute_turn_with_move`'s message list.
    pub link_turn_prefix_msgs: Vec<String>,
    /// Link-battle end result, set once the battle is over in link mode (the
    /// local outcome of the shared resolution). The driver consumes it to
    /// announce `BattleResult` to the peer and drive the result screen.
    pub link_result: Option<crate::link::protocol::LinkBattleResult>,
}

/// Derive pokeball indicator status from each party member's HP and status,
/// matching the original PickPokeball logic in draw_hud_pokeball_gfx.asm:
/// - Empty slot (beyond party count) → Empty
/// - HP == 0 → Fainted (crossed ball)
/// - Non-volatile status active → StatusAilment (black ball)
/// - Otherwise → Normal (filled ball)
fn derive_pokeball_status(party: &[state::Pokemon]) -> [PokeballSlotStatus; 6] {
    let mut result = [PokeballSlotStatus::Empty; 6];
    for (i, mon) in party.iter().enumerate().take(6) {
        result[i] = if mon.hp == 0 {
            PokeballSlotStatus::Fainted
        } else if !mon.status.is_none() {
            PokeballSlotStatus::StatusAilment
        } else {
            PokeballSlotStatus::Normal
        };
    }
    result
}

/// Battle-theme classification, matching the original `PlayBattleMusic`
/// (audio/play_battle_music.asm): the gym-leader theme plays for the 8 gym
/// leaders (their gym scripts set `wGymLeaderNo` != 0 right before the battle;
/// it is zeroed on every map entry by `ClearVariablesOnEnterMap`) and for
/// LANCE, who is special-cased onto the gym-leader theme (`cp OPP_LANCE`).
/// LORELEI/BRUNO/AGATHA are NOT here: their rooms never set `wGymLeaderNo`,
/// so they keep the normal trainer theme. The champion (RIVAL3) is handled
/// separately in `battle_music_id` (MUSIC_FINAL_BATTLE).
/// Note: GIOVANNI also appears as the Rocket boss, where the original plays
/// the trainer theme (`wGymLeaderNo` == 0 outside Viridian Gym); class-based
/// classification cannot distinguish the two, so we follow his gym battle.
fn is_gym_leader_battle_theme(tc: Option<TrainerClass>) -> bool {
    matches!(
        tc,
        Some(
            TrainerClass::Brock
                | TrainerClass::Misty
                | TrainerClass::LtSurge
                | TrainerClass::Erika
                | TrainerClass::Koga
                | TrainerClass::Blaine
                | TrainerClass::Sabrina
                | TrainerClass::Giovanni
                | TrainerClass::Lance
        )
    )
}

/// Victory-fanfare classification, matching the original `TrainerBattleVictory`
/// (engine/battle/core.asm): the gym-leader fanfare plays when
/// `wGymLeaderNo` != 0 (the 8 gym leaders) and for the RIVAL3 champion battle
/// (`cp RIVAL3` — which also sets BIT_NO_MAP_MUSIC). Lance is NOT
/// special-cased there, so he keeps the normal defeated-trainer fanfare.
fn is_gym_leader_victory_theme(tc: Option<TrainerClass>) -> bool {
    matches!(
        tc,
        Some(
            TrainerClass::Brock
                | TrainerClass::Misty
                | TrainerClass::LtSurge
                | TrainerClass::Erika
                | TrainerClass::Koga
                | TrainerClass::Blaine
                | TrainerClass::Sabrina
                | TrainerClass::Giovanni
                | TrainerClass::Rival3
        )
    )
}

impl BattleScreen {
    pub fn new(is_wild: bool) -> Self {
        let transition = BattleTransition::default();
        Self {
            phase: intro_start_phase(transition),
            battle_menu: BattleMenuState::new(),
            party_submenu: None,
            bag_menu: None,
            is_wild,
            trainer_class: None,
            trainer_name: None,
            player_bag: Inventory::new_bag(),
            enemy_species: Species::Pikachu,
            enemy_level: 25,
            enemy_hp: 55,
            enemy_max_hp: 55,
            enemy_status: StatusCondition::None,
            player_species: Species::Charmander,
            player_level: 5,
            player_hp: 19,
            player_max_hp: 20,
            player_status: StatusCondition::None,
            player_party_size: 1,
            enemy_party_size: 1,
            player_pokeball_status: [PokeballSlotStatus::Normal; 6],
            enemy_pokeball_status: [PokeballSlotStatus::Empty; 6],
            show_player_pokeballs: false,
            show_enemy_pokeballs: false,
            battle_state: None,
            move_menu: None,
            current_message: None,
            party_cursor: 0,
            settlement: None,
            player_money: 0,
            trainer_npc_index: None,
            end_battle_text: None,
            captured_mon: None,
            map_id: 0,
            battle_transition: transition,
            enemy_ai_count: 0,
            is_ghost: false,
            ghost_marowak_reveal: false,
            ghost_marowak_unveiled: false,
            is_safari: false,
            safari: None,
            safari_menu: menu::SafariBattleMenuState::new(0),
            is_old_man: false,
            hooked: false,
            poke_flute_sfx_pending: false,
            pending_anim_events: std::collections::VecDeque::new(),
            hp_bar_anim: HpBarAnim::default(),
            battle_style: BattleStyle::Shift,
            player_name: None,
            shift_prompt_yes: false,
            pending_shift_switch: None,
            player_badges: 0,
            player_id: 0,
            link_mode: false,
            link_rng: None,
            link_pending_local_action: None,
            link_enemy_move_override: None,
            link_enemy_skips_turn: false,
            link_turn_prefix_msgs: Vec::new(),
            link_result: None,
        }
    }

    pub fn from_parties(
        is_wild: bool,
        player_party: &[state::Pokemon],
        enemy_party: &[state::Pokemon],
        trainer_class: Option<TrainerClass>,
    ) -> Self {
        let player = &player_party[0];
        let enemy = &enemy_party[0];
        let battle_type = if is_wild {
            BattleType::Wild
        } else {
            BattleType::Trainer
        };
        let mut bs =
            state::new_battle_state(battle_type, player_party.to_vec(), enemy_party.to_vec());
        bs.party_gain_exp_flags[0] = true;

        let player_first_alive_level = player_party
            .iter()
            .find(|m| m.hp > 0)
            .map(|m| m.level)
            .unwrap_or(player.level);
        let transition = BattleTransition::select(
            !is_wild,
            enemy.level,
            player_first_alive_level,
            0,
        );

        Self {
            phase: intro_start_phase(transition),
            battle_menu: BattleMenuState::new(),
            party_submenu: None,
            bag_menu: None,
            is_wild,
            trainer_class,
            trainer_name: trainer_class.map(|tc| tc.display_name().to_string()),
            player_bag: Inventory::new_bag(),
            enemy_species: enemy.species,
            enemy_level: enemy.level,
            enemy_hp: enemy.hp,
            enemy_max_hp: enemy.max_hp,
            enemy_status: enemy.status,
            player_species: player.species,
            player_level: player.level,
            player_hp: player.hp,
            player_max_hp: player.max_hp,
            player_status: player.status,
            player_party_size: player_party.len(),
            enemy_party_size: enemy_party.len(),
            player_pokeball_status: derive_pokeball_status(player_party),
            enemy_pokeball_status: derive_pokeball_status(enemy_party),
            show_player_pokeballs: false,
            show_enemy_pokeballs: false,
            battle_state: Some(bs),
            move_menu: None,
            current_message: None,
            party_cursor: 0,
            settlement: None,
            player_money: 0,
            trainer_npc_index: None,
            end_battle_text: None,
            captured_mon: None,
            map_id: 0,
            battle_transition: transition,
            enemy_ai_count: trainer_class
                .map(|tc| trainer_ai_config(tc).ai_count)
                .unwrap_or(0),
            is_ghost: false,
            ghost_marowak_reveal: false,
            ghost_marowak_unveiled: false,
            is_safari: false,
            safari: None,
            safari_menu: menu::SafariBattleMenuState::new(0),
            is_old_man: false,
            hooked: false,
            poke_flute_sfx_pending: false,
            pending_anim_events: std::collections::VecDeque::new(),
            hp_bar_anim: HpBarAnim::default(),
            battle_style: BattleStyle::Shift,
            player_name: None,
            shift_prompt_yes: false,
            pending_shift_switch: None,
            player_badges: 0,
            player_id: 0,
            link_mode: false,
            link_rng: None,
            link_pending_local_action: None,
            link_enemy_move_override: None,
            link_enemy_skips_turn: false,
            link_turn_prefix_msgs: Vec::new(),
            link_result: None,
        }
    }

    /// Push the frontend-supplied player context (badges / trainer ID) into the
    /// battle state, and make sure the active mon's badge-boosted working stats
    /// exist — the send-out `ApplyBadgeStatBoosts` (core.asm:1659), applied
    /// lazily so badges assigned after battle construction still count.
    fn sync_player_context(&mut self) {
        if let Some(ref mut bs) = self.battle_state {
            bs.player_badges = self.player_badges;
            bs.player_id = self.player_id;
            crate::battle::badge_boosts::ensure_initialized(&mut bs.player, self.player_badges);
        }
    }

    pub fn set_map_id(&mut self, map_id: u8) {
        self.map_id = map_id;
        self.battle_transition = BattleTransition::select(
            !self.is_wild,
            self.enemy_level,
            self.player_level,
            map_id,
        );
        // Re-point the (not yet started) intro at the newly selected
        // transition — including the FlashScreen strobe that precedes the
        // Circle/DoubleCircle wipes.
        if matches!(
            self.phase,
            BattlePhase::Intro {
                phase: IntroPhase::BattleTransitionWipe(_) | IntroPhase::TransitionFlash,
                ..
            }
        ) {
            self.phase = intro_start_phase(self.battle_transition);
        }
    }

    pub fn with_bag(mut self, bag: Inventory) -> Self {
        self.player_bag = bag;
        self
    }

    pub fn battle_music_id(&self) -> u8 {
        use pokered_data::music::MusicId;
        if self.is_wild {
            MusicId::WildBattle as u8
        } else if self.trainer_class == Some(TrainerClass::Rival3) {
            // audio/play_battle_music.asm: OPP_RIVAL3 (the champion) → MUSIC_FINAL_BATTLE
            MusicId::FinalBattle as u8
        } else if is_gym_leader_battle_theme(self.trainer_class) {
            MusicId::GymLeaderBattle as u8
        } else {
            MusicId::TrainerBattle as u8
        }
    }

    pub fn victory_music_id(&self) -> u8 {
        use pokered_data::music::MusicId;
        if self.is_wild {
            MusicId::DefeatedWildMon as u8
        } else if is_gym_leader_victory_theme(self.trainer_class) {
            MusicId::DefeatedGymLeader as u8
        } else {
            MusicId::DefeatedTrainer as u8
        }
    }

    pub fn is_victory_phase(&self) -> bool {
        matches!(
            self.phase,
            BattlePhase::TrainerVictory {
                phase: VictoryPhase::DefeatedText,
                ..
            } | BattlePhase::BattleOver { won: true, .. }
        )
    }

    /// Take the pending in-battle POKé FLUTE jingle request (see
    /// [`Self::poke_flute_sfx_pending`]). Returns `true` at most once per use.
    pub fn take_poke_flute_sfx_pending(&mut self) -> bool {
        core::mem::take(&mut self.poke_flute_sfx_pending)
    }

    /// Pop the oldest pending non-move battle animation request (see
    /// [`Self::pending_anim_events`]). The frontend drains this every frame
    /// and stages the matching animation sequence (ball toss/shake/poof
    /// choreography, X-stat spiral, …).
    pub fn take_anim_event(&mut self) -> Option<BattleAnimEvent> {
        self.pending_anim_events.pop_front()
    }

    /// Take the pending HP-bar *drain* notification (see
    /// [`HpBarAnim::drain_sfx_pending`]). Returns `true` at most once per
    /// drain. The frontend plays `SfxId::Damage` on it.
    pub fn take_hp_drain_sfx_pending(&mut self) -> bool {
        core::mem::take(&mut self.hp_bar_anim.drain_sfx_pending)
    }

    /// Low-health alarm flag (`wLowHealthAlarm` bit 7), replicating the
    /// enable check in the original's player-HUD redraw
    /// (`DrawPlayerHUDAndHPBar`, engine/battle/core.asm:1851-1875): the
    /// alarm sounds while the active player mon is alive and its HP bar
    /// is red. Red = bar under 10 of 48 pixels (`GetHealthBarColor`,
    /// home/palettes.asm:44-57), with the pixel count `HP * 48 / maxHP`,
    /// minimum 1 (`HPBarLength`, engine/gfx/hp_bar.asm:4-6). Fainting,
    /// healing above the threshold, or switching to a healthier mon turns
    /// it off — the same HUD redraw re-evaluates it in the original
    /// (heal-item path: engine/items/item_effects.asm:993).
    pub fn low_health_alarm(&self) -> bool {
        // `EndLowHealthAlarm` (core.asm:864-872): once the battle is
        // decided the alarm is killed and `wLowHealthAlarmDisabled`
        // blocks reactivation; battle end clears it outright
        // (engine/battle/end_of_battle.asm:48). Gen-1 wins always come
        // from wiping the enemy party (wild: core.asm:793; trainer:
        // core.asm:916); the battle-over phases cover escape/loss/catch.
        if self.enemy_party_wiped()
            || matches!(
                self.phase,
                BattlePhase::TrainerVictory { .. } | BattlePhase::BattleOver { .. }
            )
        {
            return false;
        }
        if self.player_hp == 0 || self.player_max_hp == 0 {
            return false;
        }
        let pixels = (self.player_hp as u32 * 48 / self.player_max_hp as u32).max(1);
        pixels < 10
    }

    /// All enemy party members fainted — the point where the original
    /// calls `EndLowHealthAlarm`.
    fn enemy_party_wiped(&self) -> bool {
        self.battle_state
            .as_ref()
            .is_some_and(|bs| bs.enemy.party.iter().all(|p| p.hp == 0))
    }

    fn sync_display_from_state(&mut self) {
        if let Some(ref bs) = self.battle_state {
            let p = bs.player.active_mon();
            self.player_species = p.species;
            self.player_level = p.level;
            self.hp_bar_anim.set_target(
                BattleSide::Player,
                p.hp,
                p.max_hp,
                p.species,
                &mut self.player_hp,
            );
            self.player_max_hp = p.max_hp;
            self.player_status = p.status;
            self.player_party_size = bs.player.party.len();
            self.player_pokeball_status = derive_pokeball_status(&bs.player.party);

            let e = bs.enemy.active_mon();
            self.enemy_species = e.species;
            self.enemy_level = e.level;
            self.hp_bar_anim.set_target(
                BattleSide::Enemy,
                e.hp,
                e.max_hp,
                e.species,
                &mut self.enemy_hp,
            );
            self.enemy_max_hp = e.max_hp;
            self.enemy_status = e.status;
            self.enemy_party_size = bs.enemy.party.len();
            self.enemy_pokeball_status = derive_pokeball_status(&bs.enemy.party);
        }
    }

    fn generate_move_randoms() -> MoveRandoms {
        MoveRandoms {
            confusion_roll: rand::random(),
            paralysis_roll: rand::random(),
            crit_roll: rand::random(),
            accuracy_roll: rand::random(),
            damage_roll: rand::random(),
            effect_randoms: EffectRandoms {
                side_effect_roll: rand::random(),
                duration_roll: rand::random(),
                multi_hit_roll: rand::random(),
            },
        }
    }

    fn pick_enemy_move(bs: &BattleState, trainer_class: Option<TrainerClass>) -> (MoveId, u8) {
        let mon = bs.enemy.active_mon();
        let available: Vec<(MoveId, u8)> = mon
            .moves
            .iter()
            .enumerate()
            .filter(|(i, m)| **m != MoveId::None && mon.pp[*i] > 0)
            .map(|(i, m)| (*m, i as u8))
            .collect();
        if available.is_empty() {
            return (MoveId::Struggle, 0);
        }

        if let Some(tc) = trainer_class {
            let layers = move_choice_layers(tc);
            if !layers.is_empty() {
                // Gen-1 FAITHFUL: `wAILayer2Encouragement` is provably never set to 1
                // in the original — its only writer (`ReplaceFaintedEnemyMon`) does
                // `xor a; ld [wAILayer2Encouragement], a` and its WRAM default is 0, so
                // `AIMoveChoiceModification2` (`cp $1 / ret nz`) is dead code. Passing 0
                // reproduces that: Layer2 classes effectively behave as Layer1(+Layer3).
                // Do NOT flip this to 1 — it would DIVERGE from Gen-1 (a "smarter AI"
                // house-rule, not a fidelity fix). See pret/pokered engine/battle/{trainer_ai,core}.asm.
                const AI_LAYER2_ENCOURAGEMENT: u8 = 0;
                let result = choose_moves(layers, &bs.enemy, &bs.player, AI_LAYER2_ENCOURAGEMENT);
                if let Some(slot) = result.pick_move(rand::random::<u8>()) {
                    let move_id = mon.moves[slot];
                    if move_id != MoveId::None && mon.pp[slot] > 0 {
                        return (move_id, slot as u8);
                    }
                }
            }
        }

        let idx: usize = rand::random::<usize>() % available.len();
        available[idx]
    }

    fn build_move_menu_from_state(bs: &BattleState) -> MoveMenuState {
        let mon = bs.player.active_mon();
        let slots: Vec<MoveSlot> = mon
            .moves
            .iter()
            .enumerate()
            .filter(|(_, m)| **m != MoveId::None)
            .map(|(i, m)| {
                let max_pp = MoveData::get(*m).map_or(0, |d| d.pp);
                MoveSlot {
                    move_id: *m,
                    current_pp: mon.pp[i],
                    max_pp,
                    is_disabled: bs.player.disabled_move > 0
                        && bs.player.disabled_move == (i as u8 + 1),
                }
            })
            .collect();
        MoveMenuState::new(slots)
    }

    fn format_move_outcome(
        side_name: &str,
        move_name: &str,
        outcome: &move_execution::MoveOutcome,
        _target_name: &str,
    ) -> Vec<String> {
        let mut msgs = vec![format!("{} used {}!", side_name, move_name)];
        match outcome {
            move_execution::MoveOutcome::Success {
                is_critical,
                type_effectiveness,
                ..
            } => {
                if *is_critical {
                    msgs.push("Critical hit!".to_string());
                }
                if type_effectiveness.is_super_effective() {
                    msgs.push("It's super effective!".to_string());
                } else if type_effectiveness.is_not_very_effective() {
                    msgs.push("It's not very effective...".to_string());
                } else if type_effectiveness.is_no_effect() {
                    msgs.push("It doesn't affect the enemy!".to_string());
                }
            }

            move_execution::MoveOutcome::Missed => {
                msgs.push(format!("{}'s attack missed!", side_name));
            }
            move_execution::MoveOutcome::CannotMove(reason) => {
                msgs.clear();
                msgs.push(format!("{} {}", side_name, format_cannot_move(reason)));
            }
            move_execution::MoveOutcome::NoDamageMove { .. } => {}
        }
        msgs
    }

    pub fn update_frame(&mut self, input: BattleInput) -> ScreenAction {
        // HP-bar drain/refill animation (engine/gfx/hp_bar.asm): steps the
        // displayed HP toward the real HP at 1 bar pixel per 2 frames.
        self.hp_bar_anim
            .tick(&mut self.player_hp, &mut self.enemy_hp);
        match self.phase.clone() {
            BattlePhase::Intro {
                phase,
                mut wait_frames,
            } => {
                // Button press can skip text-wait phases but NOT
                // TransitionFlash or SilhouetteSlide (non-skippable animations).
                let is_unskippable = matches!(
                    phase,
                    IntroPhase::SilhouetteSlide
                        | IntroPhase::BattleTransitionWipe(_)
                        | IntroPhase::TransitionFlash
                );
                if !is_unskippable && (input.a || input.b) {
                    wait_frames = 0;
                }
                if wait_frames > 0 {
                    self.phase = BattlePhase::Intro {
                        phase,
                        wait_frames: wait_frames - 1,
                    };
                    return ScreenAction::Continue;
                }
                let needs_input = matches!(
                    phase,
                    IntroPhase::WildReveal
                        | IntroPhase::GhostCantID
                        | IntroPhase::GhostUnveil
                        | IntroPhase::TrainerReveal
                        | IntroPhase::TrainerSendOut
                        | IntroPhase::PlayerSendOut
                );
                if needs_input && !input.a && !input.b {
                    return ScreenAction::Continue;
                }
                let next_phase = match phase {
                    IntroPhase::BattleTransitionWipe(_) => BattlePhase::Intro {
                        phase: IntroPhase::SilhouetteSlide,
                        wait_frames: 72,
                    },
                    IntroPhase::SilhouetteSlide => {
                        if self.is_wild {
                            BattlePhase::Intro {
                                phase: IntroPhase::WildReveal,
                                wait_frames: 30,
                            }
                        } else {
                            self.show_enemy_pokeballs = true;
                            BattlePhase::Intro {
                                phase: IntroPhase::TrainerReveal,
                                wait_frames: 30,
                            }
                        }
                    }
                    IntroPhase::WildReveal => {
                        // PrintBeginningBattleText (engine/battle/common_text.asm):
                        // the ghost-Marowak battle WITH the scope runs the unveil
                        // text + MarowakAnim, then loops back here as a normal
                        // "Wild MAROWAK appeared!"; a no-scope ghost battle prints
                        // the second "can't be ID'd" line before the send-out.
                        if self.ghost_marowak_reveal && !self.ghost_marowak_unveiled {
                            BattlePhase::Intro {
                                // ≈ the frontend GhostMarowakReveal anim length
                                // (flash 8× + fade out + fade in, ~240 frames).
                                phase: IntroPhase::GhostUnveil,
                                wait_frames: 250,
                            }
                        } else if self.is_ghost {
                            BattlePhase::Intro {
                                phase: IntroPhase::GhostCantID,
                                wait_frames: 30,
                            }
                        } else {
                            BattlePhase::Intro {
                                phase: IntroPhase::PlayerSendOut,
                                wait_frames: 0,
                            }
                        }
                    }
                    IntroPhase::GhostCantID => BattlePhase::Intro {
                        phase: IntroPhase::PlayerSendOut,
                        wait_frames: 0,
                    },
                    IntroPhase::GhostUnveil => {
                        // The GHOST is unveiled as MAROWAK: the intro loops back to
                        // WildReveal, which now shows the normal "Wild MAROWAK
                        // appeared!" line (and the revealed sprite).
                        self.ghost_marowak_unveiled = true;
                        BattlePhase::Intro {
                            phase: IntroPhase::WildReveal,
                            wait_frames: 30,
                        }
                    }
                    IntroPhase::TrainerReveal => BattlePhase::Intro {
                        phase: IntroPhase::TrainerSendOut,
                        wait_frames: 30,
                    },
                    IntroPhase::TrainerSendOut => BattlePhase::Intro {
                        phase: IntroPhase::PlayerSendOut,
                        wait_frames: 0,
                    },
                    IntroPhase::PlayerSendOut => {
                        self.show_player_pokeballs = true;
                        let species_name = format!("{}", self.player_species).to_uppercase();
                        self.current_message = Some(format!("Go! {}!", species_name));
                        BattlePhase::PlayerMenu
                    }
                    // FlashScreen strobe (Circle/DoubleCircle only) plays
                    // BEFORE the wipe, exactly as BattleTransition_Circle /
                    // BattleTransition_DoubleCircle call it in the original.
                    IntroPhase::TransitionFlash => BattlePhase::Intro {
                        phase: IntroPhase::BattleTransitionWipe(self.battle_transition),
                        wait_frames: 120,
                    },
                };
                self.phase = next_phase;
                ScreenAction::Continue
            }
            BattlePhase::PlayerMenu => {
                // Old-Man catch tutorial: the game auto-plays a scripted, guaranteed
                // catch (no player input). Runs once — it transitions off PlayerMenu.
                if self.is_old_man {
                    self.resolve_old_man_tutorial();
                    return ScreenAction::Continue;
                }
                let menu_input = BattleMenuInput {
                    up: input.up,
                    down: input.down,
                    left: input.left,
                    right: input.right,
                    a: input.a,
                    b: input.b,
                };
                if self.is_safari {
                    // Safari battle: BALL / BAIT / ROCK / RUN, no attacking.
                    if let Some(action) = self.safari_menu.update_frame(menu_input) {
                        self.resolve_safari_action(action);
                    }
                } else if let Some(action) = self.battle_menu.update_frame(menu_input) {
                    match action {
                        BattleMenuAction::Fight => {
                            if let Some(ref bs) = self.battle_state {
                                self.move_menu = Some(Self::build_move_menu_from_state(bs));
                            }
                            self.phase = BattlePhase::MoveSelect;
                        }
                        BattleMenuAction::Run => {
                            self.handle_run();
                        }
                        BattleMenuAction::Pokemon => {
                            if let Some(ref bs) = self.battle_state {
                                if bs.player.party.len() > 1 {
                                    self.party_cursor = 0;
                                    self.phase = BattlePhase::PartySelect;
                                } else {
                                    self.show_text_then(
                                        vec!["No other POKeMON!".to_string()],
                                        BattlePhase::PlayerMenu,
                                    );
                                }
                            }
                        }
                        BattleMenuAction::Bag => {
                            if self.link_mode {
                                // Items can't be used in link battles
                                // (engine/battle/core.asm:2171-2179).
                                self.show_text_then(
                                    vec!["Items can't be used here.".to_string()],
                                    BattlePhase::PlayerMenu,
                                );
                                return ScreenAction::Continue;
                            }
                            let usable_items: Vec<(ItemId, u8)> = self
                                .player_bag
                                .items()
                                .iter()
                                .filter(|(id, _)| {
                                    let cat = ItemCategory::from_item(*id);
                                    if self.is_wild {
                                        cat.is_usable_in_battle()
                                    } else {
                                        cat.is_usable_in_trainer_battle()
                                    }
                                })
                                .map(|&(id, q)| (id, q as u8))
                                .collect();
                            if usable_items.is_empty() {
                                self.show_text_then(
                                    vec!["No items!".to_string()],
                                    BattlePhase::PlayerMenu,
                                );
                            } else {
                                self.bag_menu = Some(BagMenuState::new(usable_items));
                                self.phase = BattlePhase::BagSelect;
                            }
                        }
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::MoveSelect => {
                let menu_input = MenuInput {
                    up: input.up,
                    down: input.down,
                    a: input.a,
                    b: input.b,
                };
                if let Some(ref mut mm) = self.move_menu {
                    if let Some(result) = mm.update_frame(menu_input) {
                        match result {
                            MoveMenuResult::Selected(idx) => {
                                if self.link_mode {
                                    // Link battle: defer execution until the
                                    // remote action arrives (the original
                                    // exchanges actions in SelectEnemyMove
                                    // before anything resolves — a locked
                                    // mon's forced move is what actually goes
                                    // over the wire, the menu choice ignored).
                                    let locked_idx = self.battle_state.as_ref().and_then(|bs| {
                                        if move_is_locked(&bs.player) {
                                            Some(bs.player.selected_move_index)
                                        } else {
                                            None
                                        }
                                    });
                                    self.link_pending_local_action =
                                        Some(crate::link::protocol::LinkAction::UseMove(
                                            locked_idx.unwrap_or(idx as u8),
                                        ));
                                    self.phase = BattlePhase::LinkWaiting;
                                } else {
                                    self.execute_turn_with_move(idx);
                                }
                            }
                            MoveMenuResult::Cancelled => {
                                self.move_menu = None;
                                self.battle_menu = BattleMenuState::new();
                                self.phase = BattlePhase::PlayerMenu;
                            }
                            MoveMenuResult::NoPP(_) => {
                                self.current_message = Some("No PP left!".to_string());
                            }
                            MoveMenuResult::Disabled(_) => {
                                self.current_message = Some("Move is disabled!".to_string());
                            }
                        }
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::BagSelect => {
                let menu_input = BattleMenuInput {
                    up: input.up,
                    down: input.down,
                    left: false,
                    right: false,
                    a: input.a,
                    b: input.b,
                };
                if let Some(ref mut bm) = self.bag_menu {
                    if let Some(result) = bm.update_frame(menu_input) {
                        match result {
                            BagMenuResult::Selected(item_id) => {
                                self.handle_item_use(item_id);
                            }
                            BagMenuResult::Cancelled => {
                                self.bag_menu = None;
                                self.battle_menu = BattleMenuState::new();
                                self.phase = BattlePhase::PlayerMenu;
                            }
                        }
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::ItemTargetSelect { item_id } => {
                if input.b {
                    self.bag_menu = None;
                    self.battle_menu = BattleMenuState::new();
                    self.phase = BattlePhase::PlayerMenu;
                    return ScreenAction::Continue;
                }
                if let Some(ref bs) = self.battle_state {
                    let party_len = bs.player.party.len();
                    if input.down {
                        self.party_cursor = (self.party_cursor + 1) % party_len;
                    } else if input.up {
                        self.party_cursor = if self.party_cursor == 0 {
                            party_len - 1
                        } else {
                            self.party_cursor - 1
                        };
                    }
                    if input.a {
                        self.apply_item_to_pokemon(item_id, self.party_cursor);
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::ShowingText {
                messages,
                current,
                wait_frames,
                next_phase,
            } => {
                if wait_frames > 0 {
                    self.phase = BattlePhase::ShowingText {
                        messages: messages.clone(),
                        current,
                        wait_frames: wait_frames - 1,
                        next_phase,
                    };
                    return ScreenAction::Continue;
                }
                self.current_message = Some(messages[current].clone());
                // The original animates the HP bar *synchronously* before
                // printing the next line (predef UpdateHPBar2,
                // engine/battle/core.asm:4727); reproduce that by holding the
                // current page until the drain/refill finishes.
                if (input.a || input.b) && !self.hp_bar_anim.is_active() {
                    let next_idx = current + 1;
                    if next_idx >= messages.len() {
                        self.current_message = None;
                        self.phase = *next_phase;
                        self.post_text_transition();
                    } else {
                        self.phase = BattlePhase::ShowingText {
                            messages,
                            current: next_idx,
                            wait_frames: BATTLE_TEXT_PAGE_WAIT_FRAMES,
                            next_phase,
                        };
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::PartySelect => {
                if input.b {
                    self.battle_menu = BattleMenuState::new();
                    self.phase = BattlePhase::PlayerMenu;
                    return ScreenAction::Continue;
                }
                if let Some(ref bs) = self.battle_state {
                    let party_len = bs.player.party.len();
                    if input.down {
                        self.party_cursor = (self.party_cursor + 1) % party_len;
                    } else if input.up {
                        self.party_cursor = if self.party_cursor == 0 {
                            party_len - 1
                        } else {
                            self.party_cursor - 1
                        };
                    }
                    if input.a {
                        let chosen = self.party_cursor;
                        self.party_submenu = Some(PartySubMenuState::new());
                        self.phase = BattlePhase::PartySubMenu {
                            selected_index: chosen,
                        };
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::PartySubMenu { selected_index } => {
                if let Some(ref mut submenu) = self.party_submenu {
                    let menu_input = BattleMenuInput {
                        up: input.up,
                        down: input.down,
                        left: false,
                        right: false,
                        a: input.a,
                        b: input.b,
                    };
                    if let Some(action) = submenu.update_frame(menu_input) {
                        match action {
                            PartySubMenuAction::Switch => {
                                if let Some(ref bs) = self.battle_state {
                                    let active = bs.player.active_pokemon_index;
                                    if selected_index == active {
                                        let name =
                                            format!("{}", bs.player.party[selected_index].species)
                                                .to_uppercase();
                                        self.show_text_then(
                                            vec![
                                                format!("{} is", name),
                                                "already out!".to_string(),
                                            ],
                                            BattlePhase::PartySelect,
                                        );
                                    } else if bs.player.party[selected_index].hp == 0 {
                                        self.show_text_then(
                                            vec![
                                                "There's no will".to_string(),
                                                "to fight!".to_string(),
                                            ],
                                            BattlePhase::PartySelect,
                                        );
                                    } else {
                                        self.party_submenu = None;
                                        self.switch_player_pokemon(selected_index);
                                    }
                                }
                            }
                            PartySubMenuAction::Stats => {
                                self.phase = BattlePhase::PartyStats {
                                    pokemon_index: selected_index,
                                };
                            }
                            PartySubMenuAction::Cancel => {
                                self.party_submenu = None;
                                self.phase = BattlePhase::PartySelect;
                            }
                        }
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::PartyStats { pokemon_index } => {
                if input.a || input.b {
                    self.phase = BattlePhase::PartySelect;
                }
                ScreenAction::Continue
            }
            BattlePhase::EnemySendingNext { wait_frames } => {
                if wait_frames > 0 {
                    self.phase = BattlePhase::EnemySendingNext {
                        wait_frames: wait_frames - 1,
                    };
                    return ScreenAction::Continue;
                }
                self.sync_display_from_state();
                self.show_enemy_pokeballs = !self.is_wild;
                self.battle_menu = BattleMenuState::new();
                if let Some(chosen) = self.pending_shift_switch.take() {
                    // SHIFT-prompt YES: the enemy's next mon is out; now the
                    // player's free switch runs (original ReplaceFaintedEnemyMon
                    // → SwitchPlayerMon). No enemy free turn — the turn already
                    // ended with the faint.
                    self.apply_shift_switch(chosen);
                } else {
                    self.phase = BattlePhase::PlayerMenu;
                }
                ScreenAction::Continue
            }
            BattlePhase::ShiftPrompt => {
                // YES/NO cursor (up/down toggles; B answers NO, like the
                // original's TWO_OPTION_MENU cancel landing on the default).
                if input.up || input.down {
                    self.shift_prompt_yes = !self.shift_prompt_yes;
                }
                let answered_yes = input.a && self.shift_prompt_yes;
                let answered_no = input.b || (input.a && !self.shift_prompt_yes);
                if answered_no {
                    self.current_message = None;
                    self.send_next_enemy();
                    self.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
                } else if answered_yes {
                    self.current_message = None;
                    self.party_cursor = 0;
                    self.phase = BattlePhase::ShiftSwitchSelect;
                }
                ScreenAction::Continue
            }
            BattlePhase::ShiftSwitchSelect => {
                // B backs out of the party menu and the battle proceeds without
                // switching (original `GoBackToPartyMenu` → `jr c, .next7`).
                if input.b {
                    self.send_next_enemy();
                    self.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
                    return ScreenAction::Continue;
                }
                if let Some(ref bs) = self.battle_state {
                    let party_len = bs.player.party.len();
                    if input.down {
                        self.party_cursor = (self.party_cursor + 1) % party_len;
                    } else if input.up {
                        self.party_cursor = if self.party_cursor == 0 {
                            party_len - 1
                        } else {
                            self.party_cursor - 1
                        };
                    }
                    if input.a {
                        let chosen = self.party_cursor;
                        let active = bs.player.active_pokemon_index;
                        if chosen == active {
                            let name = format!("{}", bs.player.party[chosen].species)
                                .to_uppercase();
                            self.show_text_then(
                                vec![format!("{} is", name), "already out!".to_string()],
                                BattlePhase::ShiftSwitchSelect,
                            );
                        } else if bs.player.party[chosen].hp == 0 {
                            self.show_text_then(
                                vec!["There's no will".to_string(), "to fight!".to_string()],
                                BattlePhase::ShiftSwitchSelect,
                            );
                        } else {
                            // Defer the actual switch until the enemy's next mon
                            // has been sent out (EnemySendingNext completion).
                            self.pending_shift_switch = Some(chosen);
                            self.send_next_enemy();
                            self.phase = BattlePhase::EnemySendingNext { wait_frames: 30 };
                        }
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::PlayerFaintSwitch => {
                if let Some(ref bs) = self.battle_state {
                    let party_len = bs.player.party.len();
                    if input.down {
                        self.party_cursor = (self.party_cursor + 1) % party_len;
                    } else if input.up {
                        self.party_cursor = if self.party_cursor == 0 {
                            party_len - 1
                        } else {
                            self.party_cursor - 1
                        };
                    }
                    if input.a {
                        let chosen = self.party_cursor;
                        if bs.player.party[chosen].hp == 0 {
                            self.show_text_then(
                                vec!["There's no will".to_string(), "to fight!".to_string()],
                                BattlePhase::PlayerFaintSwitch,
                            );
                        } else {
                            self.force_switch_player(chosen);
                        }
                    }
                }
                ScreenAction::Continue
            }
            BattlePhase::LinkWaiting => {
                // Waiting for the remote player's action. The driver calls
                // resolve_link_turn when both sides' actions are exchanged.
                ScreenAction::Continue
            }
            BattlePhase::TrainerVictory {
                phase,
                mut wait_frames,
                player_won,
            } => {
                use VictoryPhase::*;
                match phase {
                    DefeatedText => {
                        if input.a || input.b || wait_frames == 0 {
                            self.current_message = None;
                            self.phase = BattlePhase::TrainerVictory {
                                phase: TrainerPicScrollIn,
                                wait_frames: 50,
                                player_won,
                            };
                        } else {
                            self.phase = BattlePhase::TrainerVictory {
                                phase: DefeatedText,
                                wait_frames: wait_frames - 1,
                                player_won,
                            };
                        }
                        ScreenAction::Continue
                    }
                    TrainerPicScrollIn => {
                        if wait_frames > 0 {
                            self.phase = BattlePhase::TrainerVictory {
                                phase: TrainerPicScrollIn,
                                wait_frames: wait_frames - 1,
                                player_won,
                            };
                            ScreenAction::Continue
                        } else {
                            self.phase = BattlePhase::TrainerVictory {
                                phase: WaitFrames,
                                wait_frames: 40,
                                player_won,
                            };
                            ScreenAction::Continue
                        }
                    }
                    WaitFrames => {
                        if input.a || input.b || wait_frames == 0 {
                            self.phase = BattlePhase::TrainerVictory {
                                phase: EndBattleText,
                                wait_frames: 60,
                                player_won,
                            };
                        } else {
                            self.phase = BattlePhase::TrainerVictory {
                                phase: WaitFrames,
                                wait_frames: wait_frames - 1,
                                player_won,
                            };
                        }
                        ScreenAction::Continue
                    }
                    EndBattleText => {
                        if wait_frames == 60 {
                            if self.link_mode {
                                // Link battles award no money (TrainerBattleVictory
                                // skips the prize flow for LINK_STATE_BATTLING,
                                // core.asm:934-937); the win/loss line was already
                                // shown by finish_link_battle / the faint flow —
                                // advance straight to the settlement.
                                self.show_text_then(
                                    vec![],
                                    BattlePhase::TrainerVictory {
                                        phase: EndBattleText,
                                        wait_frames: 0,
                                        player_won,
                                    },
                                );
                                return ScreenAction::Continue;
                            }
                            if player_won {
                                let prize = self.calc_prize_money();
                                let payday = self
                                    .battle_state
                                    .as_ref()
                                    .map(|bs| bs.total_payday_money)
                                    .unwrap_or(0);
                                let total = calc_total_winnings(prize, payday);
                                let trainer_name = self
                                    .trainer_name
                                    .clone()
                                    .unwrap_or_else(|| "TRAINER".to_string());
                                let mut money_msgs = vec![
                                    format!("{} wants to", trainer_name),
                                    "give you a tip!".to_string(),
                                ];
                                if prize > 0 {
                                    money_msgs.push(format!("Player got ${} for", prize));
                                    money_msgs.push("winning!".to_string());
                                }
                                if payday > 0 {
                                    money_msgs.push(format!("Plus ${} from Pay Day!", payday));
                                }
                                if total > 0 {
                                    money_msgs.push(format!("Total: ${}!", total));
                                }
                                // Prepend the trainer's one-shot victory quip
                                // (original PrintEndBattleText, shown before the
                                // prize-money text) for sight/talk battles.
                                let mut msgs = Vec::new();
                                if let Some(quip) =
                                    self.end_battle_text.as_ref().filter(|q| !q.is_empty())
                                {
                                    msgs.push(quip.clone());
                                }
                                msgs.extend(money_msgs);
                                self.show_text_then(
                                    msgs,
                                    BattlePhase::TrainerVictory {
                                        phase: EndBattleText,
                                        wait_frames: 0,
                                        player_won,
                                    },
                                );
                            } else {
                                self.show_text_then(
                                    vec!["Player blacked out!".to_string()],
                                    BattlePhase::TrainerVictory {
                                        phase: EndBattleText,
                                        wait_frames: 0,
                                        player_won,
                                    },
                                );
                            }
                            return ScreenAction::Continue;
                        }
                        if input.a || input.b || wait_frames == 0 {
                            let outcome = if player_won {
                                BattleOutcome::Win
                            } else {
                                BattleOutcome::Loss
                            };
                            self.finalize_settlement(outcome);
                            ScreenAction::Transition(GameScreen::Overworld)
                        } else {
                            self.phase = BattlePhase::TrainerVictory {
                                phase: EndBattleText,
                                wait_frames: wait_frames - 1,
                                player_won,
                            };
                            ScreenAction::Continue
                        }
                    }
                }
            }
            BattlePhase::BattleOver {
                won,
                escaped,
                mut wait_frames,
            } => {
                if input.a || input.b {
                    wait_frames = 0;
                }
                if wait_frames > 0 {
                    self.phase = BattlePhase::BattleOver {
                        won,
                        escaped,
                        wait_frames: wait_frames - 1,
                    };
                    return ScreenAction::Continue;
                }
                let outcome = if won {
                    BattleOutcome::Win
                } else if escaped {
                    BattleOutcome::Escaped
                } else {
                    BattleOutcome::Loss
                };
                self.finalize_settlement(outcome);
                ScreenAction::Transition(GameScreen::Overworld)
            }
        }
    }

    fn show_text_then(&mut self, messages: Vec<String>, next: BattlePhase) {
        if messages.is_empty() {
            self.phase = next;
            return;
        }

        let expanded: Vec<String> = messages
            .iter()
            .flat_map(|m| paginate_battle_text(m))
            .collect();

        if expanded.is_empty() {
            self.phase = next;
            return;
        }

        self.current_message = Some(expanded[0].clone());
        self.phase = BattlePhase::ShowingText {
            messages: expanded,
            current: 0,
            wait_frames: BATTLE_TEXT_PAGE_WAIT_FRAMES,
            next_phase: Box::new(next),
        };
    }

    fn handle_item_use(&mut self, item_id: ItemId) {
        let category = ItemCategory::from_item(item_id);
        match category {
            ItemCategory::Ball => {
                if !self.is_wild {
                    self.show_text_then(
                        vec!["No! There's no running from a trainer battle!".to_string()],
                        BattlePhase::PlayerMenu,
                    );
                    return;
                }
                self.use_ball(item_id);
            }
            ItemCategory::Healing | ItemCategory::StatusCure | ItemCategory::Revive => {
                self.phase = BattlePhase::ItemTargetSelect { item_id };
            }
            ItemCategory::BattleStat => {
                self.use_battle_stat_item(item_id);
            }
            ItemCategory::UsableInBattle => {
                if item_id == ItemId::PokeDoll {
                    self.use_poke_doll();
                } else if item_id == ItemId::PokeFlute {
                    self.use_poke_flute();
                } else {
                    self.show_text_then(
                        vec!["Can't use that here!".to_string()],
                        BattlePhase::PlayerMenu,
                    );
                }
            }
            ItemCategory::NotUsableInBattle => {
                self.show_text_then(
                    vec!["Can't use that here!".to_string()],
                    BattlePhase::PlayerMenu,
                );
            }
        }
    }

    fn apply_item_to_pokemon(&mut self, item_id: ItemId, pokemon_index: usize) {
        if pokemon_index == 0 && item_id != ItemId::Revive && item_id != ItemId::MaxRevive {
            let active_hp = self.player_hp;
            if active_hp == 0 && item_id != ItemId::Revive && item_id != ItemId::MaxRevive {
                self.show_text_then(vec!["No effect!".to_string()], BattlePhase::PlayerMenu);
                return;
            }
        }

        let category = ItemCategory::from_item(item_id);
        let result_msg = if let Some(ref mut bs) = self.battle_state {
            let mon = &mut bs.player.party[pokemon_index];
            match category {
                ItemCategory::Healing => {
                    use crate::items::healing::{use_healing_item, HealResult};
                    match use_healing_item(mon, item_id) {
                        HealResult::Healed { hp_restored } => {
                            self.consume_selected_item();
                            format!("HP restored by {}!", hp_restored)
                        }
                        HealResult::Revived { hp_restored } => {
                            self.consume_selected_item();
                            format!("Revived! HP restored by {}!", hp_restored)
                        }
                        HealResult::AlreadyFullHp => "Already at full HP!".to_string(),
                        HealResult::NotFainted => "Not fainted!".to_string(),
                        HealResult::NotApplicable => "No effect!".to_string(),
                    }
                }
                ItemCategory::StatusCure => {
                    use crate::items::status_cure::{use_status_cure, StatusCureResult};
                    match use_status_cure(mon, item_id) {
                        StatusCureResult::Cured => {
                            self.consume_selected_item();
                            "Status cured!".to_string()
                        }
                        StatusCureResult::NoEffect => "No status to cure!".to_string(),
                        StatusCureResult::NotApplicable => "No effect!".to_string(),
                    }
                }
                ItemCategory::Revive => {
                    use crate::items::healing::{use_healing_item, HealResult};
                    match use_healing_item(mon, item_id) {
                        HealResult::Revived { hp_restored } => {
                            self.consume_selected_item();
                            format!("Revived! HP restored by {}!", hp_restored)
                        }
                        _ => "Can't revive that!".to_string(),
                    }
                }
                _ => "No effect!".to_string(),
            }
        } else {
            "No effect!".to_string()
        };

        self.sync_display_from_state();
        self.bag_menu = None;
        self.show_text_then(vec![result_msg], BattlePhase::PlayerMenu);
    }

    fn consume_selected_item(&mut self) {
        if let Some(ref bm) = self.bag_menu {
            let cursor = bm.cursor();
            if self.player_bag.remove_item_at(cursor, 1).is_ok() {
                let remaining_items: Vec<(ItemId, u8)> = self
                    .player_bag
                    .items()
                    .iter()
                    .filter(|(id, _)| {
                        let cat = ItemCategory::from_item(*id);
                        if self.is_wild {
                            cat.is_usable_in_battle()
                        } else {
                            cat.is_usable_in_trainer_battle()
                        }
                    })
                    .map(|&(id, q)| (id, q as u8))
                    .collect();
                if remaining_items.is_empty() {
                    self.bag_menu = None;
                } else {
                    let saved = bm.saved_cursor().min(remaining_items.len() - 1);
                    self.bag_menu = Some(BagMenuState::with_saved_cursor(remaining_items, saved));
                }
            }
        }
    }

    /// Resolve one Safari turn (BALL / BAIT / ROCK / RUN), faithful to Gen-1's
    /// `engine/battle/core.asm` order: action → out-of-balls check → per-turn upkeep
    /// (`PrintSafariZoneBattleText`) → flee roll. See [`safari`](crate::battle::safari).
    fn resolve_safari_action(&mut self, action: menu::SafariMenuAction) {
        use crate::battle::capture::{CaptureRandoms, CaptureResult};
        use crate::battle::safari::{roll_bait_rock_amount, SafariUpkeep};
        use menu::SafariMenuAction;

        let escaped_over = BattlePhase::BattleOver { won: false, escaped: true, wait_frames: 30 };

        // RUN always escapes a Safari battle.
        if matches!(action, SafariMenuAction::Run) {
            self.show_text_then(vec!["Got away safely!".to_string()], escaped_over);
            return;
        }

        // Snapshot the wild mon (name / hp / status / speed) before mutating.
        let (max_hp, cur_hp, status, speed, name) = match self.battle_state.as_ref() {
            Some(bs) => {
                let e = bs.enemy.active_mon();
                let n = format!(
                    "Wild {}",
                    pokered_data::lang_data::species_name(e.species, false).to_uppercase()
                );
                (e.max_hp, e.hp, e.status, e.speed, n)
            }
            None => return,
        };

        let mut msgs: Vec<String> = Vec::new();
        let mut caught = false;
        match action {
            SafariMenuAction::Ball => {
                let randoms = CaptureRandoms { rand1: rand::random(), rand2: rand::random() };
                let result = self
                    .safari
                    .as_mut()
                    .map(|s| s.throw_ball(max_hp, cur_hp, status, randoms));
                match result {
                    Some(CaptureResult::Captured) => {
                        caught = true;
                        // $43: caught — toss, poof, hide pic, 3 shakes.
                        self.pending_anim_events.push_back(BattleAnimEvent::Ball {
                            ball: ItemId::SafariBall,
                            shakes: 3,
                            outcome: BallAnimOutcome::Caught,
                        });
                        if let Some(bs) = self.battle_state.as_ref() {
                            self.captured_mon = Some(bs.enemy.active_mon().clone());
                        }
                        msgs.push(format!("Gotcha!\n{} was caught!", name));
                    }
                    Some(CaptureResult::Failed { shakes }) => {
                        self.pending_anim_events.push_back(BattleAnimEvent::Ball {
                            ball: ItemId::SafariBall,
                            shakes,
                            outcome: BallAnimOutcome::BrokeFree,
                        });
                        msgs.push(
                            match shakes {
                                0 => "You missed the POKéMON!",
                                1 => "Darn! The POKéMON\nbroke free!",
                                2 => "Aww! It appeared\nto be caught!",
                                _ => "Shoot! It was so\nclose, too!",
                            }
                            .to_string(),
                        );
                    }
                    None => return,
                }
            }
            SafariMenuAction::Bait => {
                let amt = roll_bait_rock_amount(&mut || rand::random());
                if let Some(s) = self.safari.as_mut() {
                    s.apply_bait(amt);
                }
                msgs.push("Threw some BAIT!".to_string());
            }
            SafariMenuAction::Rock => {
                let amt = roll_bait_rock_amount(&mut || rand::random());
                if let Some(s) = self.safari.as_mut() {
                    s.apply_rock(amt);
                }
                msgs.push("Threw a ROCK!".to_string());
            }
            SafariMenuAction::Run => unreachable!("handled above"),
        }

        // A catch ends the battle immediately (the app adds the caught mon on settle).
        if caught {
            self.show_text_then(
                msgs,
                BattlePhase::BattleOver { won: true, escaped: false, wait_frames: 30 },
            );
            return;
        }

        // Out of Safari Balls → the game ends (checked BEFORE the upkeep, per the ASM).
        if self.safari.as_ref().map_or(false, |s| s.balls == 0) {
            msgs.push("PA: You're out of\nSAFARI BALLs! Game over!".to_string());
            self.show_text_then(msgs, escaped_over);
            return;
        }

        // Per-turn upkeep: decrement the bait/anger counter, narrate eating/angry.
        match self.safari.as_mut().map(|s| s.upkeep()) {
            Some(SafariUpkeep::Eating) => msgs.push(format!("{} is eating!", name)),
            Some(SafariUpkeep::Angry) => msgs.push(format!("{} is angry!", name)),
            _ => {}
        }

        // Flee roll.
        let fled = self
            .safari
            .as_ref()
            .map_or(false, |s| s.flee_roll(speed, rand::random()));
        if fled {
            msgs.push(format!("{} ran away!", name));
            self.show_text_then(msgs, escaped_over);
            return;
        }

        // Continue: refresh the BALL count shown in the menu and return to it.
        let balls = self.safari.as_ref().map_or(0, |s| s.balls);
        self.safari_menu = menu::SafariBattleMenuState::new(balls);
        self.show_text_then(msgs, BattlePhase::PlayerMenu);
    }

    /// The Old-Man catch tutorial's scripted, guaranteed catch (Gen-1
    /// `BATTLE_TYPE_OLD_MAN`): the OLD MAN throws a POKé BALL and the wild WEEDLE is
    /// caught — but it is a DEMO, so `captured_mon` stays `None` (nothing joins the
    /// party) and the battle simply ends, resuming the Viridian dialogue.
    fn resolve_old_man_tutorial(&mut self) {
        let species = match self.battle_state.as_ref() {
            Some(bs) => pokered_data::lang_data::species_name(
                bs.enemy.active_mon().species,
                false,
            )
            .to_uppercase(),
            None => "POKéMON".to_string(),
        };
        // The scripted catch runs the full ball choreography ($43: toss →
        // poof → hide pic → 3 shakes) like any successful capture.
        self.pending_anim_events.push_back(BattleAnimEvent::Ball {
            ball: ItemId::PokeBall,
            shakes: 3,
            outcome: BallAnimOutcome::Caught,
        });
        self.show_text_then(
            vec![
                "OLD MAN used\nPOKé BALL!".to_string(),
                format!("All right!\n{} was caught!", species),
            ],
            BattlePhase::BattleOver { won: true, escaped: false, wait_frames: 30 },
        );
    }

    fn use_ball(&mut self, ball_id: ItemId) {
        use crate::battle::capture::{try_capture, CaptureContext, CaptureRandoms, CaptureResult};
        use pokered_data::pokemon_data::get_base_stats;
        let ball_name = pokered_data::item_data::get_item_data(ball_id)
            .map(|d| d.name)
            .unwrap_or("POKé BALL");
        let thrower = self
            .player_name
            .clone()
            .unwrap_or_else(|| "RED".to_string())
            .to_uppercase();
        let used_msg = format!("{} used\n{}!", thrower, ball_name);
        // A Pokémon-Tower GHOST (no Silph Scope) is uncatchable — the ball is dodged and
        // NOT consumed (the mon is unidentified until the Scope reveals it).
        if self.is_ghost {
            // wPokeBallAnimData = $10: toss only — the ghost dodges
            // (DoBallTossSpecialEffects slides it left for the last frames).
            self.pending_anim_events.push_back(BattleAnimEvent::Ball {
                ball: ball_id,
                shakes: 0,
                outcome: BallAnimOutcome::Dodged,
            });
            self.show_text_then(
                vec![used_msg, "The GHOST is dodging\nyour POKé BALLs!".to_string()],
                BattlePhase::PlayerMenu,
            );
            return;
        }
        if let Some(ref mut bs) = self.battle_state {
            let enemy = bs.enemy.active_mon();
            let catch_rate = get_base_stats(enemy.species)
                .map(|s| s.catch_rate)
                .unwrap_or(255);
            // Snapshot the wild mon in its current (weakened) state before the
            // borrow of `bs` ends, so it can be handed to the party on a catch.
            // A freshly caught mon is the player's own: stamp it with the
            // player's OT ID/name (MON_OTID + the party OT-name table) so
            // obedience and the SRAM round-trip see it as self-caught.
            let mut caught_candidate = enemy.clone();
            caught_candidate.ot_id = self.player_id;
            if caught_candidate.ot_name.is_none() {
                caught_candidate.ot_name = self.player_name.clone();
            }
            let ctx = CaptureContext {
                ball: ball_id,
                wild_max_hp: enemy.max_hp,
                wild_current_hp: enemy.hp,
                wild_catch_rate: catch_rate,
                wild_status: enemy.status,
            };
            let randoms = CaptureRandoms {
                rand1: rand::random(),
                rand2: rand::random(),
            };
            let result = try_capture(&ctx, &randoms);
            self.consume_selected_item();
            // wPokeBallAnimData: $43 caught (3 shakes) / $20 missed /
            // $61-$63 broke free after N shakes (ItemUseBall's
            // .setAnimData). The frontend stages the choreography from this.
            let (outcome, shakes) = match result {
                CaptureResult::Captured => (BallAnimOutcome::Caught, 3),
                CaptureResult::Failed { shakes } => (BallAnimOutcome::BrokeFree, shakes),
            };
            self.pending_anim_events.push_back(BattleAnimEvent::Ball {
                ball: ball_id,
                shakes,
                outcome,
            });
            let (msg, next) = match result {
                CaptureResult::Captured => {
                    self.captured_mon = Some(caught_candidate);
                    // A catch ENDS the battle (the app adds the caught mon on
                    // settle) — previously the BattleOver phase set here was
                    // clobbered by the show_text_then below.
                    (
                        "Caught!".to_string(),
                        BattlePhase::BattleOver {
                            won: true,
                            escaped: false,
                            wait_frames: 30,
                        },
                    )
                }
                CaptureResult::Failed { shakes } => {
                    let shake_msg = match shakes {
                        0 => "Oh no! The ball missed!",
                        1 => "Aww! It broke free!",
                        2 => "Shoot! It almost had it!",
                        3 => "Shoot! It was so close too!",
                        _ => "It broke free!",
                    };
                    (shake_msg.to_string(), BattlePhase::PlayerMenu)
                }
            };
            self.bag_menu = None;
            self.show_text_then(vec![used_msg, msg], next);
        }
    }

    fn use_battle_stat_item(&mut self, item_id: ItemId) {
        use crate::items::battle_items::{use_battle_item, BattleItemResult};
        // X-stat items route through the same `StatModifierUpEffect` as a stat-up
        // move (engine/items/item_effects.asm `ItemUseXStat` → `farcall
        // StatModifierUpEffect`), so a successful one ALSO re-applies the badge
        // boosts (the stat-up glitch, effects.asm:499).
        self.sync_player_context();
        if let Some(ref mut bs) = self.battle_state {
            let badges = bs.player_badges;
            let result = use_battle_item(&mut bs.player, item_id);
            match &result {
                BattleItemResult::StatBoosted { stat } => {
                    crate::battle::badge_boosts::reapply_on_stage_change_legacy(
                        &mut bs.player,
                        badges,
                        Some(*stat),
                    );
                }
                // X Accuracy maps to ACCURACY_UP1_EFFECT in the original — the
                // same effect path, so it too triggers the boost round (no stat
                // reset: accuracy is not one of the four boosted stats).
                BattleItemResult::FlagSet if item_id == ItemId::XAccuracy => {
                    crate::battle::badge_boosts::reapply_on_stage_change_legacy(
                        &mut bs.player,
                        badges,
                        None,
                    );
                }
                _ => {}
            }
            self.consume_selected_item();
            // ItemUseXStat plays XSTATITEM_ANIM on the player's mon
            // (engine/items/item_effects.asm:1657, then StatModifierUpEffect
            // runs wPlayerMoveNum = XSTATITEM_ANIM). Only a successful use
            // animates — "No effect!" / "Can't use that!" print directly.
            if matches!(
                result,
                BattleItemResult::StatBoosted { .. } | BattleItemResult::FlagSet
            ) {
                self.pending_anim_events.push_back(BattleAnimEvent::XStatItem);
            }
            let msg = match result {
                BattleItemResult::StatBoosted { stat } => {
                    let stat_name = match stat {
                        crate::battle::stat_stages::StatIndex::Attack => "ATTACK",
                        crate::battle::stat_stages::StatIndex::Defense => "DEFENSE",
                        crate::battle::stat_stages::StatIndex::Speed => "SPEED",
                        crate::battle::stat_stages::StatIndex::Special => "SPECIAL",
                        crate::battle::stat_stages::StatIndex::Accuracy => "ACCURACY",
                        crate::battle::stat_stages::StatIndex::Evasion => "EVASION",
                    };
                    format!("{} rose!", stat_name)
                }
                BattleItemResult::FlagSet => "Effect applied!".to_string(),
                BattleItemResult::NoEffect => "No effect!".to_string(),
                BattleItemResult::NotApplicable => "Can't use that!".to_string(),
                BattleItemResult::Escaped => unreachable!(),
            };
            self.bag_menu = None;
            self.sync_display_from_state();
            self.show_text_then(vec![msg], BattlePhase::PlayerMenu);
        }
    }

    fn use_poke_doll(&mut self) {
        self.consume_selected_item();
        self.bag_menu = None;
        self.show_text_then(
            vec!["Got away safely!".to_string()],
            BattlePhase::BattleOver {
                won: false,
                escaped: true,
                wait_frames: 30,
            },
        );
    }

    fn use_poke_flute(&mut self) {
        if let Some(ref mut bs) = self.battle_state {
            let player_was_asleep = bs.player.active_mon().status.is_sleep();
            let enemy_was_asleep = bs.enemy.active_mon().status.is_sleep();

            if player_was_asleep || enemy_was_asleep {
                for mon in bs.player.party.iter_mut() {
                    if mon.status.is_sleep() {
                        mon.status = StatusCondition::None;
                    }
                }
                for mon in bs.enemy.party.iter_mut() {
                    if mon.status.is_sleep() {
                        mon.status = StatusCondition::None;
                    }
                }
                bs.player.active_mon_mut().status = StatusCondition::None;
                bs.enemy.active_mon_mut().status = StatusCondition::None;
                self.sync_display_from_state();
                self.bag_menu = None;
                // engine/items/item_effects.asm:1732-1739: the in-battle flute
                // music plays only when some Pokémon were asleep.
                self.poke_flute_sfx_pending = true;
                self.show_text_then(
                    vec![
                        "Played the POKE FLUTE!".to_string(),
                        "All sleeping POKeMON woke up!".to_string(),
                    ],
                    BattlePhase::PlayerMenu,
                );
            } else {
                self.bag_menu = None;
                self.show_text_then(
                    vec!["Played the POKE FLUTE!".to_string()],
                    BattlePhase::PlayerMenu,
                );
            }
        }
    }

    fn handle_run(&mut self) {
        if self.link_mode {
            // Link battles: running ALWAYS succeeds and is COORDINATED with
            // the remote player (TryRunningFromBattle skips the speed check
            // for LINK_STATE_BATTLING, core.asm:1503-1510). Defer — the
            // driver sends RUN and the outcome (win/lose/draw) depends on the
            // remote's action.
            self.link_pending_local_action = Some(crate::link::protocol::LinkAction::Run);
            self.phase = BattlePhase::LinkWaiting;
            return;
        }
        // TryRunningFromBattle (engine/battle/core.asm:1496-1498): a ghost battle
        // (Pokémon Tower without the SILPH SCOPE) ALWAYS allows escape
        // (`call IsGhostBattle / jp z, .canEscape`).
        if self.is_ghost && self.battle_state.is_some() {
            self.show_text_then(
                vec!["Got away safely!".to_string()],
                BattlePhase::BattleOver {
                    won: false,
                    escaped: true,
                    wait_frames: 30,
                },
            );
            return;
        }
        if let Some(ref mut bs) = self.battle_state {
            let result = try_run_from_battle(bs, rand::random());
            match result {
                RunResult::Escaped => {
                    self.show_text_then(
                        vec!["Got away safely!".to_string()],
                        BattlePhase::BattleOver {
                            won: false,
                            escaped: true,
                            wait_frames: 30,
                        },
                    );
                }
                RunResult::CannotRun => {
                    self.show_text_then(
                        vec!["No! There's no running from a trainer battle!".to_string()],
                        BattlePhase::PlayerMenu,
                    );
                }
                RunResult::FailedToEscape => {
                    self.execute_enemy_free_turn();
                }
            }
        } else {
            self.phase = BattlePhase::BattleOver {
                won: false,
                escaped: false,
                wait_frames: 30,
            };
        }
    }

    /// A single ENEMY free move (after a failed escape / after the player switches),
    /// run on the STACK engine with the player doing Nothing — retiring the legacy
    /// `execute_move`/`apply_move_effect` dispatcher from production. Mirrors the
    /// enemy side of [`execute_turn_with_move`]: the AI pick + a locked/forced move,
    /// Metronome/Mirror Move resolution, and the enemy-side post-turn reconciliations
    /// (Mimic, Pay Day, Teleport-flee, recharge/charge/Substitute narration). `prefix`
    /// is prepended to the turn's messages ("Can't escape!" / the come-back lines).
    /// `enemy_move_override` replaces the AI pick with a wire-provided move
    /// `(move_id, move_index)` — the link-battle path, where the "enemy" is the
    /// remote player and its action came over the wire.
    fn run_enemy_free_turn_stack(
        &mut self,
        mut msgs: Vec<String>,
        enemy_move_override: Option<(MoveId, u8)>,
    ) {
        use crate::battle::pokered_rules;
        use crate::battle::state::status1::CHARGING_UP;
        use crate::battle::state::status2::{HAS_SUBSTITUTE_UP, NEEDS_TO_RECHARGE};
        use jrpg_engine::battle::stack::{StackDriver, TurnEvent};
        use jrpg_engine::battle::{BattleAction, BattlerRef};

        // Player just switched / is fleeing: the incoming mon's badge boosts
        // apply from send-out (before the enemy's free hit lands).
        self.sync_player_context();

        // Choose the enemy's move (AI, honoring a locked/forced move), then resolve
        // Metronome / Mirror Move — exactly as the enemy path of execute_turn_with_move.
        let enemy_move_idx;
        let (enemy_move_id, enemy_call, enemy_call_failed);
        {
            let bs = match self.battle_state.as_ref() {
                Some(b) => b,
                None => return,
            };
            let mut default_rng = pokered_rules::runtime::RandBattleRng;
            let rng: &mut dyn jrpg_engine::battle::rng::BattleRng = match self.link_rng.as_mut()
            {
                Some(link) => link,
                None => &mut default_rng,
            };
            let (eid, eidx) = match enemy_move_override {
                Some(m) => m,
                None => Self::pick_enemy_move(bs, self.trainer_class),
            };
            enemy_move_idx = eidx;
            let base = if move_is_locked(&bs.enemy) { bs.enemy.selected_move } else { eid };
            let (rid, rcall, rfailed) = resolve_called_move(base, bs.player.last_move_used, rng);
            enemy_move_id = rid;
            enemy_call = rcall;
            enemy_call_failed = rfailed;
        }
        let enemy_move = match MoveData::get(enemy_move_id) {
            Some(m) => *m,
            None => return,
        };
        // PrintGhostText (engine/battle/core.asm:3281-3299): in a ghost battle
        // the GHOST's own move is always blocked ("GHOST: Get out...").
        let ghost_enemy_blocked = self.is_ghost;

        pokered_rules::install_canonical();
        pokered_rules::clear_current_moves();
        pokered_rules::set_current_move(BattlerRef::OPPONENT, enemy_move);

        let (turn_msgs, escape_battle): (Vec<String>, bool) = {
            let bs = self.battle_state.as_mut().unwrap();
            let player_prior_move = bs.player.last_move_used;
            bs.enemy.selected_move = enemy_move_id;
            bs.enemy.selected_move_index = enemy_move_idx;
            let e_recharging = bs.enemy.has_status2(NEEDS_TO_RECHARGE);
            let e_was_charging = bs.enemy.has_status1(CHARGING_UP);
            let e_had_sub = bs.enemy.has_status2(HAS_SUBSTITUTE_UP);
            let (mut state, mut effects) = pokered_rules::runtime::engine_state_from_legacy(bs);
            // The player is fleeing / has just switched → it does Nothing this turn.
            // In a ghost battle the GHOST itself never attacks either —
            // PrintGhostText's `.Ghost` branch (engine/battle/core.asm:3295-3299)
            // prints "GHOST: Get out..." and skips its move.
            let actions = [
                BattleAction::<pokered_rules::PokeredRules>::Nothing,
                if enemy_call_failed || ghost_enemy_blocked {
                    BattleAction::Nothing
                } else {
                    BattleAction::Fight { move_: enemy_move_id }
                },
            ];
            // Shared-stream injection: in a link battle the turn's random
            // draws (accuracy, damage, crit, status, multi-hit, …) come from
            // the link RNG so both sides resolve identically (BattleRandom).
            let mut default_rng = pokered_rules::runtime::RandBattleRng;
            let rng: &mut dyn jrpg_engine::battle::rng::BattleRng = match self.link_rng.as_mut()
            {
                Some(link) => link,
                None => &mut default_rng,
            };
            let (_r, log) = StackDriver::execute_turn_logged(
                &pokered_rules::PokeredRules, &mut state, &mut effects, actions, rng,
            );
            pokered_rules::runtime::apply_engine_to_legacy(bs, &state, &effects);

            for ev in &log.events {
                if let TurnEvent::MoveUsed { actor, move_ } = ev {
                    if actor.side == 0 {
                        bs.player.last_move_used = *move_;
                    } else {
                        bs.enemy.last_move_used = *move_;
                    }
                }
            }
            let enemy_connected = |mv: MoveId| {
                log.events.iter().any(|ev| matches!(ev, TurnEvent::MoveUsed { actor, move_ }
                    if *actor == BattlerRef::OPPONENT && *move_ == mv))
                    && !log.events.iter().any(|ev| matches!(ev, TurnEvent::Missed { actor }
                        if *actor == BattlerRef::OPPONENT))
            };
            // Enemy Mimic: overwrite its Mimic slot with the player's prior move (PP→5).
            if enemy_connected(MoveId::Mimic) && player_prior_move != MoveId::None {
                let slot = bs.enemy.selected_move_index as usize;
                if slot < 4 {
                    let mon = bs.enemy.active_mon_mut();
                    mon.moves[slot] = player_prior_move;
                    mon.pp[slot] = 5;
                }
            }
            // Enemy Pay Day scatters coins into the pot.
            if enemy_connected(MoveId::PayDay) {
                let lvl = bs.enemy.active_mon().level as u32;
                bs.total_payday_money = bs.total_payday_money.saturating_add(lvl * 2);
            }
            // A connecting enemy Whirlwind/Roar/Teleport flees a WILD battle.
            let escape = log.events.iter().any(|ev| {
                if let TurnEvent::MoveUsed { actor, move_ } = ev {
                    let is_flee = MoveData::get(*move_).map(|m| m.effect)
                        == Some(pokered_data::moves::MoveEffect::SwitchAndTeleportEffect);
                    is_flee
                        && !log.events.iter().any(|e2| matches!(e2, TurnEvent::Missed { actor: a2 } if a2 == actor))
                } else {
                    false
                }
            });
            if e_recharging {
                bs.enemy.clear_status2(NEEDS_TO_RECHARGE);
            }

            let mut m = pokered_rules::runtime::translate_turn(&log, &state, &effects);
            if let Some(label) = enemy_call {
                if !ghost_enemy_blocked {
                    if enemy_call_failed {
                        m.insert(0, "But it failed!".to_string());
                    }
                    m.insert(0, format!("{} used {}!",
                        pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT), label));
                }
            }
            // The ghost battle's enemy-side text (_GetOutText) — the GHOST never
            // actually uses its move.
            if ghost_enemy_blocked {
                m.push("GHOST: Get out...\nGet out...".to_string());
            }
            if !e_was_charging && bs.enemy.has_status1(CHARGING_UP) {
                m.push(format!("{} {}",
                    pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT),
                    charge_message(enemy_move_id)));
            }
            let now_sub = bs.enemy.has_status2(HAS_SUBSTITUTE_UP);
            let created_sub = pokered_rules::sub_created_this_turn(BattlerRef::OPPONENT);
            if created_sub {
                m.push(format!("{} put in a SUBSTITUTE!",
                    pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT)));
            }
            if (e_had_sub || created_sub) && !now_sub {
                m.push(format!("{}'s SUBSTITUTE broke!",
                    pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT)));
            }
            if e_recharging {
                m.insert(0, format!("{} must recharge!",
                    pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT)));
            }
            (m, escape)
        };
        msgs.extend(turn_msgs);

        self.sync_display_from_state();
        let next = if escape_battle && self.is_wild {
            msgs.push("Got away safely!".to_string());
            BattlePhase::BattleOver { won: false, escaped: true, wait_frames: 30 }
        } else {
            self.check_faint_after_turn()
        };
        self.append_exp_messages(&mut msgs);
        self.show_text_then(msgs, next);
    }

    fn execute_enemy_free_turn(&mut self) {
        self.run_enemy_free_turn_stack(vec!["Can't escape!".to_string()], None);
    }

    /// Decide the enemy's trainer-AI action for this turn (Gen-1 `TrainerAI`), spending
    /// one `wAICount` charge per consultation (count > 0, guard passed) regardless of the
    /// routine's outcome. Returns the action to apply, or `None` (wild / exhausted /
    /// guarded / DoNothing / a switch with no living target). Does NOT mutate the battler —
    /// see [`apply_enemy_ai_action`]. No refund on a wasted charge: a fainting mon re-seeds
    /// its count on the next send-out, so an over-spend on a KO'd mon never matters.
    fn decide_enemy_ai_action(&mut self, rand_val: u8) -> Option<AiAction> {
        use crate::battle::state::status2;

        let class = self.trainer_class?;
        if self.enemy_ai_count == 0 {
            return None;
        }
        let cfg = trainer_ai_config(class);
        let count_before = self.enemy_ai_count; // > 0 (checked above)
        let action = {
            let bs = self.battle_state.as_ref()?;
            // Conservative guard (documented simplification): skip — and spend NO charge —
            // while the enemy is locked into a multi-turn move or recharging.
            if move_is_locked(&bs.enemy) || bs.enemy.has_status2(status2::NEEDS_TO_RECHARGE) {
                return None;
            }
            execute_ai_action(cfg.routine, &mut self.enemy_ai_count, &bs.enemy, rand_val)
        };
        // Per-turn budget: one charge per consultation (overrides execute_ai_action's
        // per-action decrement). Gen-1 decrements `wAICount` in the wrapper BEFORE the
        // class routine runs, so a DoNothing roll or a rolled-but-impossible switch STILL
        // spends a charge — a routine may only act within the first `count` turns a mon is out.
        self.enemy_ai_count = count_before - 1;
        match action {
            AiAction::DoNothing => None,
            AiAction::SwitchPokemon => {
                // Fires only if a living mon exists to switch to; otherwise the enemy
                // attacks normally and the charge stays spent (Gen-1 AISwitchIfEnoughMons).
                let bs = self.battle_state.as_ref()?;
                let active = bs.enemy.active_pokemon_index;
                if bs.enemy.party.iter().enumerate().any(|(i, p)| i != active && p.hp > 0) {
                    Some(AiAction::SwitchPokemon)
                } else {
                    None
                }
            }
            other => Some(other),
        }
    }

    /// Apply a decided trainer-AI action to the enemy battler and narrate into `msgs`
    /// (heal HP / cure status / +1 stat stage / Mist / switch). A switch re-seeds the AI
    /// budget for the incoming mon. Placement (pre- vs post-player-move) is the caller's
    /// job — see the speed-ordered dispatch in `execute_turn_with_move`.
    fn apply_enemy_ai_action(&mut self, action: AiAction, msgs: &mut Vec<String>) {
        use crate::battle::stat_stages::StatIndex;
        use crate::battle::state::status2;

        let trainer_name = self
            .trainer_name
            .clone()
            .unwrap_or_else(|| "ENEMY".to_string());
        let ai_count_seed = self
            .trainer_class
            .map(|c| trainer_ai_config(c).ai_count)
            .unwrap_or(0);
        let bs = match self.battle_state.as_mut() {
            Some(b) => b,
            None => return,
        };
        // Canonical display spelling (matches the stack-log narration, which also uses
        // species_name) — NOT the strum variant name (e.g. "MR.MIME", not "MRMIME").
        let enemy_display = format!(
            "Enemy {}",
            pokered_data::lang_data::species_name(bs.enemy.active_mon().species, false)
                .to_uppercase()
        );
        match action {
            AiAction::UsePotion { heal_amount } => {
                let mon = bs.enemy.active_mon_mut();
                let item = if heal_amount == 0 {
                    // Full Restore: heals to max AND cures status.
                    mon.hp = mon.max_hp;
                    mon.status = StatusCondition::None;
                    "FULL RESTORE"
                } else {
                    mon.hp = mon.hp.saturating_add(heal_amount).min(mon.max_hp);
                    match heal_amount {
                        20 => "POTION",
                        50 => "SUPER POTION",
                        200 => "HYPER POTION",
                        _ => "POTION",
                    }
                };
                msgs.push(format!("{} used {}!", trainer_name, item));
            }
            AiAction::UseFullHeal => {
                bs.enemy.active_mon_mut().status = StatusCondition::None;
                msgs.push(format!("{} used FULL HEAL!", trainer_name));
            }
            AiAction::UseXAttack
            | AiAction::UseXDefend
            | AiAction::UseXSpeed
            | AiAction::UseXSpecial => {
                let (stat, stat_name, item) = match action {
                    AiAction::UseXAttack => (StatIndex::Attack, "ATTACK", "X ATTACK"),
                    AiAction::UseXDefend => (StatIndex::Defense, "DEFENSE", "X DEFEND"),
                    AiAction::UseXSpeed => (StatIndex::Speed, "SPEED", "X SPEED"),
                    _ => (StatIndex::Special, "SPECIAL", "X SPECIAL"),
                };
                let changed = bs.enemy.stat_stages.modify(stat, 1);
                msgs.push(format!("{} used {}!", trainer_name, item));
                if changed {
                    msgs.push(format!("{}'s {} rose!", enemy_display, stat_name));
                }
            }
            AiAction::UseGuardSpec => {
                bs.enemy.set_status2(status2::PROTECTED_BY_MIST);
                msgs.push(format!("{} used GUARD SPEC.!", trainer_name));
            }
            AiAction::SwitchPokemon => {
                let active = bs.enemy.active_pokemon_index;
                let target = bs
                    .enemy
                    .party
                    .iter()
                    .enumerate()
                    .find(|(i, p)| *i != active && p.hp > 0)
                    .map(|(i, _)| i);
                match target {
                    Some(idx) => {
                        let name = |bs: &BattleState| {
                            pokered_data::lang_data::species_name(
                                bs.enemy.active_mon().species,
                                false,
                            )
                            .to_uppercase()
                        };
                        let old = name(bs);
                        bs.enemy.active_pokemon_index = idx;
                        bs.enemy.reset_volatile_status();
                        bs.enemy.refresh_unmodified_stats();
                        let new = name(bs);
                        // Each mon that enters play re-seeds its AI budget (wAICount),
                        // overriding the per-turn decrement.
                        self.enemy_ai_count = ai_count_seed;
                        msgs.push(format!("{} withdrew {}!", trainer_name, old));
                        msgs.push(format!("{} sent out {}!", trainer_name, new));
                    }
                    None => {
                        // No living target (decide already excludes this) — defensive no-op.
                    }
                }
            }
            AiAction::DoNothing => {}
        }
    }

    /// Test / pre-turn convenience (the enemy-first semantics): decide AND immediately
    /// apply. Returns whether an action fired.
    #[cfg(test)]
    fn enemy_ai_action_inner(&mut self, rand_val: u8, msgs: &mut Vec<String>) -> bool {
        match self.decide_enemy_ai_action(rand_val) {
            Some(action) => {
                self.apply_enemy_ai_action(action, msgs);
                true
            }
            None => false,
        }
    }

    fn execute_turn_with_move(&mut self, move_index: usize) {
        // Badge stat boosts / obedience context: push the frontend-supplied
        // badges + trainer ID into the battle state and apply the send-out
        // badge boost if the active mon has none yet.
        self.sync_player_context();
        // Trainer AI (Gen-1 `TrainerAI`, speed-ordered). Decide the enemy's item / switch
        // action now (spending its `wAICount` charge), but PLACE it by turn order below:
        //   * enemy FIRST  → apply BEFORE the turn (the player's move hits the result);
        //   * player FIRST → DEFER to after the player's move, CANCELLED on a KO (Gen-1
        //     skips TrainerAI when the player already fainted the enemy).
        // Either way the enemy does NOT attack this turn (its action → `Nothing`).
        // Link battles never run the AI — the enemy's action came over the wire
        // (TrainerAI returns early for LINK_STATE_BATTLING,
        // engine/battle/trainer_ai.asm:296-298); `link_enemy_skips_turn` covers
        // a remote switch/no-action (enemy does Nothing this turn).
        let ai_action = if self.link_enemy_skips_turn || self.link_enemy_move_override.is_some()
        {
            None
        } else {
            self.decide_enemy_ai_action(rand::random())
        };
        let enemy_ai_fired = ai_action.is_some() || self.link_enemy_skips_turn;
        let mut ai_msgs: Vec<String> = Vec::new();
        let mut ai_applied_pre = false; // enemy-first: applied before the turn
        let mut deferred_ai: Option<AiAction> = None; // player-first: applied after the turn

        let (mut player_move_id, mut enemy_move_id, enemy_move_idx);

        if let Some(ref bs) = self.battle_state {
            let mon = bs.player.active_mon();
            // Locked into a forced move (charge strike, rampage, …): the menu choice is
            // ignored — the continuation re-uses the locked move (persisted in
            // `selected_move`). forced_action forces the same move engine-side; pinning
            // it here keeps CURRENT_MOVES consistent so the native handlers use its data.
            player_move_id = if move_is_locked(&bs.player) {
                bs.player.selected_move
            } else {
                mon.moves[move_index]
            };
            // Link battles: the enemy move comes from the wire (raw move index
            // into the remote party's active mon); a locked enemy still forces
            // its locked move, exactly like the AI pick would be overridden.
            let (eid, eidx) = match self.link_enemy_move_override.take() {
                Some(m) => m,
                None => Self::pick_enemy_move(bs, self.trainer_class),
            };
            enemy_move_id = if move_is_locked(&bs.enemy) {
                bs.enemy.selected_move
            } else {
                eid
            };
            enemy_move_idx = eidx;
        } else {
            return;
        }

        // Ghost battle (Pokémon Tower without the SILPH SCOPE) — PrintGhostText
        // (engine/battle/core.asm:3281-3307), called at the top of BOTH
        // ExecutePlayerMove and ExecuteEnemyMove:
        //   * player side: unless the mon is frozen/asleep (those fall through to
        //     the normal status handling), "X is too scared to move!" prints and
        //     the move is skipped — ALL moves, before any PP is spent;
        //   * ghost side: "GHOST: Get out... Get out..." prints and its move is
        //     skipped, so the GHOST never attacks.
        let (player_scared, ghost_enemy_blocked) = match self.battle_state.as_ref() {
            Some(bs) if self.is_ghost => (
                !matches!(
                    bs.player.active_mon().status,
                    StatusCondition::Sleep(_) | StatusCondition::Freeze
                ),
                true,
            ),
            _ => (false, false),
        };

        // CheckForDisobedience (core.asm:3828, called from ExecutePlayerMove after
        // PrintGhostText and the status-condition gates): a TRADED mon above its
        // badge threshold may refuse its orders — nap, loaf around, hit itself,
        // or use a different move. The deterministic no-move states the original
        // resolves first (sleep/freeze/flinch/recharge) and the charging strike
        // (which explicitly SKIPS the check) are excluded here; the confusion and
        // full-paralysis ROLLS live inside the stack's BeforeMove gates, so for a
        // confused/paralyzed traded mon this check fires BEFORE those rolls —
        // in the original they resolve first (a documented ordering deviation:
        // the turn is lost either way; only the message attribution differs).
        let mut move_index = move_index;
        let mut disobedience_msgs: Vec<String> = Vec::new();
        let mut player_disobeyed = false;
        if !player_scared {
            if let Some(ref mut bs) = self.battle_state {
                let p = &bs.player;
                let skip = matches!(
                    p.active_mon().status,
                    StatusCondition::Sleep(_) | StatusCondition::Freeze
                ) || p.has_status1(state::status1::FLINCHED)
                    || p.has_status1(state::status1::CHARGING_UP)
                    || p.has_status2(state::status2::NEEDS_TO_RECHARGE);
                if !skip {
                    let sel_slot = if move_is_locked(p) {
                        p.selected_move_index as usize
                    } else {
                        move_index
                    };
                    let (level, ot_id, moves, pp, has_disabled, name) = {
                        let m = p.active_mon();
                        (
                            m.level,
                            m.ot_id,
                            m.moves,
                            m.pp,
                            p.disabled_move > 0,
                            m.display_name(),
                        )
                    };
                    if crate::battle::obedience::is_traded_for(ot_id, bs.player_id) {
                        use crate::battle::obedience::DisobedienceOutcome as Outcome;
                        // In a link battle the disobedience rolls come from the
                        // shared stream (the original draws BattleRandom here).
                        let mut link_rng = self.link_rng.as_mut();
                        let mut draw = || match &mut link_rng {
                            Some(r) => r.next_u8(),
                            None => rand::random::<u8>(),
                        };
                        let outcome = crate::battle::obedience::check_disobedience(
                            level,
                            bs.player_badges,
                            sel_slot.min(3),
                            &moves,
                            &pp,
                            has_disabled,
                            &mut draw,
                        );
                        match outcome {
                            Outcome::Obey => {}
                            Outcome::UseRandomMove(slot) => {
                                // No special text — the mon just uses that move.
                                player_move_id = moves[slot];
                                move_index = slot;
                            }
                            Outcome::Nap(turns) => {
                                bs.player.active_mon_mut().status =
                                    StatusCondition::Sleep(turns);
                                disobedience_msgs.push(format!("{name} began\nto nap!"));
                                player_disobeyed = true;
                            }
                            Outcome::LoafAround => {
                                disobedience_msgs.push(format!("{name} is\nloafing around."));
                                player_disobeyed = true;
                            }
                            Outcome::WontObey => {
                                disobedience_msgs.push(format!("{name} won't\nobey!"));
                                player_disobeyed = true;
                            }
                            Outcome::TurnedAway => {
                                disobedience_msgs.push(format!("{name} turned\naway!"));
                                player_disobeyed = true;
                            }
                            Outcome::IgnoredOrders => {
                                disobedience_msgs.push(format!("{name}\nignored orders!"));
                                player_disobeyed = true;
                            }
                            Outcome::WontObeySelfHit => {
                                disobedience_msgs.push(format!("{name} won't\nobey!"));
                                // HandleSelfConfusionDamage (core.asm:3672): a
                                // typeless 40-power self-hit, always hits, no crit.
                                let dmg = disobedience_self_hit_damage(bs);
                                let mon = bs.player.active_mon_mut();
                                mon.hp = mon.hp.saturating_sub(dmg);
                                disobedience_msgs
                                    .push("It hurt itself in\nits confusion!".to_string());
                                player_disobeyed = true;
                            }
                        }
                    }
                }
            }
        }

        // Resolve "call another move" effects (Metronome / Mirror Move) BEFORE building
        // the MoveData + Fight actions, so both resolution channels (the action's move_
        // and CURRENT_MOVES) agree on the actual move. A failed Mirror Move (no foe last
        // move) resolves to Nothing so no phantom move runs.
        let (mut player_call, mut enemy_call): (Option<&'static str>, Option<&'static str>) =
            (None, None);
        let (mut player_call_failed, mut enemy_call_failed) = (false, false);
        if let Some(ref bs) = self.battle_state {
            let mut default_rng = pokered_rules::runtime::RandBattleRng;
            let rng: &mut dyn jrpg_engine::battle::rng::BattleRng = match self.link_rng.as_mut()
            {
                Some(link) => link,
                None => &mut default_rng,
            };
            let (pid, pl, pf) =
                resolve_called_move(player_move_id, bs.enemy.last_move_used, rng);
            player_move_id = pid;
            player_call = pl;
            player_call_failed = pf;
            // A fired AI item/switch makes the enemy do Nothing this turn, so skip
            // resolving its (unused) called move.
            if !enemy_ai_fired {
                let (eid, el, ef) =
                    resolve_called_move(enemy_move_id, bs.player.last_move_used, rng);
                enemy_move_id = eid;
                enemy_call = el;
                enemy_call_failed = ef;
            }
        }
        // A blocked side (ghost battle) never announces a called move.
        if player_scared {
            player_call = None;
        }
        if ghost_enemy_blocked {
            enemy_call = None;
        }

        // Speed-ordered placement of the decided AI action. Turn order is computed from the
        // SELECTED moves (Gen-1 computes it before `TrainerAI`): enemy-first applies now
        // (the player then hits the healed/boosted/switched enemy); player-first defers the
        // apply until after the player's move (post-block), where a KO cancels it.
        if let Some(action) = ai_action {
            let enemy_first = match self.battle_state.as_mut() {
                Some(bs) => {
                    bs.player.selected_move = player_move_id;
                    bs.enemy.selected_move = enemy_move_id;
                    crate::battle::turn_order::determine_order(bs, rand::random())
                        == crate::battle::turn_order::TurnOrder::EnemyFirst
                }
                None => return,
            };
            if enemy_first {
                self.apply_enemy_ai_action(action, &mut ai_msgs);
                ai_applied_pre = true;
            } else {
                deferred_ai = Some(action);
            }
        }

        let player_move = match MoveData::get(player_move_id) {
            Some(m) => *m,
            None => return,
        };
        let enemy_move = match MoveData::get(enemy_move_id) {
            Some(m) => *m,
            None => return,
        };

        // ════ P6: the whole turn runs through the stack engine ════
        // pick_enemy_move (legacy AI) chose the enemy move above; both movers +
        // ordering + residual + the faint short-circuit then run in ONE StackDriver
        // call, and the resulting TurnLog is translated to the same paginated battle
        // text shown via the existing UI. The legacy execute_move dispatcher is
        // retired from production — it survives only as the differential-test oracle.
        {
            use crate::battle::pokered_rules;
            use jrpg_engine::battle::stack::StackDriver;
            use jrpg_engine::battle::{BattleAction, BattlerRef};

            pokered_rules::install_canonical();
            pokered_rules::clear_current_moves();
            pokered_rules::set_current_move(BattlerRef::PLAYER, player_move);
            pokered_rules::set_current_move(BattlerRef::OPPONENT, enemy_move);

            // A connecting Whirlwind/Roar/Teleport (detected inside the block) ends a
            // WILD battle (escape) via the phase override below.
            let (mut msgs, escape_battle) = {
                use crate::battle::state::status1::CHARGING_UP;
                use crate::battle::state::status2::NEEDS_TO_RECHARGE;
                let bs = self.battle_state.as_mut().unwrap();
                // Each side's PRIOR last-used move (before this turn) — what a Mimic
                // this turn copies from the FOE (this repo's oracle: the foe's LAST
                // move, not a random one).
                let enemy_prior_move = bs.enemy.last_move_used;
                let player_prior_move = bs.player.last_move_used;
                // Prime the turn-start last-move-used so a Disable this turn disables
                // its target's prior move (the engine battler carries no last-move).
                // Filtered by the oracle's PP>0 guard: an out-of-PP last move primes
                // None, so `disable_install` fails to a no-op exactly like apply_disable.
                pokered_rules::set_last_move_live(BattlerRef::PLAYER, disable_target_last_move(&bs.player));
                pokered_rules::set_last_move_live(BattlerRef::OPPONENT, disable_target_last_move(&bs.enemy));
                bs.player.selected_move = player_move_id;
                bs.player.selected_move_index = move_index as u8;
                bs.enemy.selected_move = enemy_move_id;
                bs.enemy.selected_move_index = enemy_move_idx;
                // A mon that ENTERS the turn recharging (Hyper Beam) is forced to skip
                // by PokeredRules::forced_action; the recharge is spent by that skip.
                let p_recharging = bs.player.has_status2(NEEDS_TO_RECHARGE);
                let e_recharging = bs.enemy.has_status2(NEEDS_TO_RECHARGE);
                // A charge move's GATHER turn newly SETS CHARGING_UP (it wasn't set on
                // entry); we narrate "flew up high!" etc. for that transition below.
                let p_was_charging = bs.player.has_status1(CHARGING_UP);
                let e_was_charging = bs.enemy.has_status1(CHARGING_UP);
                // Substitute presence BEFORE the turn — to narrate a doll raised or
                // broken (the write-back below updates HAS_SUBSTITUTE_UP).
                let p_had_sub = bs.player.has_status2(crate::battle::state::status2::HAS_SUBSTITUTE_UP);
                let e_had_sub = bs.enemy.has_status2(crate::battle::state::status2::HAS_SUBSTITUTE_UP);
                let (mut state, mut effects) = pokered_rules::runtime::engine_state_from_legacy(bs);
                let actions = [
                    if player_call_failed || player_scared || player_disobeyed {
                        BattleAction::<pokered_rules::PokeredRules>::Nothing
                    } else {
                        BattleAction::Fight { move_: player_move_id }
                    },
                    if enemy_ai_fired || enemy_call_failed || ghost_enemy_blocked {
                        BattleAction::<pokered_rules::PokeredRules>::Nothing
                    } else {
                        BattleAction::Fight { move_: enemy_move_id }
                    },
                ];
                // Speed order of the (blocked) turns, for placing the ghost-battle
                // texts (PrintGhostText prints each side's line as its turn comes).
                let ghost_enemy_first = ghost_enemy_blocked
                    && crate::battle::turn_order::determine_order(bs, rand::random())
                        == crate::battle::turn_order::TurnOrder::EnemyFirst;
            let mut default_rng = pokered_rules::runtime::RandBattleRng;
            let rng: &mut dyn jrpg_engine::battle::rng::BattleRng = match self.link_rng.as_mut()
            {
                Some(link) => link,
                None => &mut default_rng,
            };
            let (_result, log) = StackDriver::execute_turn_logged(
                &pokered_rules::PokeredRules, &mut state, &mut effects, actions, rng,
            );
                pokered_rules::runtime::apply_engine_to_legacy(bs, &state, &effects);
                // Track the last move each side ACTUALLY used (for Mimic / Mirror
                // Move). The driver logs MoveUsed only for a move that passed the
                // BeforeMove gates and executed, so a blocked / asleep / recharge /
                // forced-Nothing turn correctly leaves last_move_used unchanged.
                for ev in &log.events {
                    if let jrpg_engine::battle::stack::TurnEvent::MoveUsed { actor, move_ } = ev {
                        if actor.side == 0 {
                            bs.player.last_move_used = *move_;
                        } else {
                            bs.enemy.last_move_used = *move_;
                        }
                    }
                }
                // Mimic: if a side executed Mimic this turn, overwrite its Mimic slot
                // with the FOE's prior last-used move, PP→5 (apply_mimic). Fails
                // silently if the foe has no last move.
                {
                    use jrpg_engine::battle::stack::TurnEvent;
                    let used_mimic = |who: BattlerRef| {
                        log.events.iter().any(|ev| {
                            matches!(ev, TurnEvent::MoveUsed { actor, move_ }
                                if *actor == who && *move_ == pokered_data::moves::MoveId::Mimic)
                        }) && !log.events.iter().any(|ev| {
                            matches!(ev, TurnEvent::Missed { actor } if *actor == who)
                        })
                    };
                    if used_mimic(BattlerRef::PLAYER) && enemy_prior_move != pokered_data::moves::MoveId::None {
                        let slot = bs.player.selected_move_index as usize;
                        if slot < 4 {
                            let mon = bs.player.active_mon_mut();
                            mon.moves[slot] = enemy_prior_move;
                            mon.pp[slot] = 5;
                        }
                    }
                    if used_mimic(BattlerRef::OPPONENT) && player_prior_move != pokered_data::moves::MoveId::None {
                        let slot = bs.enemy.selected_move_index as usize;
                        if slot < 4 {
                            let mon = bs.enemy.active_mon_mut();
                            mon.moves[slot] = player_prior_move;
                            mon.pp[slot] = 5;
                        }
                    }
                }
                // Pay Day: each side that connected with Pay Day scatters coins = 2 ×
                // its level into the battle-wide payday pot (read by settlement).
                {
                    use jrpg_engine::battle::stack::TurnEvent;
                    let used_payday = |who: BattlerRef| {
                        log.events.iter().any(|ev| {
                            matches!(ev, TurnEvent::MoveUsed { actor, move_ }
                                if *actor == who && *move_ == pokered_data::moves::MoveId::PayDay)
                        }) && !log.events.iter().any(|ev| {
                            matches!(ev, TurnEvent::Missed { actor } if *actor == who)
                        })
                    };
                    if used_payday(BattlerRef::PLAYER) {
                        let lvl = bs.player.active_mon().level as u32;
                        bs.total_payday_money = bs.total_payday_money.saturating_add(lvl * 2);
                    }
                    if used_payday(BattlerRef::OPPONENT) {
                        let lvl = bs.enemy.active_mon().level as u32;
                        bs.total_payday_money = bs.total_payday_money.saturating_add(lvl * 2);
                    }
                }
                // Whirlwind / Roar / Teleport: a connecting switch/flee move ends a
                // WILD battle. Detect it here (the phase override happens after the
                // block, where `self` is free); keyed on the resolved move's effect so
                // Metronome/Mirror-Move→Teleport also flees.
                let escape = {
                    use jrpg_engine::battle::stack::TurnEvent;
                    log.events.iter().any(|ev| {
                        if let TurnEvent::MoveUsed { actor, move_ } = ev {
                            let is_flee = MoveData::get(*move_).map(|m| m.effect)
                                == Some(pokered_data::moves::MoveEffect::SwitchAndTeleportEffect);
                            let missed = log.events.iter().any(|e2| {
                                matches!(e2, TurnEvent::Missed { actor: a2 } if a2 == actor)
                            });
                            is_flee && !missed
                        } else {
                            false
                        }
                    })
                };
                // Consume the recharge that was spent this turn. Without this, the
                // write-back re-derives NEEDS_TO_RECHARGE from the still-live volatile
                // and the mon would be stuck recharging forever.
                if p_recharging {
                    bs.player.clear_status2(NEEDS_TO_RECHARGE);
                }
                if e_recharging {
                    bs.enemy.clear_status2(NEEDS_TO_RECHARGE);
                }
                let mut m = pokered_rules::runtime::translate_turn(&log, &state, &effects);
                // Metronome / Mirror Move announce themselves before the resolved move's
                // own log lines; a failed Mirror Move shows "But it failed!".
                if let Some(label) = enemy_call {
                    if enemy_call_failed {
                        m.insert(0, "But it failed!".to_string());
                    }
                    m.insert(
                        0,
                        format!(
                            "{} used {}!",
                            pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT),
                            label
                        ),
                    );
                }
                if let Some(label) = player_call {
                    if player_call_failed {
                        m.insert(0, "But it failed!".to_string());
                    }
                    m.insert(
                        0,
                        format!(
                            "{} used {}!",
                            pokered_rules::runtime::display_name(&state, BattlerRef::PLAYER),
                            label
                        ),
                    );
                }
                // Charge move gather turn → append its "flew up high!" style line after
                // the "used X!" line (the strike turn narrates normally via the log).
                if !p_was_charging && bs.player.has_status1(CHARGING_UP) {
                    m.push(format!(
                        "{} {}",
                        pokered_rules::runtime::display_name(&state, BattlerRef::PLAYER),
                        charge_message(player_move_id)
                    ));
                }
                if !e_was_charging && bs.enemy.has_status1(CHARGING_UP) {
                    m.push(format!(
                        "{} {}",
                        pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT),
                        charge_message(enemy_move_id)
                    ));
                }
                // Substitute: narrate a doll raised this turn ("put in a SUBSTITUTE!")
                // and/or broken ("'s SUBSTITUTE broke!"). The absorb itself is silent, as
                // in Gen-1. `sub_created_this_turn` is the per-turn raise signal (set by
                // substitute_install) so BOTH lines show even when a doll is raised AND
                // broken the same turn — which the pre/post HAS_SUBSTITUTE_UP flags alone
                // cannot distinguish.
                {
                    use crate::battle::state::status2::HAS_SUBSTITUTE_UP;
                    for (who, had) in [(BattlerRef::PLAYER, p_had_sub), (BattlerRef::OPPONENT, e_had_sub)] {
                        let side = if who.side == 0 { &bs.player } else { &bs.enemy };
                        let now = side.has_status2(HAS_SUBSTITUTE_UP);
                        let created = pokered_rules::sub_created_this_turn(who);
                        if created {
                            m.push(format!("{} put in a SUBSTITUTE!", pokered_rules::runtime::display_name(&state, who)));
                        }
                        if (had || created) && !now {
                            m.push(format!("{}'s SUBSTITUTE broke!", pokered_rules::runtime::display_name(&state, who)));
                        }
                    }
                }
                // The forced Nothing produces no log event, so narrate the recharge
                // skip game-side (mirrors the "MON must recharge!" original text).
                if e_recharging {
                    m.insert(
                        0,
                        format!(
                            "{} must recharge!",
                            pokered_rules::runtime::display_name(&state, BattlerRef::OPPONENT)
                        ),
                    );
                }
                if p_recharging {
                    m.insert(
                        0,
                        format!(
                            "{} must recharge!",
                            pokered_rules::runtime::display_name(&state, BattlerRef::PLAYER)
                        ),
                    );
                }
                // Ghost-battle texts (_ScaredText / _GetOutText): each blocked
                // side's line, in speed order, leading the turn text (residual
                // damage etc. still applies afterwards, as in the original).
                if ghost_enemy_blocked || player_scared {
                    let mut ghost_msgs: Vec<String> = Vec::new();
                    if player_scared {
                        ghost_msgs.push(format!(
                            "{} is too\nscared to move!",
                            pokered_rules::runtime::display_name(&state, BattlerRef::PLAYER)
                        ));
                    }
                    if ghost_enemy_blocked {
                        ghost_msgs.push("GHOST: Get out...\nGet out...".to_string());
                    }
                    if ghost_enemy_first {
                        ghost_msgs.reverse();
                    }
                    if player_scared || ghost_enemy_first {
                        for (i, gm) in ghost_msgs.into_iter().enumerate() {
                            m.insert(i, gm);
                        }
                    } else {
                        // Only the GHOST's line, player moving first: the player's
                        // status line (fast asleep / frozen) from the log leads.
                        m.extend(ghost_msgs);
                    }
                }
                (m, escape)
            };
            // enemy-FIRST AI: its narration was generated pre-turn → it LEADS the turn text.
            if ai_applied_pre && !ai_msgs.is_empty() {
                let mut combined = std::mem::take(&mut ai_msgs);
                combined.append(&mut msgs);
                msgs = combined;
            }
            // Disobedience narration, placed in speed order: the lines print where
            // the player's (forfeited) move would have happened — leading when the
            // player moves first, trailing the enemy's move text otherwise.
            if !disobedience_msgs.is_empty() {
                // The tie-break roll draws from the shared stream in link
                // battles (original: `call BattleRandom` for the speed tie).
                let order_rand = self.next_battle_random();
                let player_first = self
                    .battle_state
                    .as_ref()
                    .map(|bs| {
                        crate::battle::turn_order::determine_order(bs, order_rand)
                            == crate::battle::turn_order::TurnOrder::PlayerFirst
                    })
                    .unwrap_or(true);
                if player_first {
                    let mut combined = std::mem::take(&mut disobedience_msgs);
                    combined.append(&mut msgs);
                    msgs = combined;
                } else {
                    msgs.extend(disobedience_msgs);
                }
            }
            // player-FIRST AI: apply now (after the player's move) UNLESS the player KO'd
            // the enemy — Gen-1 skips TrainerAI on a KO (the spent charge is moot: a
            // fainting mon re-seeds its count on the next send-out). Narration TRAILS the
            // player's move text.
            if let Some(action) = deferred_ai.take() {
                let enemy_alive = self
                    .battle_state
                    .as_ref()
                    .map(|bs| bs.enemy.active_mon().hp > 0)
                    .unwrap_or(false);
                if enemy_alive {
                    self.apply_enemy_ai_action(action, &mut msgs);
                }
            }
            self.move_menu = None;
            self.sync_display_from_state();
            // A connecting Whirlwind/Roar/Teleport ends a WILD battle (escape); vs a
            // trainer it does nothing (falls through to the normal faint check).
            let next = if escape_battle && self.is_wild {
                msgs.push("Got away safely!".to_string());
                BattlePhase::BattleOver { won: false, escaped: true, wait_frames: 30 }
            } else {
                self.check_faint_after_turn()
            };
            self.append_exp_messages(&mut msgs);
            // Link mode: lead the turn text with narration set by
            // resolve_link_turn (e.g. the remote's switch "sent out" line).
            if !self.link_turn_prefix_msgs.is_empty() {
                let mut prefix = std::mem::take(&mut self.link_turn_prefix_msgs);
                prefix.append(&mut msgs);
                msgs = prefix;
            }
            self.show_text_then(msgs, next);
        }
    }

    fn check_faint_after_turn(&mut self) -> BattlePhase {
        if let Some(ref bs) = self.battle_state {
            let player_fainted = bs.player.active_mon().hp == 0;
            let enemy_fainted = bs.enemy.active_mon().hp == 0;

            if enemy_fainted {
                let alive_enemies = bs
                    .enemy
                    .party
                    .iter()
                    .any(|p| p.hp > 0 && !std::ptr::eq(p, bs.enemy.active_mon()));
                if !alive_enemies {
                    // The remote player is out of mons: battle over, we win.
                    // Both sides resolve this identically, so both end their
                    // battle here without another exchange (the original's
                    // AnyEnemyPokemonAliveCheck → TrainerBattleVictory vs
                    // HandlePlayerBlackOut on the remote side).
                    if self.link_mode {
                        self.link_result = Some(crate::link::protocol::LinkBattleResult::Win);
                    }
                    if bs.battle_type == BattleType::Trainer {
                        return BattlePhase::TrainerVictory {
                            phase: VictoryPhase::DefeatedText,
                            wait_frames: 30,
                            player_won: true,
                        };
                    }
                    return BattlePhase::BattleOver {
                        won: true,
                        escaped: false,
                        wait_frames: 60,
                    };
                }
                self.show_player_pokeballs = false;
                self.show_enemy_pokeballs = false;
                if self.link_mode {
                    // No SHIFT prompt / auto send-out in link battles: the
                    // enemy trainer's replacement is the remote player's
                    // choice and arrives via the next exchanged action
                    // (the original skips the shift prompt for
                    // LINK_STATE_BATTLING, core.asm:1322). Back to the menu.
                    return BattlePhase::PlayerMenu;
                }
                // SHIFT battle style (wOptions BIT_BATTLE_SHIFT clear): in a
                // trainer battle with a player party of 2+, prompt "Enemy X is
                // about to use Y! Will <PLAYER> change #MON?" before the next
                // enemy is sent out (ReplaceFaintedEnemyMon,
                // engine/battle/core.asm:1376-1395). SET style and wild battles
                // skip the prompt (the original returns early for wild battles
                // in HandleEnemyMonFainted; link battles don't exist here).
                if bs.battle_type == BattleType::Trainer
                    && self.battle_style == BattleStyle::Shift
                    && bs.player.party.len() >= 2
                {
                    return BattlePhase::ShiftPrompt;
                }
                return BattlePhase::EnemySendingNext { wait_frames: 30 };
            }

            if player_fainted {
                let alive_player = bs
                    .player
                    .party
                    .iter()
                    .any(|p| p.hp > 0 && !std::ptr::eq(p, bs.player.active_mon()));
                if !alive_player {
                    // We're out of mons: battle over, we lose (the original's
                    // HandlePlayerBlackOut → LinkBattleLostText; the remote
                    // side resolves the mirror and wins).
                    if self.link_mode {
                        self.link_result = Some(crate::link::protocol::LinkBattleResult::Lose);
                    }
                    if bs.battle_type == BattleType::Trainer {
                        return BattlePhase::TrainerVictory {
                            phase: VictoryPhase::TrainerPicScrollIn,
                            wait_frames: 50,
                            player_won: false,
                        };
                    }
                    return BattlePhase::BattleOver {
                        won: false,
                        escaped: false,
                        wait_frames: 60,
                    };
                }
                self.show_player_pokeballs = false;
                self.show_enemy_pokeballs = false;
                return BattlePhase::PlayerFaintSwitch;
            }
        }
        BattlePhase::PlayerMenu
    }

    fn append_exp_messages(&mut self, msgs: &mut Vec<String>) {
        if self.link_mode {
            // No exp in link battles — the original's GainExperience returns
            // immediately for LINK_STATE_BATTLING (experience.asm:1-4). This
            // is also what keeps both sides' party copies identical: a
            // mid-battle level-up would otherwise refresh the active mon's
            // stats on one side only, desyncing every later damage roll.
            return;
        }
        // Reads the *real* enemy HP from battle_state — `self.enemy_hp` is
        // the displayed value, which lags behind during the HP-bar drain.
        let enemy_fainted = self
            .battle_state
            .as_ref()
            .is_some_and(|bs| bs.enemy.active_mon().hp == 0);
        if enemy_fainted {
            let exp_msgs = self.process_exp_gain();
            msgs.extend(exp_msgs);
        }
    }

    fn post_text_transition(&mut self) {
        match &self.phase {
            BattlePhase::PlayerMenu => {
                self.battle_menu = BattleMenuState::new();
            }
            BattlePhase::EnemySendingNext { .. } => {
                self.send_next_enemy();
            }
            BattlePhase::ShiftPrompt => {
                // Cursor defaults to NO (original `ld a, 1 / ld [wCurrentMenuItem], a`).
                self.shift_prompt_yes = false;
                self.current_message = Some(self.shift_prompt_message());
            }
            _ => {}
        }
    }

    /// `TrainerAboutToUseText` (data/text/text_2.asm): "<TRAINER> is about to
    /// use <MON>! / Will <PLAYER> change #MON?" — names the mon the trainer is
    /// about to send out (the next living one, as `send_next_enemy` picks it).
    fn shift_prompt_message(&self) -> String {
        let trainer = self
            .trainer_name
            .clone()
            .or_else(|| self.trainer_class.map(|tc| tc.display_name().to_string()))
            .unwrap_or_else(|| "Enemy".to_string());
        let next_mon = self
            .battle_state
            .as_ref()
            .and_then(|bs| bs.enemy.party.iter().find(|p| p.hp > 0))
            .map(|p| format!("{}", p.species).to_uppercase())
            .unwrap_or_default();
        let player = self.player_name.clone().unwrap_or_else(|| "you".to_string());
        format!(
            "{} is about to use {}!\nWill {} change POKéMON?",
            trainer, next_mon, player
        )
    }

    fn send_next_enemy(&mut self) {
        let next_idx = self
            .battle_state
            .as_ref()
            .and_then(|bs| bs.enemy.party.iter().position(|p| p.hp > 0));
        if let Some(idx) = next_idx {
            if let Some(ref mut bs) = self.battle_state {
                bs.enemy.active_pokemon_index = idx;
                bs.enemy.reset_volatile_status();
                bs.enemy.refresh_unmodified_stats();
            }
            // Each newly sent-out enemy mon resets its trainer-AI budget (wAICount).
            if let Some(tc) = self.trainer_class {
                self.enemy_ai_count = trainer_ai_config(tc).ai_count;
            }
            self.sync_display_from_state();
        }
    }

    fn switch_player_pokemon(&mut self, new_index: usize) {
        if self.link_mode {
            // Link battle: the switch is the local action — defer it until
            // the remote action arrives (the opponent's move hits the
            // incoming mon; there is no free turn in link battles).
            self.link_pending_local_action =
                Some(crate::link::protocol::LinkAction::Switch(new_index as u8));
            self.phase = BattlePhase::LinkWaiting;
            return;
        }
        let msgs = self.apply_player_switch(new_index);
        self.execute_enemy_free_turn_after_switch(msgs);
    }

    /// The shared player-switch mechanics (active switch or faint replacement;
    /// used by the normal free-turn flow and by link-mode resolution).
    /// Returns the "come back / Go!" narration lines.
    fn apply_player_switch(&mut self, new_index: usize) -> Vec<String> {
        if let Some(ref mut bs) = self.battle_state {
            let old_name = format!("{}", bs.player.active_mon().species).to_uppercase();
            bs.player.active_pokemon_index = new_index;
            bs.player.reset_volatile_status();
            bs.player.refresh_unmodified_stats();
            if new_index < 6 {
                bs.party_gain_exp_flags[new_index] = true;
            }
            let new_name = format!("{}", bs.player.active_mon().species).to_uppercase();

            self.sync_display_from_state();

            return vec![
                format!("{}, come back!", old_name),
                format!("Go! {}!", new_name),
            ];
        }
        Vec::new()
    }

    fn execute_enemy_free_turn_after_switch(&mut self, msgs: Vec<String>) {
        self.run_enemy_free_turn_stack(msgs, None);
    }

    /// SHIFT-prompt YES resolution (original `ReplaceFaintedEnemyMon` →
    /// `SwitchPlayerMon`, engine/battle/core.asm:1426-1444): called once the
    /// enemy's next mon is already out — the player's chosen mon switches in
    /// on a free turn, so unlike [`Self::switch_player_pokemon`] no enemy free
    /// turn runs afterwards (the fainted enemy cannot attack; the battle
    /// returns to the main menu, `jp MainInBattleLoop`).
    fn apply_shift_switch(&mut self, new_index: usize) {
        let (old_name, new_name) = match self.battle_state {
            Some(ref mut bs) => {
                let old_name = format!("{}", bs.player.active_mon().species).to_uppercase();
                bs.player.active_pokemon_index = new_index;
                bs.player.reset_volatile_status();
                bs.player.refresh_unmodified_stats();
                if new_index < 6 {
                    bs.party_gain_exp_flags[new_index] = true;
                }
                let new_name = format!("{}", bs.player.active_mon().species).to_uppercase();
                (old_name, new_name)
            }
            None => return,
        };
        self.sync_display_from_state();
        let trainer = self
            .trainer_name
            .clone()
            .or_else(|| self.trainer_class.map(|tc| tc.display_name().to_string()))
            .unwrap_or_else(|| "Enemy".to_string());
        let enemy_name = self
            .battle_state
            .as_ref()
            .map(|bs| format!("{}", bs.enemy.active_mon().species).to_uppercase())
            .unwrap_or_default();
        // Original text order: TrainerSentOutText, then the retreat/send-out
        // pair from RetreatMon + SendOutMon.
        let msgs = vec![
            format!("{} sent out {}!", trainer, enemy_name),
            format!("{}, come back!", old_name),
            format!("Go! {}!", new_name),
        ];
        self.show_player_pokeballs = true;
        self.show_enemy_pokeballs = !self.is_wild;
        self.show_text_then(msgs, BattlePhase::PlayerMenu);
    }

    fn force_switch_player(&mut self, new_index: usize) {
        if self.link_mode {
            // Link battle: the faint replacement is the local action — defer
            // (the remote side picks its action simultaneously, as the
            // original's ChooseNextMon exchange).
            self.link_pending_local_action =
                Some(crate::link::protocol::LinkAction::Switch(new_index as u8));
            self.phase = BattlePhase::LinkWaiting;
            return;
        }
        if let Some(ref mut bs) = self.battle_state {
            bs.player.active_pokemon_index = new_index;
            bs.player.reset_volatile_status();
            bs.player.refresh_unmodified_stats();
            if new_index < 6 {
                bs.party_gain_exp_flags[new_index] = true;
            }
            let new_name = format!("{}", bs.player.active_mon().species).to_uppercase();

            self.sync_display_from_state();
            self.show_player_pokeballs = true;
            self.show_enemy_pokeballs = !self.is_wild;
            self.show_text_then(vec![format!("Go! {}!", new_name)], BattlePhase::PlayerMenu);
        }
    }

    fn has_exp_all(&self) -> bool {
        self.player_bag.has_item(ItemId::ExpAll, 1)
    }

    fn process_exp_gain(&mut self) -> Vec<String> {
        let (defeated_species, defeated_level, is_traded, is_trainer) =
            if let Some(ref bs) = self.battle_state {
                let enemy = bs.enemy.active_mon();
                if enemy.hp > 0 {
                    return vec![];
                }
                (
                    enemy.species,
                    enemy.level,
                    bs.player.active_mon().is_traded,
                    bs.battle_type == BattleType::Trainer,
                )
            } else {
                return vec![];
            };

        let has_exp_all = self.has_exp_all();

        let result = if let Some(ref mut bs) = self.battle_state {
            gain_experience(bs, defeated_species, defeated_level, has_exp_all)
        } else {
            return vec![];
        };

        if let Some(ref mut bs) = self.battle_state {
            let active_idx = bs.player.active_pokemon_index;
            // wCanEvolveFlags: a level-up flags the mon for the post-battle
            // evolution check (experience.asm:257).
            for &idx in &result.leveled_up {
                if idx < bs.party_leveled_up_flags.len() {
                    bs.party_leveled_up_flags[idx] = true;
                }
            }
            if result.leveled_up.contains(&active_idx) {
                bs.player.refresh_unmodified_stats();
                // Mid-battle level-up (experience.asm:233-238): the battle stats
                // are recomputed from the NEW unmodified stats, then
                // `ApplyBadgeStatBoosts` runs once — the accumulated stat-up-
                // glitch rounds are wiped and re-seeded fresh.
                let mon = bs.player.active_mon();
                let raw = [mon.attack, mon.defense, mon.speed, mon.special];
                bs.player.badge_boosted_stats = Some(
                    crate::battle::badge_boosts::initial_boosted_stats(raw, bs.player_badges),
                );
            }
        }

        self.sync_display_from_state();

        let base_exp = get_base_stats(defeated_species)
            .map(|b| b.base_exp)
            .unwrap_or(0);
        let exp_amount = calc_exp_gain(base_exp, defeated_level, is_traded, is_trainer);

        let mut msgs = vec![format!(
            "{} gained {} exp. points!",
            self.player_species, exp_amount
        )];

        for &idx in &result.leveled_up {
            if let Some(ref bs) = self.battle_state {
                let mon = &bs.player.party[idx];
                msgs.push(format!(
                    "{} grew to level {}!",
                    mon.display_name(),
                    mon.level
                ));
            }
        }

        for &(idx, move_id) in &result.new_moves {
            if let Some(ref bs) = self.battle_state {
                let mon = &bs.player.party[idx];
                msgs.push(format!(
                    "{} learned {}!",
                    mon.display_name(),
                    move_display_name(move_id)
                ));
            }
        }

        msgs
    }

    fn calc_prize_money(&self) -> u32 {
        if let Some(ref bs) = self.battle_state {
            if bs.battle_type != BattleType::Trainer {
                return 0;
            }
            if let Some(class) = self.trainer_class {
                let last_level = bs.enemy.party.last().map(|m| m.level).unwrap_or(1);
                return calc_prize_money(class, last_level);
            }
        }
        0
    }

    fn finalize_settlement(&mut self, outcome: BattleOutcome) {
        if let Some(ref mut bs) = self.battle_state {
            let mut settlement =
                settle_battle(bs, outcome, self.trainer_class, self.player_money);
            if self.link_mode {
                // Link battles neither pay nor lose money: the original's
                // TrainerBattleVictory skips the prize flow for
                // LINK_STATE_BATTLING (core.asm:934-937) and HandlePlayerBlackOut
                // returns before any blackout penalty (core.asm:1148-1152).
                // No exp either (GainExperience returns early, experience.asm:1-4),
                // so no settlement exp entries.
                settlement.money_gained = 0;
                settlement.money_lost = 0;
                settlement.exp_entries = Vec::new();
            }
            self.settlement = Some(settlement);
        }
    }

    // ── Link-battle resolution (called by `crate::battle::link_battle_driver`) ──

    /// Draw the next battle random byte: from the shared link stream when a
    /// link battle is active, otherwise from `rand` (the normal path).
    fn next_battle_random(&mut self) -> u8 {
        match &mut self.link_rng {
            Some(rng) => rng.next_u8(),
            None => rand::random::<u8>(),
        }
    }

    /// Map the remote player's wire action to the enemy's move for this turn.
    /// `NoAction` → `None` (the enemy does nothing); an out-of-range or empty
    /// move slot is treated the same (defensive — the wire index mirrors the
    /// remote party's active mon, which both sides keep identical).
    fn link_enemy_move_from_action(
        &self,
        action: crate::link::protocol::LinkAction,
    ) -> Option<(MoveId, u8)> {
        use crate::link::protocol::LinkAction;
        match action {
            LinkAction::UseMove(idx) => self.battle_state.as_ref().and_then(|bs| {
                let mon = bs.enemy.active_mon();
                let idx = idx as usize;
                if idx < 4 && mon.moves[idx] != MoveId::None {
                    Some((mon.moves[idx], idx as u8))
                } else {
                    None
                }
            }),
            LinkAction::Struggle => Some((MoveId::Struggle, 0)),
            _ => None,
        }
    }

    /// Switch the enemy (the remote player's) active mon to `idx`, with the
    /// trainer "sent out" narration. Mirrors the local effect of the remote's
    /// switch (`SwitchEnemyMon` / `EnemySendOutFirstMon`, core.asm:342-369,
    /// 1325-1330): the remote chose the index; we send that mon out on our
    /// side. Returns the narration lines (empty when nothing changed).
    fn switch_enemy_to_link(&mut self, idx: usize) -> Vec<String> {
        let msgs = match self.battle_state {
            Some(ref mut bs) => {
                if idx >= bs.enemy.party.len()
                    || bs.enemy.party[idx].hp == 0
                    || idx == bs.enemy.active_pokemon_index
                {
                    // Defensive no-op: an invalid wire index means the states
                    // have already diverged; no fallback can fix that.
                    Vec::new()
                } else {
                    bs.enemy.active_pokemon_index = idx;
                    bs.enemy.reset_volatile_status();
                    bs.enemy.refresh_unmodified_stats();
                    let name = format!("{}", bs.enemy.active_mon().species).to_uppercase();
                    let trainer = self
                        .trainer_name
                        .clone()
                        .unwrap_or_else(|| "Enemy".to_string());
                    vec![format!("{} sent out {}!", trainer, name)]
                }
            }
            None => Vec::new(),
        };
        self.sync_display_from_state();
        msgs
    }

    /// A link turn where the LOCAL player switched and the remote sent a
    /// move: the remote's move executes against the incoming mon — there is
    /// no free turn in link battles, the opponent's action still lands
    /// (ExecuteEnemyMove runs after the player's menu switch in the original).
    fn run_link_enemy_free_turn(
        &mut self,
        remote: crate::link::protocol::LinkAction,
        msgs: Vec<String>,
    ) {
        match self.link_enemy_move_from_action(remote) {
            Some((move_id, idx)) => {
                self.run_enemy_free_turn_stack(msgs, Some((move_id, idx)));
            }
            None => {
                // The remote did nothing this turn (NoAction / invalid wire
                // data): no attacks — back to the menu after the switch text.
                self.show_text_then(msgs, BattlePhase::PlayerMenu);
            }
        }
    }

    /// Resolve the pending link turn with the remote player's action. Called
    /// by the driver once both sides' actions have been exchanged
    /// (`LinkBattleManager` TurnReady). Both sides run the mirror of this
    /// resolution over the shared RNG stream, so the battle states converge.
    ///
    /// ASM semantics (engine/battle/core.asm `MainInBattleLoop` 340-369 +
    /// `LinkBattleExchangeData` 3008): RUN is always allowed and coordinated
    /// (both ran → DRAW; only the runner loses, the other side sees the enemy
    /// ran and wins — `EnemyRan` 246-267); a switch resolves BEFORE any move
    /// (switches are never free — the opponent's action lands on the incoming
    /// mon); move-vs-move resolves as a normal two-action turn.
    pub fn resolve_link_turn(&mut self, remote_action: crate::link::protocol::LinkAction) {
        use crate::link::protocol::{LinkAction, LinkBattleResult};
        let local_action = match self.link_pending_local_action.take() {
            Some(a) => a,
            None => return,
        };
        // Fresh turn: clear any leftovers from the previous resolution.
        self.link_enemy_move_override = None;
        self.link_enemy_skips_turn = false;
        self.link_turn_prefix_msgs = Vec::new();
        match (local_action, remote_action) {
            (LinkAction::Run, LinkAction::Run) => {
                // Both ran → DRAW (TryRunningFromBattle, core.asm:1599-1606).
                self.finish_link_battle(
                    LinkBattleResult::Draw,
                    vec!["Got away safely!".to_string()],
                );
            }
            (LinkAction::Run, _) => {
                // We ran, the remote didn't → we lose (wBattleResult = 1).
                self.finish_link_battle(
                    LinkBattleResult::Lose,
                    vec!["Got away safely!".to_string()],
                );
            }
            (_, LinkAction::Run) => {
                // The remote ran → we win (`EnemyRan`, core.asm:246-267:
                // "Enemy {nick} ran!", wBattleResult = 0).
                let nick = format!("{}", self.enemy_species).to_uppercase();
                self.finish_link_battle(
                    LinkBattleResult::Win,
                    vec![format!("Enemy {} ran!", nick)],
                );
            }
            (LinkAction::Switch(pa), LinkAction::Switch(ea)) => {
                // Both switched: no attacks this turn.
                let mut msgs = self.apply_player_switch(pa as usize);
                msgs.extend(self.switch_enemy_to_link(ea as usize));
                self.show_text_then(msgs, BattlePhase::PlayerMenu);
            }
            (LinkAction::Switch(pa), remote_move) => {
                // We switched: the remote's move lands on the incoming mon.
                let msgs = self.apply_player_switch(pa as usize);
                self.run_link_enemy_free_turn(remote_move, msgs);
            }
            (local_move, LinkAction::Switch(ea)) => {
                // The remote switched: its replacement comes out first, then
                // our move resolves against it; the remote does not attack
                // (the switch was its action).
                self.link_turn_prefix_msgs = self.switch_enemy_to_link(ea as usize);
                self.link_enemy_skips_turn = true;
                self.execute_turn_with_move(self.link_local_move_index(local_move));
            }
            (local_move, remote_move) => {
                // Normal two-action turn: the remote's move replaces the AI
                // pick (TrainerAI never runs in link battles). NoAction /
                // invalid wire data → the enemy skips its move this turn.
                match self.link_enemy_move_from_action(remote_move) {
                    Some(m) => {
                        self.link_enemy_move_override = Some(m);
                    }
                    None => {
                        self.link_enemy_skips_turn = true;
                    }
                }                self.execute_turn_with_move(self.link_local_move_index(local_move));
            }
        }
    }

    /// The local move index for a wire action (the menu only ever produces
    /// `UseMove`; the other variants are defensive).
    fn link_local_move_index(&self, action: crate::link::protocol::LinkAction) -> usize {
        match action {
            crate::link::protocol::LinkAction::UseMove(idx) => idx as usize,
            _ => 0,
        }
    }

    /// End a link battle with the given outcome and narration. Sets
    /// `link_result` for the driver; the win/lose lines mirror the original's
    /// `TrainerDefeatedText` ("<PLAYER> defeated {trainer}!") and
    /// `LinkBattleLostText` ("<PLAYER> lost to {trainer}!", text_2.asm:904).
    fn finish_link_battle(
        &mut self,
        result: crate::link::protocol::LinkBattleResult,
        mut msgs: Vec<String>,
    ) {
        use crate::link::protocol::LinkBattleResult;
        self.link_result = Some(result);
        let player = self
            .player_name
            .clone()
            .unwrap_or_else(|| "Player".to_string());
        let trainer = self
            .trainer_name
            .clone()
            .unwrap_or_else(|| "Enemy".to_string());
        match result {
            LinkBattleResult::Win => {
                msgs.push(format!("{} defeated {}!", player, trainer));
                self.show_text_then(
                    msgs,
                    BattlePhase::TrainerVictory {
                        phase: VictoryPhase::DefeatedText,
                        wait_frames: 30,
                        player_won: true,
                    },
                );
            }
            LinkBattleResult::Lose => {
                msgs.push(format!("{} lost to {}!", player, trainer));
                self.show_text_then(
                    msgs,
                    BattlePhase::TrainerVictory {
                        phase: VictoryPhase::TrainerPicScrollIn,
                        wait_frames: 50,
                        player_won: false,
                    },
                );
            }
            LinkBattleResult::Draw => {
                self.show_text_then(
                    msgs,
                    BattlePhase::BattleOver {
                        won: false,
                        escaped: true,
                        wait_frames: 30,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod recharge_lifecycle_tests {
    use super::*;
    use crate::battle::state::status1::CHARGING_UP;
    use crate::battle::state::status2::NEEDS_TO_RECHARGE;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    /// End-to-end Fly charge→strike across two live turns via the production loop:
    /// turn 1 gathers (CHARGING_UP set, no damage), turn 2 the strike is FORCED
    /// (the menu is ignored) and lands. Retries around Fly's 95% strike accuracy.
    #[test]
    fn fly_charge_strike_full_lifecycle() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let enemy_hp = |s: &BattleScreen| s.battle_state.as_ref().unwrap().enemy.active_mon().hp;
        let charging =
            |s: &BattleScreen| s.battle_state.as_ref().unwrap().player.has_status1(CHARGING_UP);

        for _attempt in 0..60 {
            let player = vec![mk(Species::Pidgeot, 50, [MoveId::Fly, MoveId::Growl, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Snorlax, 100, [MoveId::Growl, MoveId::Growl, MoveId::Growl, MoveId::Growl])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            let full = enemy_hp(&screen);

            // Turn 1 — gather: CHARGING_UP set, opponent takes no damage.
            screen.execute_turn_with_move(0);
            assert!(charging(&screen), "Fly's gather turn sets CHARGING_UP");
            assert_eq!(enemy_hp(&screen), full, "the gather turn deals no damage");

            // Turn 2 — the strike is FORCED even though we pass a different move index
            // (Growl at slot 1); the charge move re-issues. Retry if Fly missed.
            screen.execute_turn_with_move(1);
            assert!(!charging(&screen), "CHARGING_UP cleared after the strike");
            if enemy_hp(&screen) < full {
                return; // strike connected → charge→strike lifecycle confirmed
            }
        }
        panic!("Fly never connected on the strike across 60 attempts");
    }

    /// End-to-end Thrash across its lock via the production loop: turn 1 locks (menu
    /// ignored thereafter), the forced Thrash keeps dealing damage, and when the
    /// rampage ends the mon self-confuses. Deterministic (Thrash is 100% accuracy);
    /// a low-level attacker keeps the bulky foe alive through the whole lock.
    #[test]
    fn thrash_lock_lifecycle_then_confuses() {
        use crate::battle::state::status1::{CONFUSED, THRASHING_ABOUT};
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let enemy_hp = |s: &BattleScreen| s.battle_state.as_ref().unwrap().enemy.active_mon().hp;
        let thrashing = |s: &BattleScreen| s.battle_state.as_ref().unwrap().player.has_status1(THRASHING_ABOUT);
        let confused = |s: &BattleScreen| s.battle_state.as_ref().unwrap().player.has_status1(CONFUSED);

        let player = vec![mk(Species::Tauros, 10, [MoveId::Thrash, MoveId::Growl, MoveId::None, MoveId::None])];
        let enemy = vec![mk(Species::Snorlax, 100, [MoveId::Growl, MoveId::Growl, MoveId::Growl, MoveId::Growl])];
        let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
        let full = enemy_hp(&screen);

        // Turn 1 — select Thrash: locks + deals damage.
        screen.execute_turn_with_move(0);
        assert!(thrashing(&screen), "Thrash locks the mon");
        assert!(enemy_hp(&screen) < full, "Thrash dealt damage on turn 1");

        // Subsequent turns pass Growl's slot (1); the lock must FORCE Thrash until it
        // exhausts (bounded loop), after which the mon self-confuses.
        for _ in 0..5 {
            if !thrashing(&screen) {
                break;
            }
            let before = enemy_hp(&screen);
            screen.execute_turn_with_move(1); // menu says Growl (power 0); lock forces Thrash
            assert!(enemy_hp(&screen) < before, "the forced Thrash keeps hitting (menu ignored)");
        }
        assert!(!thrashing(&screen), "the rampage ended within its 2–3 turn lock");
        assert!(confused(&screen), "the mon self-confused after the rampage");
    }

    /// last_move_used records the move each side actually executed this turn (the
    /// groundwork Mimic / Mirror Move read). MoveUsed is logged before the accuracy
    /// roll, so this is independent of hit/miss.
    #[test]
    fn last_move_used_tracks_the_executed_move() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let player = vec![mk(Species::Tauros, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let enemy = vec![mk(Species::Snorlax, 50, [MoveId::Growl, MoveId::Growl, MoveId::Growl, MoveId::Growl])];
        let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
        screen.execute_turn_with_move(0);
        let bs = screen.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.last_move_used, MoveId::Tackle, "player's executed move recorded");
        assert_eq!(bs.enemy.last_move_used, MoveId::Growl, "enemy's executed move recorded");
    }

    /// End-to-end Substitute via the production loop: the (faster) player raises a doll
    /// (paying max_hp/4), then the enemy's same-turn attack is absorbed — the player's
    /// REAL HP loses ONLY the sub cost (the doll protects it). Round-trips SubstituteHp
    /// ↔ HAS_SUBSTITUTE_UP through build_volatiles/write_party. Retries until the enemy
    /// connects so the absorb is genuinely exercised.
    #[test]
    fn substitute_live_absorbs_enemy_hit() {
        use crate::battle::state::status2::HAS_SUBSTITUTE_UP;
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        for _attempt in 0..40 {
            // Fast Jolteon raises a doll; a weak low-level Rattata tackles it.
            let player = vec![mk(Species::Jolteon, 50, [MoveId::Substitute, MoveId::Tackle, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Rattata, 5, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            let max_hp = screen.battle_state.as_ref().unwrap().player.active_mon().max_hp;
            let cost = max_hp / 4;

            screen.execute_turn_with_move(0); // Substitute
            let bs = screen.battle_state.as_ref().unwrap();
            assert!(bs.player.has_status2(HAS_SUBSTITUTE_UP), "the doll is up after Substitute");
            assert_eq!(
                bs.player.active_mon().hp,
                max_hp - cost,
                "real HP lost ONLY the sub cost — the enemy's hit went to the doll"
            );
            if (bs.player.substitute_hp as u16) < cost {
                return; // the enemy connected → the doll genuinely absorbed a hit
            }
        }
        panic!("the enemy never connected on the substitute across 40 attempts");
    }

    /// The enemy's free move (after a failed escape) now runs on the STACK, not the
    /// retired legacy execute_move: the move is tracked as last_move_used and its
    /// damage persists to the player's real HP.
    #[test]
    fn enemy_free_turn_runs_on_the_stack() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        for _attempt in 0..30 {
            let player = vec![mk(Species::Snorlax, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Tauros, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            let p_hp0 = screen.battle_state.as_ref().unwrap().player.active_mon().hp;

            screen.execute_enemy_free_turn(); // the enemy's post-failed-escape attack

            let bs = screen.battle_state.as_ref().unwrap();
            assert_eq!(bs.enemy.last_move_used, MoveId::Tackle, "the enemy's move ran on the stack (last_move tracked)");
            if bs.player.active_mon().hp < p_hp0 {
                return; // damage persisted → the free turn ran on the stack end-to-end
            }
        }
        panic!("the enemy's free-turn Tackle never connected across 30 attempts");
    }

    /// End-to-end Conversion via the production loop: the user's types change to the
    /// target's, persisted into the battle-only `conversion_type1/2` fields (the full
    /// install → write_party → build_volatiles round-trip). Retries around Conversion's
    /// accuracy so a stray miss doesn't flake.
    #[test]
    fn conversion_live_copies_target_types() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        for _attempt in 0..30 {
            let player = vec![mk(Species::Porygon, 50, [MoveId::Conversion, MoveId::Tackle, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Gengar, 50, [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            let (et1, et2) = {
                let e = screen.battle_state.as_ref().unwrap().enemy.active_mon();
                (e.type1, e.type2)
            };
            screen.execute_turn_with_move(0); // Conversion
            let bs = screen.battle_state.as_ref().unwrap();
            if bs.player.conversion_type1.is_some() {
                assert_eq!(bs.player.conversion_type1, Some(et1), "user's type1 becomes the target's");
                assert_eq!(bs.player.conversion_type2, Some(et2), "user's type2 becomes the target's");
                assert_eq!(bs.player.active_mon().species, Species::Porygon, "species unchanged");
                return;
            }
        }
        panic!("Conversion never connected across 30 attempts");
    }

    /// End-to-end Disable via the production loop: after the enemy has used a move, a
    /// (slower) Disable locks that move — the legacy `disabled_move` slot the player
    /// menu + enemy AI read is set through the round-trip. The slower user means the
    /// faster enemy's same-turn move does not decrement the fresh counter, so the slot
    /// persists. Retries around Disable's 55% accuracy.
    #[test]
    fn disable_live_sets_disabled_move() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        for _attempt in 0..40 {
            // Slowpoke (slow) Disables Jolteon (fast); Jolteon only knows Growl.
            let player = vec![mk(Species::Slowpoke, 50, [MoveId::Disable, MoveId::Tackle, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Jolteon, 50, [MoveId::Growl, MoveId::None, MoveId::None, MoveId::None])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);

            // Turn 1: player Tackles (slot 1); enemy uses Growl → its last move.
            screen.execute_turn_with_move(1);
            assert_eq!(
                screen.battle_state.as_ref().unwrap().enemy.last_move_used,
                MoveId::Growl,
                "enemy's Growl is recorded as its last move"
            );

            // Turn 2: player uses Disable (slot 0) on the enemy's Growl.
            screen.execute_turn_with_move(0);
            let bs = screen.battle_state.as_ref().unwrap();
            if bs.enemy.disabled_move != 0 {
                assert_eq!(bs.enemy.disabled_move, 1, "Growl (enemy move slot 1) is disabled");
                assert!(bs.enemy.disabled_turns_left >= 1, "the disable has turns remaining");
                return;
            }
        }
        panic!("Disable never connected across 40 attempts");
    }

    /// The Disable last-move PP guard (`disable_target_last_move`, primed into
    /// LAST_MOVE_LIVE): the oracle disables a last move only when its slot still has
    /// PP; an out-of-PP last move yields `None` so `disable_install` no-ops like
    /// `apply_disable`'s StatusFailed.
    #[test]
    fn disable_target_last_move_respects_pp() {
        let p = create_pokemon_with_moves(
            Species::Snorlax,
            50,
            [0xFF, 0xFF],
            [MoveId::BodySlam, MoveId::Tackle, MoveId::None, MoveId::None],
        )
        .unwrap();
        let mut b = crate::battle::state::new_battler_state(vec![p]);
        b.last_move_used = MoveId::BodySlam;
        assert_eq!(disable_target_last_move(&b), MoveId::BodySlam, "a last move with PP is disable-able");
        b.party[0].pp[0] = 0; // drain BodySlam's PP
        assert_eq!(disable_target_last_move(&b), MoveId::None, "an out-of-PP last move is not disable-able");
        b.party[0].pp[0] = 5;
        b.last_move_used = MoveId::None;
        assert_eq!(disable_target_last_move(&b), MoveId::None, "no last move → None");
    }

    /// Mimic copies the foe's prior last-used move into the Mimic slot (PP→5), and it
    /// is then selectable + fires. Turn 1 Mimic fails (foe has no last move yet);
    /// turn 2 copies the foe's Swift; turn 3 the copied Swift deals damage. Swift is
    /// used so the copied move connects deterministically (it never misses).
    #[test]
    fn mimic_copies_and_fires_foe_last_move() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let player = vec![mk(Species::Snorlax, 50, [MoveId::Mimic, MoveId::None, MoveId::None, MoveId::None])];
        let enemy = vec![mk(Species::Snorlax, 50, [MoveId::Swift, MoveId::Swift, MoveId::Swift, MoveId::Swift])];
        let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
        let enemy_hp = |s: &BattleScreen| s.battle_state.as_ref().unwrap().enemy.active_mon().hp;

        // Turn 1 — player Mimics, but the foe has no last move yet → slot 0 stays Mimic.
        screen.execute_turn_with_move(0);
        assert_eq!(
            screen.battle_state.as_ref().unwrap().player.active_mon().moves[0],
            MoveId::Mimic,
            "Mimic fails on turn 1 (foe had no last move)"
        );

        // Turn 2 — player Mimics again; the foe's prior move is now Swift → copy it.
        screen.execute_turn_with_move(0);
        {
            let mon = screen.battle_state.as_ref().unwrap().player.active_mon();
            assert_eq!(mon.moves[0], MoveId::Swift, "Mimic copies the foe's last move into the slot");
            assert_eq!(mon.pp[0], 5, "copied move's PP set to 5");
        }

        // Turn 3 — the copied Swift is selectable in slot 0 and (never missing) hits.
        let before = enemy_hp(&screen);
        screen.execute_turn_with_move(0);
        assert!(enemy_hp(&screen) < before, "the Mimic'd move fires and hits");
    }

    /// Metronome's pick rejection-samples (core.asm:5013-5037): 0, ≥ 0xA5
    /// (STRUGGLE), and 0x76 (METRONOME) are re-drawn; every accepted byte maps
    /// to itself — uniform over the 163 legal move ids.
    #[test]
    fn metronome_pick_skips_metronome() {
        use jrpg_engine::battle::rng::ScriptedRng;
        // Every accepted byte (1..=0xA4 except 0x76) is picked as-is, in one draw.
        for roll in 0u8..=255 {
            let mut rng = ScriptedRng::new(vec![roll, 1]);
            let picked = super::metronome_pick(&mut rng);
            assert_ne!(picked, MoveId::Metronome, "roll {roll} picked Metronome");
            assert_ne!(picked, MoveId::None, "roll {roll} picked None");
            assert_ne!(picked, MoveId::Struggle, "roll {roll} picked Struggle");
            if roll == 0 || roll >= 0xA5 || roll == 0x76 {
                assert_eq!(rng.consumed(), 2, "roll {roll} should be rejected and re-drawn");
            } else {
                assert_eq!(rng.consumed(), 1, "roll {roll} should be accepted on the first draw");
                assert_eq!(picked as u8, roll, "accepted ids map to themselves");
            }
        }
    }

    /// Metronome resolves to a different move and executes it (the resolved move is the
    /// one logged as used). Retries around the rare Metronome→Mirror-Move-fail pick.
    #[test]
    fn metronome_resolves_and_executes_a_real_move() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        for _ in 0..25 {
            let player = vec![mk(Species::Snorlax, 50, [MoveId::Metronome, MoveId::None, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Snorlax, 50, [MoveId::Tackle, MoveId::Tackle, MoveId::Tackle, MoveId::Tackle])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            screen.execute_turn_with_move(0);
            let last = screen.battle_state.as_ref().unwrap().player.last_move_used;
            assert_ne!(last, MoveId::Metronome, "Metronome never resolves to itself");
            if last != MoveId::None {
                return; // a real move executed under Metronome
            }
        }
        panic!("Metronome never resolved to an executing move in 25 tries");
    }

    /// Mirror Move replays the foe's last-used move. Turn 1 fails (foe has no last
    /// move); turn 2 replays the foe's Swift and deals damage.
    #[test]
    fn mirror_move_replays_foe_last_move() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let player = vec![mk(Species::Snorlax, 50, [MoveId::MirrorMove, MoveId::None, MoveId::None, MoveId::None])];
        let enemy = vec![mk(Species::Snorlax, 50, [MoveId::Swift, MoveId::Swift, MoveId::Swift, MoveId::Swift])];
        let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
        let enemy_hp = |s: &BattleScreen| s.battle_state.as_ref().unwrap().enemy.active_mon().hp;

        // Turn 1 — Mirror Move fails (foe has no last move) → the player does nothing.
        screen.execute_turn_with_move(0);
        let after_t1 = enemy_hp(&screen);

        // Turn 2 — Mirror Move replays the foe's Swift (never misses) → damages the foe.
        screen.execute_turn_with_move(0);
        assert!(enemy_hp(&screen) < after_t1, "Mirror Move replayed the foe's Swift");
    }

    /// Pay Day scatters coins equal to 2× the user's level into the battle payday pot.
    /// Retries around Pay Day's 1/256 miss (a missed Pay Day scatters nothing).
    #[test]
    fn pay_day_scatters_coins() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        for _ in 0..6 {
            let player = vec![mk(Species::Meowth, 30, [MoveId::PayDay, MoveId::None, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Snorlax, 50, [MoveId::Splash, MoveId::Splash, MoveId::Splash, MoveId::Splash])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            screen.execute_turn_with_move(0);
            let payday = screen.battle_state.as_ref().unwrap().total_payday_money;
            if payday == 60 {
                return; // 2 × level 30
            }
            assert_eq!(payday, 0, "Pay Day is all-or-nothing (60 on hit, 0 on the rare miss)");
        }
        panic!("Pay Day never scattered coins across 6 tries");
    }

    /// Teleport (a SwitchAndTeleport move) ends a WILD battle — the turn's next phase
    /// is BattleOver{escaped}.
    #[test]
    fn teleport_flees_a_wild_battle() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let player = vec![mk(Species::Abra, 30, [MoveId::Teleport, MoveId::None, MoveId::None, MoveId::None])];
        let enemy = vec![mk(Species::Snorlax, 50, [MoveId::Splash, MoveId::Splash, MoveId::Splash, MoveId::Splash])];
        let mut screen = BattleScreen::from_parties(true, &player, &enemy, None); // is_wild = true
        screen.execute_turn_with_move(0);
        let escaped = match &screen.phase {
            BattlePhase::ShowingText { next_phase, .. } => {
                matches!(**next_phase, BattlePhase::BattleOver { escaped: true, .. })
            }
            BattlePhase::BattleOver { escaped: true, .. } => true,
            _ => false,
        };
        assert!(escaped, "Teleport flees the wild battle (next phase is BattleOver escaped)");
    }

    /// End-to-end Hyper Beam recharge across three live turns via the production
    /// `execute_turn_with_move`: turn 1 hits + arms the recharge, turn 2 the mon is
    /// forced to skip (opponent takes no player damage) and the flag is consumed,
    /// turn 3 the mon acts again. The enemy only knows Growl (power 0) so it never
    /// KOs the player and never damages the opponent's HP we watch.
    ///
    /// `RandBattleRng` is the real (non-seedable) RNG and Hyper Beam is 90% accuracy,
    /// so we RETRY the scenario until a run where both Hyper Beams connect — the
    /// recharge only arms on a hit, which is itself the faithful Gen-1 behaviour (a
    /// missed Hyper Beam needs no recharge). The deterministic recharge mechanics are
    /// covered separately by the ScriptedRng engine tests.
    #[test]
    fn hyper_beam_recharge_full_lifecycle() {
        let mk = |sp, lvl, moves: [MoveId; 4]| {
            create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
        };
        let enemy_hp = |s: &BattleScreen| s.battle_state.as_ref().unwrap().enemy.active_mon().hp;
        let player_recharging =
            |s: &BattleScreen| s.battle_state.as_ref().unwrap().player.has_status2(NEEDS_TO_RECHARGE);

        for _attempt in 0..60 {
            // Player: level-50 Snorlax with Hyper Beam at slot 0. Enemy: a bulky
            // level-100 Snorlax that only Growls (survives Hyper Beam; deals no HP damage).
            let player = vec![mk(Species::Snorlax, 50, [MoveId::HyperBeam, MoveId::Growl, MoveId::None, MoveId::None])];
            let enemy = vec![mk(Species::Snorlax, 100, [MoveId::Growl, MoveId::Growl, MoveId::Growl, MoveId::Growl])];
            let mut screen = BattleScreen::from_parties(true, &player, &enemy, None);
            let full = enemy_hp(&screen);
            assert!(!player_recharging(&screen), "not recharging before any move");

            // Turn 1 — Hyper Beam. Retry the whole scenario if it missed (no recharge).
            screen.execute_turn_with_move(0);
            if !player_recharging(&screen) {
                continue; // Hyper Beam missed → no recharge (faithful) → try again
            }
            let after_t1 = enemy_hp(&screen);
            assert!(after_t1 < full, "Hyper Beam dealt damage on turn 1");
            assert!(after_t1 > 0, "the bulky enemy survived Hyper Beam (a KO would skip recharge)");

            // Turn 2 — forced skip: opponent takes NO player damage, recharge consumed.
            // Deterministic given the recharge was armed above.
            screen.execute_turn_with_move(0);
            assert_eq!(enemy_hp(&screen), after_t1, "recharge turn dealt no damage (forced skip)");
            assert!(!player_recharging(&screen), "recharge consumed after the skip turn");

            // Turn 3 — the mon can act again. Confirm via a connecting Hyper Beam;
            // if it missed, retry the scenario for a clean end-to-end confirmation.
            screen.execute_turn_with_move(0);
            if enemy_hp(&screen) < after_t1 {
                return; // full lifecycle confirmed
            }
        }
        panic!("Hyper Beam never connected on both turns across 60 attempts");
    }
}

/// Trainer-AI item / X-item / Guard Spec / switch wiring on the live enemy turn
/// (`take_enemy_ai_action` → `enemy_ai_action_inner`). The pure per-class decision
/// (`execute_ai_action`) is covered in `trainer_ai::ai_action`; these prove the
/// game-side APPLICATION: the mutation lands on `bs.enemy`, the AI budget lifecycle,
/// the switch target/refund, the guards, and one end-to-end pass through the
/// production loop.
#[cfg(test)]
mod trainer_ai_action_tests {
    use super::*;
    use crate::battle::stat_stages::StatIndex;
    use crate::battle::state::status1::CHARGING_UP;
    use crate::battle::state::status2::PROTECTED_BY_MIST;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;
    use pokered_data::trainer_data::TrainerClass;

    fn mk(sp: Species, lvl: u8, moves: [MoveId; 4]) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
    }

    fn tackler() -> Vec<crate::battle::state::Pokemon> {
        vec![mk(
            Species::Rattata,
            5,
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )]
    }

    /// A trainer battle of `class` against `enemy`, player = a harmless lvl-5 Tackler.
    fn trainer_battle(class: TrainerClass, enemy: Vec<crate::battle::state::Pokemon>) -> BattleScreen {
        BattleScreen::from_parties(false, &tackler(), &enemy, Some(class))
    }

    fn enemy_status(s: &BattleScreen) -> StatusCondition {
        s.battle_state.as_ref().unwrap().enemy.active_mon().status
    }
    fn enemy_hp(s: &BattleScreen) -> u16 {
        s.battle_state.as_ref().unwrap().enemy.active_mon().hp
    }
    fn enemy_def_stage(s: &BattleScreen) -> i8 {
        s.battle_state.as_ref().unwrap().enemy.stat_stages.get(StatIndex::Defense)
    }

    fn poisoned_onix() -> Vec<crate::battle::state::Pokemon> {
        let mut e = vec![mk(Species::Onix, 30, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        e[0].status = StatusCondition::Poison;
        e
    }

    /// Brock's Full Heal cures the active enemy's status. Deterministic: `ai_brock`
    /// ignores the RNG byte and fires iff the mon is statused.
    #[test]
    fn brock_full_heal_cures_status() {
        let mut screen = trainer_battle(TrainerClass::Brock, poisoned_onix());
        assert_eq!(screen.enemy_ai_count, 5, "Brock's wAICount seeds from the class");
        let mut msgs = Vec::new();
        let fired = screen.enemy_ai_action_inner(200, &mut msgs);
        assert!(fired);
        assert!(enemy_status(&screen).is_none(), "Full Heal cured the status");
        assert_eq!(screen.enemy_ai_count, 4, "one AI charge spent");
        assert!(msgs.iter().any(|m| m.contains("used FULL HEAL!")), "{msgs:?}");
    }

    /// CooltrainerF Hyper-Potions a low-HP mon back to full. Deterministic: the
    /// low-HP branch fires regardless of the RNG.
    #[test]
    fn cooltrainer_f_heals_low_hp() {
        let mut enemy = vec![mk(Species::Nidoking, 40, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let max = enemy[0].max_hp;
        enemy[0].hp = max / 6; // below the max/5 heal threshold
        let mut screen = trainer_battle(TrainerClass::CooltrainerF, enemy);
        let mut msgs = Vec::new();
        assert!(screen.enemy_ai_action_inner(0, &mut msgs));
        assert_eq!(enemy_hp(&screen), max, "Hyper Potion (+200) caps at max HP");
        assert!(msgs.iter().any(|m| m.contains("used HYPER POTION!")), "{msgs:?}");
    }

    /// A Full Restore (Rival3, `heal_amount == 0`) both heals to full AND cures status.
    #[test]
    fn full_restore_heals_and_cures() {
        let mut enemy = vec![mk(Species::Charizard, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let max = enemy[0].max_hp;
        enemy[0].hp = max / 6; // below max/5
        enemy[0].status = StatusCondition::Burn;
        let mut screen = trainer_battle(TrainerClass::Rival3, enemy);
        let mut msgs = Vec::new();
        assert!(screen.enemy_ai_action_inner(0, &mut msgs)); // rand < 33 → heal branch
        assert_eq!(enemy_hp(&screen), max, "Full Restore heals to full");
        assert!(enemy_status(&screen).is_none(), "Full Restore also cured the burn");
        assert!(msgs.iter().any(|m| m.contains("used FULL RESTORE!")), "{msgs:?}");
    }

    /// Bruno's X Defend raises the enemy defense STAGE (which the round-trip feeds to
    /// the damage formula) and narrates both the item use and the stat rise.
    #[test]
    fn bruno_x_defend_raises_defense() {
        let enemy = vec![mk(Species::Onix, 40, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let mut screen = trainer_battle(TrainerClass::Bruno, enemy);
        let mut msgs = Vec::new();
        assert!(screen.enemy_ai_action_inner(0, &mut msgs)); // rand < 64 → X Defend
        assert_eq!(enemy_def_stage(&screen), 1, "X Defend raised the defense stage");
        assert!(msgs.iter().any(|m| m.contains("used X DEFEND!")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("DEFENSE rose!")), "{msgs:?}");
    }

    /// A consulted DoNothing STILL spends a charge — Gen-1 decrements wAICount once per
    /// consultation (in the TrainerAI wrapper), not per action.
    #[test]
    fn bruno_idle_still_spends_a_charge() {
        let enemy = vec![mk(Species::Onix, 40, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let mut screen = trainer_battle(TrainerClass::Bruno, enemy);
        let start = screen.enemy_ai_count;
        let mut msgs = Vec::new();
        assert!(!screen.enemy_ai_action_inner(200, &mut msgs), "rand >= 64 → Bruno idles (no X Defend)");
        assert_eq!(screen.enemy_ai_count, start - 1, "the per-turn consultation spent a charge");
        assert_eq!(enemy_def_stage(&screen), 0, "no stat change");
    }

    /// AI narration uses the canonical display spelling (`species_name`), matching the
    /// rest of the turn's log, for glyph species like MR.MIME — not the strum variant.
    #[test]
    fn ai_narration_uses_canonical_species_name() {
        let enemy = vec![mk(Species::MrMime, 40, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let mut screen = trainer_battle(TrainerClass::Bruno, enemy);
        let mut msgs = Vec::new();
        assert!(screen.enemy_ai_action_inner(0, &mut msgs)); // X Defend → "...'s DEFENSE rose!"
        let canonical = pokered_data::lang_data::species_name(Species::MrMime, false).to_uppercase();
        let strum = format!("{}", Species::MrMime).to_uppercase();
        assert_ne!(canonical, strum, "MR.MIME must differ from the strum name to make this test meaningful");
        assert!(msgs.iter().any(|m| m.contains(&canonical)), "canonical spelling expected: {msgs:?}");
        assert!(!msgs.iter().any(|m| m.contains(&strum)), "must not use the strum variant name: {msgs:?}");
    }

    /// Giovanni's Guard Spec sets the Mist volatile (`PROTECTED_BY_MIST`), which the
    /// existing consumer uses to veto incoming stat drops.
    #[test]
    fn giovanni_guard_spec_sets_mist() {
        let enemy = vec![mk(Species::Rhyhorn, 40, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        let mut screen = trainer_battle(TrainerClass::Giovanni, enemy);
        let mut msgs = Vec::new();
        assert!(screen.enemy_ai_action_inner(0, &mut msgs)); // rand < 64 → Guard Spec
        assert!(
            screen.battle_state.as_ref().unwrap().enemy.has_status2(PROTECTED_BY_MIST),
            "Guard Spec set the Mist protection"
        );
        assert!(msgs.iter().any(|m| m.contains("used GUARD SPEC.!")), "{msgs:?}");
    }

    /// A switch changes the active enemy and resets its AI budget (a new mon = fresh
    /// wAICount). Deterministic: `ai_juggler(0)` switches.
    #[test]
    fn juggler_switches_with_two_mons() {
        let enemy = vec![
            mk(Species::Snorlax, 30, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]),
            mk(Species::Tauros, 30, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]),
        ];
        let mut screen = trainer_battle(TrainerClass::Juggler, enemy);
        assert_eq!(screen.enemy_ai_count, 3);
        let mut msgs = Vec::new();
        assert!(screen.enemy_ai_action_inner(0, &mut msgs)); // rand < 64 → Switch
        assert_eq!(
            screen.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 1,
            "switched to the first alive non-active mon"
        );
        assert_eq!(screen.enemy_ai_count, 3, "a fresh mon resets wAICount");
        assert!(msgs.iter().any(|m| m.contains("withdrew")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("sent out")), "{msgs:?}");
    }

    /// A rolled switch with no living target: the enemy attacks normally, but the charge
    /// STAYS spent (Gen-1's wrapper decrement + AISwitchIfEnoughMons early-out, no refund).
    #[test]
    fn switch_with_no_target_still_spends_a_charge() {
        let mut enemy = vec![
            mk(Species::Snorlax, 30, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]),
            mk(Species::Tauros, 30, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]),
        ];
        enemy[1].hp = 0; // the only alternative is fainted
        let mut screen = trainer_battle(TrainerClass::Juggler, enemy);
        let start = screen.enemy_ai_count;
        let mut msgs = Vec::new();
        assert!(!screen.enemy_ai_action_inner(0, &mut msgs), "no valid target → no action");
        assert_eq!(
            screen.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 0,
            "active mon unchanged"
        );
        assert_eq!(screen.enemy_ai_count, start - 1, "the consultation's charge stays spent (no refund)");
        assert!(msgs.is_empty(), "no narration for a skipped switch");
    }

    /// No action once the budget is exhausted.
    #[test]
    fn no_action_when_count_zero() {
        let mut screen = trainer_battle(TrainerClass::Brock, poisoned_onix());
        screen.enemy_ai_count = 0;
        let mut msgs = Vec::new();
        assert!(!screen.enemy_ai_action_inner(0, &mut msgs));
        assert!(matches!(enemy_status(&screen), StatusCondition::Poison), "no heal at 0 charges");
    }

    /// Wild battles carry no trainer AI.
    #[test]
    fn no_action_for_wild() {
        let mut screen = BattleScreen::from_parties(true, &tackler(), &poisoned_onix(), None);
        let mut msgs = Vec::new();
        assert!(!screen.enemy_ai_action_inner(0, &mut msgs), "wild → no trainer AI");
        assert_eq!(screen.enemy_ai_count, 0, "wild seeds a zero budget");
    }

    /// The conservative guard: no AI action while the enemy is locked / charging /
    /// recharging (an item/switch mid-forced-move is not modelled).
    #[test]
    fn no_action_when_enemy_locked() {
        let mut screen = trainer_battle(TrainerClass::Brock, poisoned_onix());
        let start = screen.enemy_ai_count;
        screen.battle_state.as_mut().unwrap().enemy.set_status1(CHARGING_UP);
        let mut msgs = Vec::new();
        assert!(!screen.enemy_ai_action_inner(0, &mut msgs), "locked/charging enemy skips the AI");
        assert!(matches!(enemy_status(&screen), StatusCondition::Poison));
        assert_eq!(screen.enemy_ai_count, start, "a guarded (skipped) turn spends no charge");
    }

    /// End-to-end through the production loop: a bulky poisoned enemy (Brock) cures
    /// its own status via Full Heal instead of attacking, and the cure survives the
    /// legacy↔engine round-trip. Deterministic — Brock ignores the RNG and the
    /// player's Tackle inflicts no status and cannot KO the bulky Snorlax.
    #[test]
    fn brock_cures_status_through_live_loop() {
        let mut enemy = vec![mk(Species::Snorlax, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        enemy[0].status = StatusCondition::Poison;
        let mut screen = trainer_battle(TrainerClass::Brock, enemy);
        assert_eq!(screen.enemy_ai_count, 5);

        screen.execute_turn_with_move(0); // player Tackle; enemy uses Full Heal

        let bs = screen.battle_state.as_ref().unwrap();
        assert!(bs.enemy.active_mon().status.is_none(), "status cured through the live loop");
        assert!(bs.enemy.active_mon().hp > 0, "the bulky enemy survived the Tackle");
        assert_eq!(screen.enemy_ai_count, 4, "one AI charge spent on the live turn");
    }

    /// Build a `CooltrainerF` trainer battle with an enemy at `hp`/`max_hp`/`speed` and a
    /// player of `speed`. CooltrainerF Hyper-Potions (rand-independently) when the enemy is
    /// below max/5 HP, so the AI decision is deterministic — only the placement varies.
    fn speed_scenario(
        enemy_hp: u16,
        enemy_max: u16,
        enemy_speed: u16,
        player_species: Species,
        player_level: u8,
        player_speed: u16,
    ) -> BattleScreen {
        let mut enemy = vec![mk(Species::Snorlax, 20, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None])];
        enemy[0].max_hp = enemy_max;
        enemy[0].hp = enemy_hp;
        enemy[0].speed = enemy_speed;
        let mut player = mk(player_species, player_level, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
        player.speed = player_speed;
        BattleScreen::from_parties(false, &[player], &enemy, Some(TrainerClass::CooltrainerF))
    }

    /// SPEED-ORDERED (the review's #1 fix): a FASTER player that KOs the enemy CANCELS the
    /// AI heal — Gen-1 runs `TrainerAI` only after the player's move and skips it on a KO,
    /// so a would-be KO is no longer negated. With a pre-turn heal the (bulky-max) enemy
    /// would be full and never faint, so a KO here proves the heal was DEFERRED + cancelled.
    #[test]
    fn player_first_ko_cancels_ai_heal() {
        for _ in 0..80 {
            // enemy 1/300 HP (below max/5 → would heal), slow; fast player.
            let mut screen = speed_scenario(1, 300, 10, Species::Electrode, 50, 200);
            screen.execute_turn_with_move(0);
            if enemy_hp(&screen) == 0 {
                return; // KO landed → the deferred heal was cancelled (not applied pre-turn)
            }
            // else the player's Tackle missed (enemy survived → heal applied); retry.
        }
        panic!("player never KO'd — a pre-turn heal to full max would prevent the faint");
    }

    /// SPEED-ORDERED: a FASTER but WEAK player does not KO; the enemy survives and the AI
    /// heal applies AFTER the player's chip (the deferred path fires).
    #[test]
    fn player_first_non_ko_heals_after_move() {
        for _ in 0..40 {
            // enemy 30/300 HP (below max/5), slow; fast but weak lvl-3 player.
            let mut screen = speed_scenario(30, 300, 10, Species::Rattata, 3, 200);
            screen.execute_turn_with_move(0);
            let hp = enemy_hp(&screen);
            if hp == 0 {
                continue; // a rare crit KO — retry
            }
            assert!(hp >= 200, "player-first non-KO: the Hyper Potion (+200) applied after the chip (hp={hp})");
            return;
        }
        panic!("enemy KO'd on every attempt (unexpected for a lvl-3 attacker)");
    }

    /// SPEED-ORDERED: a FASTER enemy heals BEFORE the (slow, weak) player's move — the
    /// player then hits the healed enemy, so it survives at high HP. If the heal were
    /// wrongly deferred, the weak player would KO the 1-HP enemy instead.
    #[test]
    fn enemy_first_heals_before_player_move() {
        // enemy 1/300 HP (below max/5), fast; slow weak lvl-3 player.
        let mut screen = speed_scenario(1, 300, 200, Species::Rattata, 3, 10);
        screen.execute_turn_with_move(0);
        let hp = enemy_hp(&screen);
        assert!(hp > 100, "enemy-first: the heal (1 → 201) applied before the player's chip (hp={hp})");
    }
}
/// Pokémon-Tower GHOST (no Silph Scope): an unidentified, uncatchable wild encounter.
#[cfg(test)]
mod ghost_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::items::ItemId;
    use pokered_data::species::Species;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
    }

    /// Ordinary battles are never ghosts.
    #[test]
    fn default_battle_is_not_ghost() {
        assert!(!BattleScreen::new(true).is_ghost);
        let s = BattleScreen::from_parties(
            true,
            &[mk(Species::Charmander, 5)],
            &[mk(Species::Gastly, 20)],
            None,
        );
        assert!(!s.is_ghost);
    }

    /// A ball thrown at a GHOST is dodged — no capture, even at 1 HP (a near-guaranteed
    /// catch normally), proving the guard prevents it rather than luck.
    #[test]
    fn ghost_ball_is_dodged_no_capture() {
        let player = vec![mk(Species::Charmander, 20)];
        let enemy = vec![mk(Species::Marowak, 30)];
        let mut s = BattleScreen::from_parties(true, &player, &enemy, None);
        s.is_ghost = true;
        s.battle_state.as_mut().unwrap().enemy.active_mon_mut().hp = 1;

        s.use_ball(ItemId::PokeBall);
        assert!(s.captured_mon.is_none(), "a GHOST cannot be caught");
        // The throw narration ("X used POKé BALL!") comes first, then the
        // dodge text — check the full message list, not just the first page.
        let msgs = turn_messages(&s);
        assert!(
            msgs.iter().any(|m| m.contains("GHOST")),
            "a dodge message is shown: {msgs:?}"
        );
    }

    fn turn_messages(s: &BattleScreen) -> Vec<String> {
        match &s.phase {
            BattlePhase::ShowingText { messages, .. } => messages.clone(),
            _ => s.current_message.clone().into_iter().collect(),
        }
    }

    /// PrintGhostText (engine/battle/core.asm:3281-3307): in a ghost battle the
    /// player's move does not execute — "<MON> is too scared to move!" prints, no
    /// damage is dealt, and no PP is spent (the check runs before GetCurrentMove).
    #[test]
    fn ghost_battle_player_move_fails_scared() {
        let player = vec![mk(Species::Charmander, 20)];
        let enemy = vec![mk(Species::Marowak, 30)];
        let mut s = BattleScreen::from_parties(true, &player, &enemy, None);
        s.is_ghost = true;
        let (enemy_hp_before, player_hp_before) = {
            let bs = s.battle_state.as_ref().unwrap();
            (bs.enemy.active_mon().hp, bs.player.active_mon().hp)
        };
        let pp_before = s.battle_state.as_ref().unwrap().player.active_mon().pp[0];

        s.execute_turn_with_move(0);

        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.enemy.active_mon().hp, enemy_hp_before, "the scared mon deals no damage");
        assert_eq!(bs.player.active_mon().pp[0], pp_before, "no PP is spent when scared");
        assert_eq!(bs.player.active_mon().hp, player_hp_before, "the GHOST never attacks");
        let msgs = turn_messages(&s);
        assert!(
            msgs.iter().any(|m| m.contains("scared to move!")),
            "the scared text is shown: {msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("GHOST: Get out...")),
            "the GHOST's GetOut text is shown: {msgs:?}"
        );
    }

    /// The player's status check comes first in the original (`ld a, [wBattleMonStatus]
    /// / and (1 << FRZ) | SLP_MASK / ret nz`): a sleeping/frozen mon gets the NORMAL
    /// status message, not the scared text. The GHOST is still blocked.
    #[test]
    fn ghost_battle_sleeping_player_not_scared() {
        let player = vec![mk(Species::Charmander, 20)];
        let enemy = vec![mk(Species::Marowak, 30)];
        let mut s = BattleScreen::from_parties(true, &player, &enemy, None);
        s.is_ghost = true;
        s.battle_state.as_mut().unwrap().player.active_mon_mut().status =
            StatusCondition::Sleep(2);

        s.execute_turn_with_move(0);

        let msgs = turn_messages(&s);
        assert!(
            !msgs.iter().any(|m| m.contains("scared to move!")),
            "a sleeping mon is not scared — the status line shows instead: {msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("asleep")),
            "the normal sleep message shows: {msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("GHOST: Get out...")),
            "the GHOST is still blocked: {msgs:?}"
        );
    }

    /// TryRunningFromBattle (engine/battle/core.asm:1496-1498): a ghost battle
    /// ALWAYS allows escape (`call IsGhostBattle / jp z, .canEscape`) — even
    /// against a much faster enemy that would normally block every attempt.
    #[test]
    fn ghost_battle_run_always_succeeds() {
        for _ in 0..10 {
            let player = vec![mk(Species::Charmander, 5)];
            let enemy = vec![mk(Species::Gastly, 40)];
            let mut s = BattleScreen::from_parties(true, &player, &enemy, None);
            s.is_ghost = true;

            s.handle_run();
            let escaped = match &s.phase {
                BattlePhase::ShowingText { next_phase, .. } => {
                    matches!(**next_phase, BattlePhase::BattleOver { escaped: true, .. })
                }
                BattlePhase::BattleOver { escaped: true, .. } => true,
                _ => false,
            };
            assert!(escaped, "a ghost battle always allows escape");
        }
    }

    /// Drive the intro with A held down, collecting the IntroPhases visited until
    /// the battle menu is reached (or the tick budget runs out).
    fn drive_intro(s: &mut BattleScreen) -> Vec<IntroPhase> {
        let mut visited = Vec::new();
        for _ in 0..2000 {
            if let BattlePhase::Intro { phase, .. } = &s.phase {
                if visited.last() != Some(phase) {
                    visited.push(*phase);
                }
            }
            if matches!(s.phase, BattlePhase::PlayerMenu) {
                break;
            }
            s.update_frame(BattleInput {
                up: false,
                down: false,
                left: false,
                right: false,
                a: true,
                b: false,
            });
        }
        visited
    }

    /// The ghost-Marowak battle WITH the scope: WildReveal (as "Enemy GHOST
    /// appeared!") → GhostUnveil (unveil text + MarowakAnim) → WildReveal again
    /// (as a normal "Wild MAROWAK appeared!") → send-out.
    #[test]
    fn ghost_marowak_reveal_intro_flow() {
        let player = vec![mk(Species::Charmander, 20)];
        let enemy = vec![mk(Species::Marowak, 30)];
        let mut s = BattleScreen::from_parties(true, &player, &enemy, None);
        s.ghost_marowak_reveal = true;

        let visited = drive_intro(&mut s);
        let unveil_pos = visited.iter().position(|p| *p == IntroPhase::GhostUnveil);
        assert!(unveil_pos.is_some(), "the unveil phase runs: {visited:?}");
        let reveal_count = visited.iter().filter(|p| **p == IntroPhase::WildReveal).count();
        assert_eq!(reveal_count, 2, "WildReveal brackets the unveil: {visited:?}");
        assert!(
            !visited.contains(&IntroPhase::GhostCantID),
            "no can't-be-ID'd line with the scope: {visited:?}"
        );
        assert!(s.ghost_marowak_unveiled, "the ghost is unveiled after the phase");
        assert!(matches!(s.phase, BattlePhase::PlayerMenu), "the intro completes: {:?}", s.phase);
    }

    /// A no-scope ghost battle shows the extra "can't be ID'd" intro line.
    #[test]
    fn no_scope_ghost_intro_has_cant_id_phase() {
        let player = vec![mk(Species::Charmander, 20)];
        let enemy = vec![mk(Species::Gastly, 20)];
        let mut s = BattleScreen::from_parties(true, &player, &enemy, None);
        s.is_ghost = true;

        let visited = drive_intro(&mut s);
        assert!(
            visited.contains(&IntroPhase::GhostCantID),
            "the can't-be-ID'd phase runs: {visited:?}"
        );
        assert!(!visited.contains(&IntroPhase::GhostUnveil));
        assert!(matches!(s.phase, BattlePhase::PlayerMenu));
    }

    /// An ordinary wild battle visits neither ghost intro phase.
    #[test]
    fn normal_wild_intro_has_no_ghost_phases() {
        let player = vec![mk(Species::Charmander, 20)];
        let enemy = vec![mk(Species::Rattata, 5)];
        let mut s = BattleScreen::from_parties(true, &player, &enemy, None);

        let visited = drive_intro(&mut s);
        assert!(!visited.contains(&IntroPhase::GhostCantID));
        assert!(!visited.contains(&IntroPhase::GhostUnveil));
        assert!(matches!(s.phase, BattlePhase::PlayerMenu));
    }
}

/// Safari battle turn integration (`resolve_safari_action` over the `safari` mechanics).
#[cfg(test)]
mod safari_integration_tests {
    use super::*;
    use crate::battle::safari::SafariState;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
    }

    fn safari_battle(catch_rate: u8, balls: u8) -> BattleScreen {
        let mut s = BattleScreen::from_parties(
            true,
            &[mk(Species::Charmander, 20)],
            &[mk(Species::Rattata, 20)],
            None,
        );
        s.is_safari = true;
        s.safari = Some(SafariState::new(catch_rate, balls));
        s.safari_menu = menu::SafariBattleMenuState::new(balls);
        s
    }

    #[test]
    fn safari_run_escapes() {
        let mut s = safari_battle(100, 30);
        s.resolve_safari_action(menu::SafariMenuAction::Run);
        assert!(
            s.current_message.as_deref().unwrap_or("").contains("Got away"),
            "RUN escapes: {:?}",
            s.current_message
        );
    }

    #[test]
    fn safari_bait_halves_catch_rate() {
        let mut s = safari_battle(100, 30);
        s.resolve_safari_action(menu::SafariMenuAction::Bait);
        assert_eq!(
            s.safari.as_ref().unwrap().catch_rate,
            50,
            "BAIT halves the catch rate (survives the flee roll either way)"
        );
    }

    #[test]
    fn safari_ball_consumes_a_ball() {
        let mut s = safari_battle(3, 30); // tiny catch rate → the throw is what we test
        s.resolve_safari_action(menu::SafariMenuAction::Ball);
        assert_eq!(s.safari.as_ref().unwrap().balls, 29, "one Safari Ball consumed");
    }
}

/// Viridian Old-Man catch tutorial (`is_old_man`): a scripted, guaranteed catch demo.
#[cfg(test)]
mod old_man_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
    }

    /// The tutorial catches the WEEDLE (demo text) but keeps NOTHING — `captured_mon`
    /// stays None so the app adds no mon to the party.
    #[test]
    fn old_man_tutorial_catches_but_keeps_nothing() {
        let mut s = BattleScreen::from_parties(
            true,
            &[mk(Species::Charmander, 5)],
            &[mk(Species::Weedle, 5)],
            None,
        );
        s.is_old_man = true;
        s.resolve_old_man_tutorial();
        assert!(
            s.current_message.as_deref().unwrap_or("").contains("OLD MAN"),
            "the demo narrates the OLD MAN's throw: {:?}",
            s.current_message
        );
        assert!(s.captured_mon.is_none(), "the tutorial WEEDLE is a demo — not kept");
    }
}

/// Battle/victory music classification, matching the original
/// `PlayBattleMusic` (audio/play_battle_music.asm) and `TrainerBattleVictory`
/// (engine/battle/core.asm).
#[cfg(test)]
mod battle_music_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::music::MusicId;
    use pokered_data::species::Species;
    use pokered_data::trainer_data::TrainerClass;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
    }

    fn battle(is_wild: bool, class: Option<TrainerClass>) -> BattleScreen {
        BattleScreen::from_parties(
            is_wild,
            &[mk(Species::Charmander, 20)],
            &[mk(Species::Rattata, 20)],
            class,
        )
    }

    #[test]
    fn wild_battle_plays_wild_theme() {
        let s = battle(true, None);
        assert_eq!(s.battle_music_id(), MusicId::WildBattle as u8);
        assert_eq!(s.victory_music_id(), MusicId::DefeatedWildMon as u8);
    }

    #[test]
    fn ordinary_trainer_plays_trainer_theme() {
        let s = battle(false, Some(TrainerClass::Youngster));
        assert_eq!(s.battle_music_id(), MusicId::TrainerBattle as u8);
        assert_eq!(s.victory_music_id(), MusicId::DefeatedTrainer as u8);
    }

    #[test]
    fn gym_leader_plays_gym_theme() {
        let s = battle(false, Some(TrainerClass::Brock));
        assert_eq!(s.battle_music_id(), MusicId::GymLeaderBattle as u8);
        assert_eq!(s.victory_music_id(), MusicId::DefeatedGymLeader as u8);
    }

    /// The champion battle (OPP_RIVAL3) is special-cased to MUSIC_FINAL_BATTLE
    /// in play_battle_music.asm; its victory fanfare is the gym-leader one
    /// (core.asm `TrainerBattleVictory`: `cp RIVAL3` → MUSIC_DEFEATED_GYM_LEADER).
    #[test]
    fn champion_rival_plays_final_battle_theme() {
        let s = battle(false, Some(TrainerClass::Rival3));
        assert_eq!(s.battle_music_id(), MusicId::FinalBattle as u8);
        assert_eq!(s.victory_music_id(), MusicId::DefeatedGymLeader as u8);
    }

    /// Lance is special-cased onto the gym-leader battle theme
    /// (`cp OPP_LANCE` in play_battle_music.asm), but his victory fanfare is
    /// the normal defeated-trainer one (only wGymLeaderNo/RIVAL3 get the gym
    /// fanfare in `TrainerBattleVictory`, and LancesRoom never sets it).
    #[test]
    fn lance_plays_gym_theme_but_trainer_victory() {
        let s = battle(false, Some(TrainerClass::Lance));
        assert_eq!(s.battle_music_id(), MusicId::GymLeaderBattle as u8);
        assert_eq!(s.victory_music_id(), MusicId::DefeatedTrainer as u8);
    }

    /// The other Elite Four members keep the normal trainer theme: their rooms
    /// never set wGymLeaderNo (cleared on every map entry), and only
    /// OPP_LANCE/OPP_RIVAL3 are special-cased in play_battle_music.asm.
    #[test]
    fn elite_four_members_play_trainer_theme() {
        for tc in [TrainerClass::Lorelei, TrainerClass::Bruno, TrainerClass::Agatha] {
            let s = battle(false, Some(tc));
            assert_eq!(s.battle_music_id(), MusicId::TrainerBattle as u8, "{tc:?}");
            assert_eq!(s.victory_music_id(), MusicId::DefeatedTrainer as u8, "{tc:?}");
        }
    }
}

/// Low-health alarm enable/disable semantics (`wLowHealthAlarm`), matching
/// `DrawPlayerHUDAndHPBar` (engine/battle/core.asm:1851-1875) and
/// `EndLowHealthAlarm` (core.asm:864-872).
#[cfg(test)]
mod low_health_alarm_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
    }

    fn battle() -> BattleScreen {
        BattleScreen::from_parties(
            true,
            &[mk(Species::Charmander, 20)],
            &[mk(Species::Rattata, 20)],
            None,
        )
    }

    #[test]
    fn alarm_off_at_full_hp() {
        let s = battle();
        assert!(!s.low_health_alarm());
    }

    /// Threshold: the HP bar is red under 10 of 48 pixels
    /// (`GetHealthBarColor`, home/palettes.asm:44-57, on the
    /// `HP * 48 / maxHP` pixel count, engine/gfx/hp_bar.asm:4-6).
    #[test]
    fn alarm_on_only_when_hp_bar_red() {
        let mut s = battle();
        s.player_max_hp = 100;
        // 20/100 HP → 9 pixels: red → alarm on.
        s.player_hp = 20;
        assert!(s.low_health_alarm());
        // 21/100 HP → 10 pixels: yellow → alarm off.
        s.player_hp = 21;
        assert!(!s.low_health_alarm());
    }

    /// Fainting disables the alarm (core.asm:1863-1870 `ld [hl], 0`).
    #[test]
    fn faint_disables_alarm() {
        let mut s = battle();
        s.player_max_hp = 100;
        s.player_hp = 20;
        assert!(s.low_health_alarm());
        s.player_hp = 0;
        assert!(!s.low_health_alarm());
    }

    /// Winning disables the alarm and blocks reactivation
    /// (`EndLowHealthAlarm`, core.asm:864-872 — called when the enemy
    /// party is wiped, core.asm:793/916).
    #[test]
    fn wiped_enemy_party_disables_alarm() {
        let mut s = battle();
        s.player_max_hp = 100;
        s.player_hp = 20;
        assert!(s.low_health_alarm());
        if let Some(ref mut bs) = s.battle_state {
            for p in bs.enemy.party.iter_mut() {
                p.hp = 0;
            }
        }
        assert!(!s.low_health_alarm());
    }

    /// Battle-over phases (escape/loss/catch) keep the alarm off — battle
    /// end clears `wLowHealthAlarm` (engine/battle/end_of_battle.asm:48).
    #[test]
    fn battle_over_phase_disables_alarm() {
        let mut s = battle();
        s.player_max_hp = 100;
        s.player_hp = 20;
        s.phase = BattlePhase::BattleOver {
            won: false,
            escaped: true,
            wait_frames: 30,
        };
        assert!(!s.low_health_alarm());
    }
}

#[cfg(test)]
mod poke_flute_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    fn battle_screen() -> BattleScreen {
        let mk = |sp| {
            create_pokemon_with_moves(
                sp,
                30,
                [0xFF, 0xFF],
                [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let player = vec![mk(Species::Pidgey)];
        let enemy = vec![mk(Species::Rattata)];
        BattleScreen::from_parties(true, &player, &enemy, None)
    }

    /// The in-battle flute music plays only when some Pokémon were asleep
    /// (`wWereAnyMonsAsleep` != 0, engine/items/item_effects.asm:1728-1739);
    /// core surfaces that as a one-shot pending flag for the app layer.
    #[test]
    fn flute_sfx_pending_only_when_a_mon_was_asleep() {
        // No one asleep → no jingle request.
        let mut s = battle_screen();
        s.use_poke_flute();
        assert!(!s.take_poke_flute_sfx_pending());

        // Enemy asleep → jingle requested exactly once, sleep cured.
        let mut s = battle_screen();
        s.battle_state
            .as_mut()
            .unwrap()
            .enemy
            .active_mon_mut()
            .status = StatusCondition::Sleep(3);
        s.use_poke_flute();
        assert!(s.take_poke_flute_sfx_pending());
        assert!(
            !s.take_poke_flute_sfx_pending(),
            "the request is consumed by the first take"
        );
        assert!(!s.enemy_status.is_sleep());
    }

    /// Player-side sleep also triggers the jingle and wakes the whole party
    /// (WakeUpEntireParty, engine/items/item_effects.asm:1755-1772).
    #[test]
    fn flute_wakes_player_party_and_requests_sfx() {
        let mut s = battle_screen();
        let bs = s.battle_state.as_mut().unwrap();
        bs.player.active_mon_mut().status = StatusCondition::Sleep(2);
        s.use_poke_flute();
        assert!(s.take_poke_flute_sfx_pending());
        assert!(!s.player_status.is_sleep());
        assert!(s
            .battle_state
            .as_ref()
            .unwrap()
            .player
            .party
            .iter()
            .all(|m| !m.status.is_sleep()));
    }
}


/// SHIFT/SET battle style (wOptions BIT_BATTLE_SHIFT): the "Enemy X is about
/// to use Y! Will <PLAYER> change #MON?" prompt (`TrainerAboutToUseText`,
/// data/text/text_2.asm) in the trainer-battle faint → send-out flow
/// (`ReplaceFaintedEnemyMon`, engine/battle/core.asm:1376-1444).
#[cfg(test)]
mod shift_style_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;
    use pokered_data::trainer_data::TrainerClass;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(
            sp,
            lvl,
            [0xFF, 0xFF],
            [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
        )
        .unwrap()
    }

    fn player_party() -> Vec<crate::battle::state::Pokemon> {
        vec![mk(Species::Charmander, 10), mk(Species::Squirtle, 10)]
    }

    fn enemy_party() -> Vec<crate::battle::state::Pokemon> {
        vec![mk(Species::Rattata, 8), mk(Species::Onix, 9)]
    }

    /// Trainer battle, SHIFT style, both parties ≥ 2, enemy lead fainted.
    fn prompted_battle() -> BattleScreen {
        let mut s = BattleScreen::from_parties(
            false,
            &player_party(),
            &enemy_party(),
            Some(TrainerClass::Youngster),
        );
        s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
        s
    }

    fn input(f: impl Fn(&mut BattleInput)) -> BattleInput {
        let mut i = BattleInput::none();
        f(&mut i);
        i
    }

    /// Tick through any ShowingText sequence (A held every frame).
    fn tick_through_text(s: &mut BattleScreen) {
        for _ in 0..200 {
            if !matches!(s.phase, BattlePhase::ShowingText { .. }) {
                break;
            }
            s.update_frame(input(|i| i.a = true));
        }
    }

    #[test]
    fn shift_style_prompts_in_trainer_battle() {
        let mut s = prompted_battle();
        s.player_name = Some("RED".to_string());
        let phase = s.check_faint_after_turn();
        assert_eq!(phase, BattlePhase::ShiftPrompt);
        s.phase = phase;
        s.post_text_transition();
        // Cursor defaults to NO (original `ld a, 1 / ld [wCurrentMenuItem], a`).
        assert!(!s.shift_prompt_yes);
        let msg = s.current_message.clone().unwrap();
        assert!(msg.contains("is about to use ONIX!"), "{msg}");
        assert!(msg.contains("Will RED change POKéMON?"), "{msg}");
    }

    #[test]
    fn set_style_suppresses_prompt() {
        let mut s = prompted_battle();
        s.battle_style = BattleStyle::Set;
        assert_eq!(
            s.check_faint_after_turn(),
            BattlePhase::EnemySendingNext { wait_frames: 30 }
        );
    }

    #[test]
    fn wild_battle_never_prompts() {
        let mut s = BattleScreen::from_parties(true, &player_party(), &enemy_party(), None);
        s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
        assert_eq!(
            s.check_faint_after_turn(),
            BattlePhase::EnemySendingNext { wait_frames: 30 }
        );
    }

    #[test]
    fn single_mon_player_party_never_prompts() {
        let mut s = BattleScreen::from_parties(
            false,
            &[mk(Species::Charmander, 10)],
            &enemy_party(),
            Some(TrainerClass::Youngster),
        );
        s.battle_state.as_mut().unwrap().enemy.party[0].hp = 0;
        assert_eq!(
            s.check_faint_after_turn(),
            BattlePhase::EnemySendingNext { wait_frames: 30 }
        );
    }

    #[test]
    fn answering_no_sends_next_enemy_without_switch() {
        let mut s = prompted_battle();
        s.phase = s.check_faint_after_turn();
        s.post_text_transition();
        // Default cursor is NO; A confirms. Enemy sends out, no player switch.
        s.update_frame(input(|i| i.a = true));
        assert_eq!(s.phase, BattlePhase::EnemySendingNext { wait_frames: 30 });
        assert!(s.current_message.is_none());
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.enemy.active_pokemon_index, 1, "next enemy sent out");
        assert_eq!(bs.player.active_pokemon_index, 0, "player did not switch");
    }

    #[test]
    fn b_button_answers_no() {
        let mut s = prompted_battle();
        s.phase = s.check_faint_after_turn();
        s.post_text_transition();
        s.update_frame(input(|i| i.b = true));
        assert_eq!(s.phase, BattlePhase::EnemySendingNext { wait_frames: 30 });
        assert_eq!(s.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 1);
    }

    #[test]
    fn answering_yes_switches_after_enemy_send_out() {
        let mut s = prompted_battle();
        s.phase = s.check_faint_after_turn();
        s.post_text_transition();
        // Up toggles the cursor to YES, A confirms → party select opens.
        s.update_frame(input(|i| i.up = true));
        assert!(s.shift_prompt_yes);
        s.update_frame(input(|i| i.a = true));
        assert_eq!(s.phase, BattlePhase::ShiftSwitchSelect);
        // Cursor starts on the active mon → "already out!"; move down to index 1.
        s.update_frame(input(|i| i.down = true));
        s.update_frame(input(|i| i.a = true));
        assert_eq!(s.pending_shift_switch, Some(1));
        assert_eq!(s.phase, BattlePhase::EnemySendingNext { wait_frames: 30 });
        assert_eq!(s.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 1);
        // After the send-out wait, the free switch applies (no enemy attack).
        for _ in 0..31 {
            s.update_frame(BattleInput::none());
        }
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(bs.player.active_pokemon_index, 1, "free switch applied");
        match &s.phase {
            BattlePhase::ShowingText { messages, next_phase, .. } => {
                // Messages are paginated/wrapped, so assert on the joined text.
                let joined = messages.join(" ");
                assert!(joined.contains("sent out"), "{joined}");
                assert!(joined.contains("ONIX!"), "{joined}");
                assert!(joined.contains("CHARMANDER, come"), "{joined}");
                assert!(joined.contains("back!"), "{joined}");
                assert!(joined.contains("Go! SQUIRTLE!"), "{joined}");
                assert_eq!(**next_phase, BattlePhase::PlayerMenu);
            }
            other => panic!("expected ShowingText after shift switch, got {other:?}"),
        }
        assert_eq!(s.pending_shift_switch, None);
    }

    #[test]
    fn shift_switch_select_rejects_active_and_fainted() {
        let mut s = prompted_battle();
        s.battle_state.as_mut().unwrap().player.party[1].hp = 0;
        s.battle_state.as_mut().unwrap().player.party.push(mk(Species::Bulbasaur, 9));
        s.phase = s.check_faint_after_turn();
        s.post_text_transition();
        s.update_frame(input(|i| i.up = true));
        s.update_frame(input(|i| i.a = true));
        assert_eq!(s.phase, BattlePhase::ShiftSwitchSelect);
        // Index 0 = the mon already out → "already out!" and stay.
        s.update_frame(input(|i| i.a = true));
        match &s.phase {
            BattlePhase::ShowingText { messages, next_phase, .. } => {
                assert!(messages.iter().any(|m| m.contains("already out!")), "{messages:?}");
                assert_eq!(**next_phase, BattlePhase::ShiftSwitchSelect);
            }
            other => panic!("expected ShowingText, got {other:?}"),
        }
        s.update_frame(input(|i| i.a = true)); // dismiss text → back to select
        tick_through_text(&mut s);
        assert_eq!(s.phase, BattlePhase::ShiftSwitchSelect);
        // Index 1 = fainted → "no will to fight" and stay.
        s.update_frame(input(|i| i.down = true));
        s.update_frame(input(|i| i.a = true));
        match &s.phase {
            BattlePhase::ShowingText { messages, next_phase, .. } => {
                assert!(messages.iter().any(|m| m.contains("to fight!")), "{messages:?}");
                assert_eq!(**next_phase, BattlePhase::ShiftSwitchSelect);
            }
            other => panic!("expected ShowingText, got {other:?}"),
        }
    }

    #[test]
    fn b_in_switch_select_proceeds_without_switching() {
        let mut s = prompted_battle();
        s.phase = s.check_faint_after_turn();
        s.post_text_transition();
        s.update_frame(input(|i| i.up = true));
        s.update_frame(input(|i| i.a = true));
        assert_eq!(s.phase, BattlePhase::ShiftSwitchSelect);
        s.update_frame(input(|i| i.b = true));
        assert_eq!(s.phase, BattlePhase::EnemySendingNext { wait_frames: 30 });
        assert_eq!(s.pending_shift_switch, None);
        assert_eq!(s.battle_state.as_ref().unwrap().enemy.active_pokemon_index, 1);
        assert_eq!(s.battle_state.as_ref().unwrap().player.active_pokemon_index, 0);
    }
}

#[cfg(test)]
mod badge_obedience_integration_tests {
    //! End-to-end coverage for the Gen-1 badge stat boosts (+ the stat-up
    //! glitch) and traded-mon obedience, through the production stack path.
    use super::*;
    use crate::battle::badge_boosts;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    const BOULDER: u8 = 1 << 0;
    const THUNDER: u8 = 1 << 2;
    const EARTH: u8 = 1 << 7;

    fn mk(sp: Species, lvl: u8, moves: [MoveId; 4]) -> state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], moves).unwrap()
    }

    fn phase_messages(s: &BattleScreen) -> Vec<String> {
        match &s.phase {
            BattlePhase::ShowingText { messages, .. } => messages.clone(),
            _ => vec![],
        }
    }

    fn has_disobedience_evidence(s: &BattleScreen) -> bool {
        let asleep = s
            .battle_state
            .as_ref()
            .map(|bs| bs.player.active_mon().status.is_sleep())
            .unwrap_or(false);
        asleep
            || phase_messages(s).iter().any(|m| {
                m.contains("loafing")
                    || m.contains("won't")
                    || m.contains("turned\naway")
                    || m.contains("ignored orders")
                    || m.contains("began\nto nap")
                    || m.contains("hurt itself")
            })
    }

    /// The send-out `ApplyBadgeStatBoosts` (core.asm:1659): after one
    /// production turn the player battler's working stats carry the boost.
    #[test]
    fn badge_boost_applies_through_production_turn() {
        let player = mk(Species::Scyther, 50, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
        let raw = [player.attack, player.defense, player.speed, player.special];
        let enemy = mk(Species::Chansey, 5, [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]);
        let mut s = BattleScreen::from_parties(true, &[player], &[enemy], None);
        s.player_badges = BOULDER | THUNDER;
        s.execute_turn_with_move(0);
        let bs = s.battle_state.as_ref().unwrap();
        assert_eq!(
            bs.player.badge_boosted_stats,
            Some(badge_boosts::initial_boosted_stats(raw, BOULDER | THUNDER)),
            "Attack + Defense boosted ×9/8, Speed/Special untouched"
        );
    }

    /// The stat-up glitch, in-turn, through the stack engine: Swords Dance
    /// re-applies the badge boosts to ALL FOUR working stats (effects.asm:499),
    /// while Attack itself is first recomputed from its unmodified value.
    /// Deterministic (scripted rng).
    #[test]
    fn stat_up_glitch_through_stack_turn() {
        use crate::battle::pokered_rules;
        use jrpg_engine::battle::rng::ScriptedRng;
        use jrpg_engine::battle::stack::StackDriver;
        use jrpg_engine::battle::{BattleAction, BattlerRef};

        let player = mk(Species::Scyther, 50, [MoveId::SwordsDance, MoveId::None, MoveId::None, MoveId::None]);
        let raw = [player.attack, player.defense, player.speed, player.special];
        let enemy = mk(Species::Chansey, 5, [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]);
        let badges = BOULDER | THUNDER;

        let mut ls = state::new_battle_state(state::BattleType::Wild, vec![player], vec![enemy]);
        ls.player_badges = badges;
        badge_boosts::ensure_initialized(&mut ls.player, badges);

        pokered_rules::install_canonical();
        pokered_rules::clear_current_moves();
        pokered_rules::set_current_move(
            BattlerRef::PLAYER,
            *MoveData::get(MoveId::SwordsDance).unwrap(),
        );
        pokered_rules::set_current_move(
            BattlerRef::OPPONENT,
            *MoveData::get(MoveId::Splash).unwrap(),
        );
        let (mut eng, mut effects) = pokered_rules::runtime::engine_state_from_legacy(&ls);
        let actions = [
            BattleAction::<pokered_rules::PokeredRules>::Fight { move_: MoveId::SwordsDance },
            BattleAction::<pokered_rules::PokeredRules>::Nothing,
        ];
        let mut rng = ScriptedRng::new(vec![0; 256]);
        let (_r, _log) = StackDriver::execute_turn_logged(
            &pokered_rules::PokeredRules,
            &mut eng,
            &mut effects,
            actions,
            &mut rng,
        );
        pokered_rules::runtime::apply_engine_to_legacy(&mut ls, &eng, &effects);

        // Expected: send-out boost, then the glitch round — Attack reset to its
        // raw value first (its own boost does NOT compound), Defense compounds.
        let mut expected = badge_boosts::initial_boosted_stats(raw, badges);
        expected[0] = raw[0];
        badge_boosts::apply_badge_stat_boosts(&mut expected, badges);
        assert_eq!(ls.player.badge_boosted_stats, Some(expected));
        assert_eq!(ls.player.stat_stages.attack, 2, "Swords Dance still applied");
        // Sanity: Defense really did compound past the single send-out boost.
        let once = badge_boosts::initial_boosted_stats(raw, badges);
        assert!(expected[1] > once[1], "Defense compounded by the glitch");
    }

    /// A traded mon at/below its badge threshold always obeys; so does any mon
    /// with the EarthBadge; so does an OWN mon at any level. (Deterministic —
    /// these paths draw no disobedience roll at all.)
    #[test]
    fn obedience_deterministic_obey_cases() {
        let enemy = || mk(Species::Chansey, 5, [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]);

        // Traded (ot_id != player_id) but L10 <= no-badge threshold 10.
        let mut traded_low = mk(Species::Mewtwo, 10, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
        traded_low.ot_id = 9999;
        let mut s = BattleScreen::from_parties(true, &[traded_low], &[enemy()], None);
        s.player_id = 1234;
        s.player_badges = 0;
        s.execute_turn_with_move(0);
        assert!(!has_disobedience_evidence(&s), "L10 at threshold obeys");
        assert_eq!(s.battle_state.as_ref().unwrap().player.last_move_used, MoveId::Tackle);

        // Traded L100 with the EarthBadge (threshold 101): always obeys.
        let mut traded_hi = mk(Species::Mewtwo, 100, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
        traded_hi.ot_id = 9999;
        let mut s = BattleScreen::from_parties(true, &[traded_hi], &[enemy()], None);
        s.player_id = 1234;
        s.player_badges = EARTH;
        s.execute_turn_with_move(0);
        assert!(!has_disobedience_evidence(&s), "EarthBadge: always obey");
        assert_eq!(s.battle_state.as_ref().unwrap().player.last_move_used, MoveId::Tackle);

        // OWN mon (ot_id == player_id) L100, no badges: always obeys.
        let mut own = mk(Species::Mewtwo, 100, [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
        own.ot_id = 1234;
        let mut s = BattleScreen::from_parties(true, &[own], &[enemy()], None);
        s.player_id = 1234;
        s.player_badges = 0;
        s.execute_turn_with_move(0);
        assert!(!has_disobedience_evidence(&s), "own mon always obeys");
        assert_eq!(s.battle_state.as_ref().unwrap().player.last_move_used, MoveId::Tackle);
    }

    /// A traded L100 mon with no badges disobeys ~96% of turns: over 30 turns
    /// the probability of NEVER seeing a disobedience outcome is ~1e-28 (each
    /// obeying turn Tackles the spongy enemy; the loop breaks on the first
    /// nap / loaf / self-hit / message).
    #[test]
    fn traded_high_level_mon_disobeys() {
        let mut traded = mk(Species::Mewtwo, 100, [MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]);
        traded.ot_id = 9999;
        let enemy = mk(Species::Chansey, 5, [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]);
        let mut s = BattleScreen::from_parties(true, &[traded], &[enemy], None);
        s.player_id = 1234;
        s.player_badges = 0;
        // Sponge: the enemy can never faint, so the battle always continues.
        if let Some(ref mut bs) = s.battle_state {
            let m = bs.enemy.active_mon_mut();
            m.max_hp = 65000;
            m.hp = 65000;
        }
        let mut seen = false;
        for _ in 0..30 {
            // Wake the mon so a previous nap doesn't legitimately skip the roll.
            if let Some(ref mut bs) = s.battle_state {
                if bs.player.active_mon().status.is_sleep() {
                    bs.player.active_mon_mut().status = StatusCondition::None;
                }
                let m = bs.enemy.active_mon_mut();
                m.hp = 65000; // top up after any Tackle
            }
            s.execute_turn_with_move(0);
            if has_disobedience_evidence(&s) {
                seen = true;
                break;
            }
        }
        assert!(seen, "a traded L100 mon with no badges should disobey within 30 turns");
    }
}


/// HP-bar drain/refill animation (`UpdateHPBar`, engine/gfx/hp_bar.asm):
/// displayed HP tweens toward real HP at 1 bar pixel per 2 frames, text
/// advancement waits for the tween, first-draw/send-out snaps instantly.
#[cfg(test)]
mod hp_bar_anim_tests {
    use super::*;
    use crate::pokemon::stats::create_pokemon_with_moves;
    use pokered_data::species::Species;

    fn mk(sp: Species, lvl: u8) -> crate::battle::state::Pokemon {
        create_pokemon_with_moves(sp, lvl, [0xFF, 0xFF], [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]).unwrap()
    }

    fn battle() -> BattleScreen {
        BattleScreen::from_parties(
            true,
            &[mk(Species::Charmander, 20)],
            &[mk(Species::Rattata, 20)],
            None,
        )
    }

    /// 1 bar pixel = `round(max_hp / 48)` HP (bar is 48 px, hp_bar.asm:15-16).
    #[test]
    fn step_size_is_one_bar_pixel() {
        assert_eq!(HpBarAnim::step_size(0), 1, "min step");
        assert_eq!(HpBarAnim::step_size(20), 1);
        assert_eq!(HpBarAnim::step_size(48), 1, "<=48 max HP: 1 HP == 1 px");
        assert_eq!(HpBarAnim::step_size(96), 2);
        assert_eq!(HpBarAnim::step_size(144), 3);
    }

    /// The bar steps on every *other* frame — the original's
    /// `UpdateHPBar_AnimateHPBar` waits 2 frames per pixel
    /// ("two waiting frames each", hp_bar.asm:140-163).
    #[test]
    fn tween_moves_one_step_per_two_frames() {
        let mut anim = HpBarAnim::default();
        let mut display = 48u16;
        // Snap at full HP first (battle-start behavior).
        anim.set_target(BattleSide::Enemy, 48, 48, Species::Rattata, &mut display);
        assert!(!anim.is_active());
        assert!(!anim.drain_sfx_pending);

        // Take 10 damage → drain starts.
        anim.set_target(BattleSide::Enemy, 38, 48, Species::Rattata, &mut display);
        assert!(anim.is_active());
        assert!(anim.drain_sfx_pending, "drain notifies the damage SFX once");

        let mut other = 10u16; // untouched side
        anim.tick(&mut other, &mut display);
        assert_eq!(display, 47, "1 px (1 HP at 48 max) on the first step frame");
        anim.tick(&mut other, &mut display);
        assert_eq!(display, 47, "no movement on the divider frame");
        anim.tick(&mut other, &mut display);
        assert_eq!(display, 46, "next step 2 frames later");
        anim.tick(&mut other, &mut display);
        assert_eq!(display, 46);
    }

    /// The tween lands exactly on the target and then goes idle.
    #[test]
    fn tween_completes_and_goes_idle() {
        let mut anim = HpBarAnim::default();
        let mut display = 48u16;
        anim.set_target(BattleSide::Player, 48, 48, Species::Charmander, &mut display);
        anim.set_target(BattleSide::Player, 45, 48, Species::Charmander, &mut display);
        let mut other = 0u16;
        for _ in 0..6 {
            anim.tick(&mut display, &mut other);
        }
        assert_eq!(display, 45);
        assert!(!anim.is_active());
        // Further ticks are no-ops.
        anim.tick(&mut display, &mut other);
        assert_eq!(display, 45);
    }

    /// Refills animate too (original `wHPBarDelta = $01` path) but do NOT
    /// request the damage SFX.
    #[test]
    fn refill_animates_without_damage_sfx() {
        let mut anim = HpBarAnim::default();
        let mut display = 20u16;
        anim.set_target(BattleSide::Player, 20, 48, Species::Charmander, &mut display);
        anim.set_target(BattleSide::Player, 30, 48, Species::Charmander, &mut display);
        assert!(anim.is_active());
        assert!(!anim.drain_sfx_pending);
        let mut other = 0u16;
        anim.tick(&mut display, &mut other);
        anim.tick(&mut display, &mut other);
        assert_eq!(display, 21, "refill steps up at the same 1 px / 2 frames");
    }

    /// A different mon (species or max-HP change — send-out after a faint)
    /// draws its bar instantly, like the original's `DrawHUDsAndHPBars`.
    #[test]
    fn send_out_snaps_instantly() {
        let mut anim = HpBarAnim::default();
        let mut display = 48u16;
        anim.set_target(BattleSide::Enemy, 48, 48, Species::Rattata, &mut display);
        // Rattata faints, Pidgey sent out: new species → snap.
        anim.set_target(BattleSide::Enemy, 40, 40, Species::Pidgey, &mut display);
        assert_eq!(display, 40);
        assert!(!anim.is_active());
        // Same species but different max HP (e.g. switched-in lookalike): snap.
        anim.set_target(BattleSide::Enemy, 50, 55, Species::Pidgey, &mut display);
        assert_eq!(display, 50);
        assert!(!anim.is_active());
    }

    /// Battle start: the first sync snaps both bars to full — no drain.
    #[test]
    fn battle_start_snaps() {
        let mut s = battle();
        s.sync_display_from_state();
        assert!(!s.hp_bar_anim.is_active());
        let (php, ehp) = {
            let bs = s.battle_state.as_ref().unwrap();
            (bs.player.active_mon().hp, bs.enemy.active_mon().hp)
        };
        assert_eq!(s.player_hp, php);
        assert_eq!(s.enemy_hp, ehp);
        assert!(!s.take_hp_drain_sfx_pending());
    }

    /// Turn pacing: while the bar is draining, A/B does NOT advance the
    /// message page; once the drain finishes, input advances normally.
    /// (The original runs the drain synchronously before the next line —
    /// predef UpdateHPBar2, engine/battle/core.asm:4727.)
    #[test]
    fn showing_text_waits_for_drain() {
        let mut s = battle();
        s.sync_display_from_state();
        let enemy_full = s.enemy_hp;
        // Deal 10 damage to the enemy and re-sync → drain starts.
        if let Some(ref mut bs) = s.battle_state {
            let m = bs.enemy.active_mon_mut();
            m.hp = m.hp.saturating_sub(10);
        }
        s.sync_display_from_state();
        assert!(s.hp_bar_anim.is_active());
        assert!(s.take_hp_drain_sfx_pending(), "app plays SfxId::Damage");
        assert!(!s.take_hp_drain_sfx_pending(), "one-shot");
        assert_eq!(s.enemy_hp, enemy_full, "display still at the pre-hit value");

        s.phase = BattlePhase::ShowingText {
            messages: vec!["hit!".to_string(), "next!".to_string()],
            current: 0,
            wait_frames: 0,
            next_phase: Box::new(BattlePhase::PlayerMenu),
        };
        // Frame 1: message shows, input pressed but drain still active.
        s.update_frame(BattleInput {
            a: true,
            ..BattleInput::none()
        });
        match &s.phase {
            BattlePhase::ShowingText { current, .. } => {
                assert_eq!(*current, 0, "page must not advance mid-drain")
            }
            other => panic!("expected ShowingText, got {other:?}"),
        }
        // Run the drain to completion (≤ 2 frames per HP point).
        for _ in 0..64 {
            s.update_frame(BattleInput::none());
        }
        assert!(!s.hp_bar_anim.is_active());
        assert_eq!(s.enemy_hp, enemy_full - 10);
        // Now A advances to the second page.
        s.update_frame(BattleInput {
            a: true,
            ..BattleInput::none()
        });
        match &s.phase {
            BattlePhase::ShowingText { current, .. } => assert_eq!(*current, 1),
            other => panic!("expected ShowingText page 2, got {other:?}"),
        }
    }
}
