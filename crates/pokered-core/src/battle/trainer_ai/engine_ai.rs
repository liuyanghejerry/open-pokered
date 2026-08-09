//! Bridge from pokered's trainer-AI move scorer to the engine's generic
//! [`BattleAiProvider`] / [`BattleAi`](dotzuki_engine::battle::BattleAi) loop (P0c).
//!
//! The engine owns the AI *decision loop scaffold* (enumerate legal actions →
//! ask the game to score → pick best with an injected tie-break RNG). All Gen-1
//! scoring stays game-side: this module delegates to the existing
//! [`choose_moves`] scorer and its [`MoveChoiceResult`], reproducing the exact
//! selection that [`pick_move`](MoveChoiceResult::pick_move) /
//! `BattleScreen::pick_enemy_move` make today.
//!
//! ## Why a separate provider type
//!
//! pokered's battle engine (`battle/state.rs`) is standalone and does **not**
//! implement the engine's [`BattleProvider`] yet. The engine's
//! `BattleAiProvider: BattleProvider` supertrait therefore needs *some* provider
//! to hang the AI on. [`TrainerAiProvider`] is a thin, AI-only provider: its
//! associated types mirror the game's (`Move = MoveId`, `Species = Species`,
//! …), and only [`score_action`](BattleAiProvider::score_action) /
//! [`legal_actions`](BattleAiProvider::legal_actions) carry real logic — the
//! engine AI loop never calls the data/formula methods, so those are stubs.
//!
//! ## Selection parity
//!
//! The original picks uniformly among the *candidate* moves that
//! [`choose_moves`] flags (its minimum-score winners). So every candidate gets
//! an **equal score** here; the engine's `BattleAi::choose` then sees a tie and
//! draws one value from the RNG to pick — and because pokered's
//! [`ScriptedRng`](dotzuki_engine::battle::rng::ScriptedRng)-style `range(n)` is
//! `rand_val % n` for `n <= 256`, that reproduces `pick_move(rand_val)` exactly
//! (same candidate order, same modulo). Non-candidate / no-PP moves are not
//! enumerated, mirroring `pick_move` skipping them.

use dotzuki_engine::battle::{
    BattleAction, BattleAiProvider, BattleProvider, BattleRng, BattleState, BattlerRef,
    BattlerState as EngineBattlerState, DamageResult, EffectResult, EnumMap, MoveEffect,
};

use pokered_data::moves::MoveId;
use pokered_data::species::Species;

use super::move_choice::choose_moves;
use super::MoveChoiceLayer;
use crate::battle::state::BattlerState;

/// AI-only engine provider: scores the enemy's moves via the existing trainer-AI
/// [`choose_moves`] scorer and exposes them to [`BattleAi::choose`].
///
/// Holds an owned snapshot of the AI's `enemy`/`player` battler state plus the
/// trainer-class modification `layers`, so the per-turn context the original AI
/// consumes travels with the provider.
///
/// [`BattleAi::choose`]: dotzuki_engine::battle::BattleAi::choose
#[derive(Debug, Clone)]
pub struct TrainerAiProvider {
    enemy: BattlerState,
    player: BattlerState,
    layers: Vec<MoveChoiceLayer>,
    /// `wAILayer2Encouragement` flag passed through to layer-2 scoring.
    ai_layer2_encouragement: u8,
}

impl TrainerAiProvider {
    /// Build a provider from the AI's per-turn inputs (matching the arguments
    /// the original passes to [`choose_moves`]).
    pub fn new(
        enemy: BattlerState,
        player: BattlerState,
        layers: &[MoveChoiceLayer],
        ai_layer2_encouragement: u8,
    ) -> Self {
        Self {
            enemy,
            player,
            layers: layers.to_vec(),
            ai_layer2_encouragement,
        }
    }

    /// The flagged candidate slots from the existing scorer (`1` = candidate).
    fn candidates(&self) -> [u8; 4] {
        choose_moves(
            &self.layers,
            &self.enemy,
            &self.player,
            self.ai_layer2_encouragement,
        )
        .candidates
    }
}

impl BattleProvider for TrainerAiProvider {
    type Monster = ();
    type Move = MoveId;
    type Ability = ();
    type Status = ();
    type Stat = ();
    type Species = Species;
    type Type = ();
    type Item = ();

    // The AI loop never calls these; the real battle formulas live in
    // `battle/damage.rs`, `battle/move_execution.rs`, etc.
    fn calculate_damage(
        &self,
        _move_: &Self::Move,
        _attacker: &EngineBattlerState<Self>,
        _defender: &EngineBattlerState<Self>,
        _random: u8,
        _is_critical: bool,
    ) -> DamageResult {
        DamageResult {
            damage: 0,
            effectiveness: 1.0,
            is_miss: false,
        }
    }

    fn select_move(
        &self,
        battler: &EngineBattlerState<Self>,
        _state: &BattleState<Self>,
    ) -> Self::Move {
        battler.moves.first().copied().unwrap_or(MoveId::None)
    }

    fn apply_move_effect(
        &self,
        _effect: MoveEffect,
        _user: &mut EngineBattlerState<Self>,
        _target: &mut EngineBattlerState<Self>,
    ) -> EffectResult {
        EffectResult::NoEffect
    }

    fn create_monster(&self, species: Self::Species, _level: u8) -> EngineBattlerState<Self> {
        EngineBattlerState::new(species, 0, 0, EnumMap::new(), Vec::new())
    }
}

impl BattleAiProvider for TrainerAiProvider {
    fn score_action(
        &self,
        _st: &BattleState<Self>,
        _me: BattlerRef,
        action: &BattleAction<Self>,
        _rng: &mut dyn BattleRng,
    ) -> i32 {
        // Every candidate the scorer flagged is equally preferred (the original
        // picks uniformly among them); the engine's tie-break does the rest.
        // Non-candidate moves are filtered out in `legal_actions` and so never
        // reach here, but score them minimally for safety.
        match action {
            BattleAction::Fight { move_ } => {
                let candidates = self.candidates();
                let moves = self.enemy.active_mon().moves;
                let is_candidate = moves
                    .iter()
                    .enumerate()
                    .any(|(i, m)| m == move_ && candidates[i] > 0);
                if is_candidate {
                    0
                } else {
                    i32::MIN
                }
            }
            _ => i32::MIN,
        }
    }

    fn legal_actions(
        &self,
        _st: &BattleState<Self>,
        _me: BattlerRef,
    ) -> Vec<BattleAction<Self>> {
        // One Fight per candidate slot, in slot order — so the engine tie-break
        // scans candidates in the same order as `MoveChoiceResult::pick_move`.
        let candidates = self.candidates();
        let moves = self.enemy.active_mon().moves;
        (0..4)
            .filter(|&i| candidates[i] > 0)
            .map(|i| BattleAction::Fight { move_: moves[i] })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::state::{new_battler_state, BattlerState, Pokemon, StatusCondition};
    use dotzuki_engine::battle::rng::ScriptedRng;
    use dotzuki_engine::battle::BattleAi;
    use pokered_data::types::PokemonType;

    fn make_mon(moves: [MoveId; 4]) -> Pokemon {
        Pokemon {
            species: Species::Tauros,
            nickname: None,
            level: 50,
            hp: 200,
            max_hp: 200,
            attack: 100,
            defense: 100,
            speed: 100,
            special: 100,
            type1: PokemonType::Normal,
            type2: PokemonType::Normal,
            moves,
            pp: [10, 10, 10, 10],
            pp_ups: [0; 4],
            status: StatusCondition::None,
            dv_bytes: [0xFF, 0xFF],
            stat_exp: [0; 5],
            total_exp: 0,
            is_traded: false, ot_id: 0, ot_name: None,
        }
    }

    fn enemy_with_moves(moves: [MoveId; 4]) -> BattlerState {
        new_battler_state(vec![make_mon(moves)])
    }

    /// Like [`make_mon`] but with explicit types — needed to exercise the
    /// type-effectiveness Layer3 scoring.
    fn make_typed_mon(moves: [MoveId; 4], type1: PokemonType, type2: PokemonType) -> Pokemon {
        Pokemon {
            type1,
            type2,
            ..make_mon(moves)
        }
    }

    fn player_normal() -> BattlerState {
        new_battler_state(vec![make_mon([
            MoveId::Tackle,
            MoveId::None,
            MoveId::None,
            MoveId::None,
        ])])
    }

    /// Mirror of pokered's `pick_enemy_move` selection: candidate list from
    /// `choose_moves`, then `pick_move(rand_val)`.
    fn pokered_pick(enemy: &BattlerState, player: &BattlerState, rand_val: u8) -> MoveId {
        let result = choose_moves(&[], enemy, player, 0);
        let idx = result.pick_move(rand_val).expect("a candidate exists");
        enemy.active_mon().moves[idx]
    }

    /// Mirror of the production `pick_enemy_move` core (`battle/mod.rs:704-712`)
    /// for an arbitrary, possibly non-empty, layer set: run the real
    /// [`choose_moves`] scorer, then [`pick_move`](MoveChoiceResult::pick_move)
    /// with the scripted `rand_val` — identical to what production runs.
    fn pokered_pick_layered(
        layers: &[MoveChoiceLayer],
        enemy: &BattlerState,
        player: &BattlerState,
        ai_layer2_encouragement: u8,
        rand_val: u8,
    ) -> MoveId {
        let result = choose_moves(layers, enemy, player, ai_layer2_encouragement);
        let idx = result.pick_move(rand_val).expect("a candidate exists");
        enemy.active_mon().moves[idx]
    }

    #[test]
    fn engine_choose_matches_pokered_pick_move_for_each_rand() {
        // Two candidate moves (both neutral → tie at the initial score), one
        // empty slot. `choose_moves` flags slots 0 and 1.
        let enemy = enemy_with_moves([MoveId::Tackle, MoveId::Growl, MoveId::None, MoveId::None]);
        let player = player_normal();

        let provider =
            TrainerAiProvider::new(enemy.clone(), player.clone(), &[], 0);
        let state = BattleState::<TrainerAiProvider>::new(vec![], vec![]);

        // For several rand values, the engine `choose` must select the same move
        // as pokered's `pick_move(rand_val)`. Both consume `rand_val % count`.
        for rand_val in [0u8, 1, 2, 3, 7, 50, 100, 255] {
            let pokered_move = pokered_pick(&enemy, &player, rand_val);

            let mut rng = ScriptedRng::new(vec![rand_val]);
            let chosen = BattleAi::choose(&provider, &state, BattlerRef::OPPONENT, &mut rng);
            let engine_move = match chosen {
                BattleAction::Fight { move_ } => move_,
                other => panic!("expected Fight, got {other:?}"),
            };

            assert_eq!(
                engine_move, pokered_move,
                "rand_val={rand_val}: engine BattleAi::choose must match pick_move"
            );
        }
    }

    #[test]
    fn single_candidate_is_chosen_without_rng() {
        // Only one real move → exactly one candidate → unique best → no tie, so
        // `choose` makes no rng draw (matching a 1-candidate `pick_move`).
        let enemy = enemy_with_moves([MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None]);
        let player = player_normal();
        let provider = TrainerAiProvider::new(enemy.clone(), player.clone(), &[], 0);
        let state = BattleState::<TrainerAiProvider>::new(vec![], vec![]);

        let mut rng = ScriptedRng::new(vec![123]);
        let chosen = BattleAi::choose(&provider, &state, BattlerRef::OPPONENT, &mut rng);
        assert!(matches!(chosen, BattleAction::Fight { move_ } if move_ == MoveId::Tackle));
        assert_eq!(rng.consumed(), 0, "single candidate draws no tie-break rng");

        // And it matches pokered's pick for any rand (count == 1 → always idx 0).
        assert_eq!(pokered_pick(&enemy, &player, 200), MoveId::Tackle);
    }

    /// P0c parity on a **non-empty layer set** with a **non-trivial winner
    /// subset** — closing the gap where only the degenerate all-tied case was
    /// covered. Drives BOTH the engine path (`BattleAi::choose` over
    /// [`TrainerAiProvider`]) and the production path (`choose_moves` +
    /// `pick_move`, exactly as `battle/mod.rs::pick_enemy_move` runs them) for
    /// the same state + same scripted rng, and asserts the chosen move AND
    /// `rng.consumed()` agree.
    ///
    /// Scenario mirrors `move_choice_tests::layer3_encourages_super_effective`
    /// but with **two** super-effective Electric moves so Layer3 narrows the
    /// candidates to a proper subset `{1, 2}` (both score 9, the minimum) while
    /// the neutral Normal moves in slots 0/3 are dropped (score 10). That
    /// 2-candidate tie is what forces the engine's rng tie-break to fire, so
    /// the layer scoring is genuinely exercised through the engine — not the
    /// flat all-tied path.
    #[test]
    fn engine_choose_matches_pokered_pick_on_non_empty_layer3() {
        let layers = [MoveChoiceLayer::Layer3];

        // Electric attacker: slots 1 (Thunderbolt) and 2 (Thunder) are
        // super-effective vs Water; slots 0/3 (Normal) are neutral.
        let enemy = new_battler_state(vec![make_typed_mon(
            [
                MoveId::Tackle,
                MoveId::Thunderbolt,
                MoveId::Thunder,
                MoveId::Pound,
            ],
            PokemonType::Electric,
            PokemonType::Electric,
        )]);
        let player = new_battler_state(vec![make_typed_mon(
            [MoveId::Surf, MoveId::None, MoveId::None, MoveId::None],
            PokemonType::Water,
            PokemonType::Water,
        )]);

        // Sanity: Layer3 must actually narrow to a *non-trivial* subset (not all
        // four, not a single move) — otherwise this would be just another
        // degenerate case and prove nothing about the layer path.
        let candidates = choose_moves(&layers, &enemy, &player, 0).candidates;
        assert_eq!(
            candidates,
            [0, 1, 1, 0],
            "Layer3 should flag exactly the two super-effective moves"
        );

        let provider = TrainerAiProvider::new(enemy.clone(), player.clone(), &layers, 0);
        let state = BattleState::<TrainerAiProvider>::new(vec![], vec![]);

        // Two candidates → a tie the engine must break with exactly one rng
        // draw (`rand_val % 2`), the same modulo `pick_move` applies.
        for rand_val in [0u8, 1, 2, 3, 7, 50, 100, 255] {
            let pokered_move = pokered_pick_layered(&layers, &enemy, &player, 0, rand_val);

            let mut rng = ScriptedRng::new(vec![rand_val]);
            let chosen = BattleAi::choose(&provider, &state, BattlerRef::OPPONENT, &mut rng);
            let engine_move = match chosen {
                BattleAction::Fight { move_ } => move_,
                other => panic!("expected Fight, got {other:?}"),
            };

            assert_eq!(
                engine_move, pokered_move,
                "rand_val={rand_val}: engine choose must match production pick on Layer3"
            );
            assert_eq!(
                rng.consumed(),
                1,
                "rand_val={rand_val}: a 2-candidate tie must draw exactly one rng value, \
                 matching production's single `rand::random::<u8>()` feeding `pick_move`"
            );
        }
    }
}
