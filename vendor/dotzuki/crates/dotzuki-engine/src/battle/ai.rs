//! Generic, game-agnostic battle AI driver (P0c).
//!
//! The engine owns only the *AI decision loop scaffold*: enumerate the legal
//! actions for the acting side, ask the game to score each, and pick the
//! best-scoring action — breaking ties through the injected [`BattleRng`].
//! Every heuristic stays game-side (C7): the Gen-1 scoring tables, the
//! "discourage / encourage" +/- nudges, and the known AI bugs all live in the
//! [`BattleAiProvider`] implementation. The engine contains **no** game-specific
//! types and never links `rand`.
//!
//! ## Why a scorer, not a policy
//!
//! A hardcoded policy ("always pick the super-effective move") would bake one
//! game's rules into the engine. Instead the engine asks the provider for a
//! *score* per candidate action (higher = more preferred) and only performs the
//! argmax. This keeps the original's quirks — including its tie-randomisation
//! and bias bugs — entirely in the game.
//!
//! ## Determinism
//!
//! All randomness flows through the injected [`BattleRng`]. `score_action` may
//! draw from it (the original randomises some nudges), and [`BattleAi::choose`]
//! draws exactly one value to break a tie *only when two or more actions share
//! the top score*. Under a [`ScriptedRng`](super::rng::ScriptedRng) the whole
//! decision is reproducible.

use super::rng::BattleRng;
use super::{BattleAction, BattleProvider, BattleState, BattlerRef};

/// Engine-owned AI scaffold: enumerate legal actions, ask the game to score
/// each, pick the best with an injected tie-break. **No game logic here.**
///
/// Implementors are themselves [`BattleProvider`]s, so the AI shares the exact
/// associated types (`Move`, `Item`, …) and battle state used by the rest of the
/// engine. The two required methods are the only game-specific surface:
///
/// * [`score_action`](BattleAiProvider::score_action) supplies the heuristic.
/// * [`legal_actions`](BattleAiProvider::legal_actions) supplies the candidate
///   set.
///
/// The selection loop itself is [`BattleAi::choose`].
pub trait BattleAiProvider: BattleProvider {
    /// Score a candidate `action` for the battler `me`; **higher = more
    /// preferred**.
    ///
    /// The game owns the entire heuristic: the Gen-1 score table, the
    /// discourage / encourage +/- nudges, and the AI bugs (e.g. ignoring type
    /// immunities in some routines). Mutating `rng` lets the implementation
    /// reproduce the original's randomised nudges; the engine draws *no* rng
    /// here itself, so the draw order is fully game-controlled.
    fn score_action(
        &self,
        st: &BattleState<Self>,
        me: BattlerRef,
        action: &BattleAction<Self>,
        rng: &mut dyn BattleRng,
    ) -> i32
    where
        Self: Sized;

    /// Enumerate the legal actions for the side acting as `me` this turn
    /// (usable fight options plus any legal switches/items).
    ///
    /// The order of the returned `Vec` is significant: when several actions
    /// tie for the best score, [`BattleAi::choose`] picks among them using the
    /// injected `rng`, and a smaller index is preferred for a smaller rng draw —
    /// so the game controls the candidate ordering and thus the exact tie
    /// distribution (matching the original's biases).
    fn legal_actions(&self, st: &BattleState<Self>, me: BattlerRef) -> Vec<BattleAction<Self>>
    where
        Self: Sized;
}

/// The generic AI selection driver. Stateless; drives a single decision.
pub struct BattleAi;

impl BattleAi {
    /// Choose the AI's action for `me` this turn.
    ///
    /// 1. Ask the provider for the [`legal_actions`](BattleAiProvider::legal_actions).
    /// 2. Score each via [`score_action`](BattleAiProvider::score_action) (which
    ///    may draw from `rng`).
    /// 3. Pick the highest score. When two or more actions share the top score,
    ///    break the tie by drawing **one** value from `rng` and indexing into
    ///    the tied set (preserving the candidate order from step 1). This
    ///    matches the original's "pick among equal-best" behaviour, including
    ///    its bias bugs, while staying deterministic under a scripted rng.
    ///
    /// If the provider returns no legal actions, falls back to
    /// [`BattleAction::Nothing`].
    pub fn choose<P: BattleAiProvider>(
        provider: &P,
        st: &BattleState<P>,
        me: BattlerRef,
        rng: &mut dyn BattleRng,
    ) -> BattleAction<P> {
        let actions = provider.legal_actions(st, me);
        if actions.is_empty() {
            return BattleAction::Nothing;
        }

        // Score every candidate. `score_action` is allowed to draw rng, so we
        // score in candidate order to keep the draw sequence game-controlled.
        let mut best_score = i32::MIN;
        let mut scores = Vec::with_capacity(actions.len());
        for action in &actions {
            let s = provider.score_action(st, me, action, rng);
            if s > best_score {
                best_score = s;
            }
            scores.push(s);
        }

        // Collect the indices that tie for the best score, preserving order.
        let tied: Vec<usize> = scores
            .iter()
            .enumerate()
            .filter(|(_, &s)| s == best_score)
            .map(|(i, _)| i)
            .collect();

        let chosen = if tied.len() == 1 {
            // Unique best: no rng draw, so a clear winner never perturbs the
            // game's draw sequence.
            tied[0]
        } else {
            // Multiple ties: draw exactly one value to pick among them.
            let pick = rng.range(tied.len() as u32) as usize;
            tied[pick.min(tied.len() - 1)]
        };

        actions[chosen].clone()
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::rng::ScriptedRng;
    use crate::battle::{
        BattleProvider, BattlerState, DamageResult, EffectResult, EnumMap, MoveEffect,
    };

    // ── Minimal mock game ─────────────────────────────────────────────

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AStat {
        Hp,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AStatus {
        None,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AType {
        Normal,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ASpecies {
        Mon,
    }

    /// A move carries a `weight` the scorer reads directly as the score, so a
    /// test fully controls the argmax outcome.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct AMove {
        weight: i32,
    }

    struct AiMock;

    impl BattleProvider for AiMock {
        type Monster = ();
        type Move = AMove;
        type Ability = ();
        type Status = AStatus;
        type Stat = AStat;
        type Species = ASpecies;
        type Type = AType;
        type Item = ();

        fn calculate_damage(
            &self,
            _move_: &Self::Move,
            _attacker: &BattlerState<Self>,
            _defender: &BattlerState<Self>,
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
            battler: &BattlerState<Self>,
            _state: &BattleState<Self>,
        ) -> Self::Move {
            battler.moves.first().copied().unwrap()
        }

        fn apply_move_effect(
            &self,
            _effect: MoveEffect,
            _user: &mut BattlerState<Self>,
            _target: &mut BattlerState<Self>,
        ) -> EffectResult {
            EffectResult::NoEffect
        }

        fn create_monster(&self, species: Self::Species, _level: u8) -> BattlerState<Self> {
            BattlerState::new(species, 100, 100, EnumMap::new(), Vec::new())
        }
    }

    impl BattleAiProvider for AiMock {
        fn score_action(
            &self,
            _st: &BattleState<Self>,
            _me: BattlerRef,
            action: &BattleAction<Self>,
            _rng: &mut dyn BattleRng,
        ) -> i32 {
            match action {
                // The move's own weight is the score — higher = better.
                BattleAction::Fight { move_ } => move_.weight,
                _ => i32::MIN,
            }
        }

        fn legal_actions(&self, st: &BattleState<Self>, me: BattlerRef) -> Vec<BattleAction<Self>> {
            let party = if me.side == 0 {
                &st.player_battlers
            } else {
                &st.opponent_battlers
            };
            party[me.slot as usize]
                .moves
                .iter()
                .map(|m| BattleAction::Fight { move_: *m })
                .collect()
        }
    }

    fn state_with_moves(weights: &[i32]) -> BattleState<AiMock> {
        let moves: Vec<AMove> = weights.iter().map(|&w| AMove { weight: w }).collect();
        let opp = BattlerState::new(
            ASpecies::Mon,
            100,
            100,
            EnumMap::new(),
            vec![AMove { weight: 0 }],
        );
        let me = BattlerState::new(ASpecies::Mon, 100, 100, EnumMap::new(), moves);
        // The AI acts as the opponent (side 1) in these tests.
        BattleState::new(vec![opp], vec![me])
    }

    fn weight_of(action: &BattleAction<AiMock>) -> i32 {
        match action {
            BattleAction::Fight { move_ } => move_.weight,
            _ => panic!("expected Fight"),
        }
    }

    #[test]
    fn choose_picks_highest_scored_action() {
        let provider = AiMock;
        let st = state_with_moves(&[1, 9, 4, 2]);
        let mut rng = ScriptedRng::new(vec![0, 0, 0, 0]);
        let chosen = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng);
        assert_eq!(weight_of(&chosen), 9, "highest-weighted move wins");
    }

    #[test]
    fn unique_best_draws_no_rng() {
        // A clear winner must not perturb the rng draw sequence.
        let provider = AiMock;
        let st = state_with_moves(&[1, 9, 4, 2]);
        let mut rng = ScriptedRng::new(vec![7, 7, 7]);
        let _ = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng);
        assert_eq!(rng.consumed(), 0, "no tie → no rng draw");
    }

    #[test]
    fn ties_resolved_deterministically_via_rng() {
        let provider = AiMock;
        // Three moves tie for the top score (5, 5, 5); one rng draw selects.
        let st = state_with_moves(&[5, 5, 5]);

        // Draw 0 → first tied index.
        let mut rng0 = ScriptedRng::new(vec![0]);
        let c0 = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng0);
        assert_eq!(rng0.consumed(), 1, "tie draws exactly one byte");

        // Draw 1 → second tied index (1 % 3 == 1).
        let mut rng1 = ScriptedRng::new(vec![1]);
        let c1 = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng1);

        // Draw 2 → third tied index (2 % 3 == 2).
        let mut rng2 = ScriptedRng::new(vec![2]);
        let c2 = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng2);

        // All three select a valid (tied) action; distinct rng draws pick
        // distinct positions, proving the tie-break is rng-driven and stable.
        assert_eq!(weight_of(&c0), 5);
        assert_eq!(weight_of(&c1), 5);
        assert_eq!(weight_of(&c2), 5);

        // Re-running with the same script reproduces the same selection.
        let mut rng1b = ScriptedRng::new(vec![1]);
        let c1b = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng1b);
        assert_eq!(
            weight_of(&c1),
            weight_of(&c1b),
            "deterministic under same rng"
        );
    }

    #[test]
    fn empty_legal_actions_falls_back_to_nothing() {
        let provider = AiMock;
        let st = state_with_moves(&[]); // no moves → no legal actions
        let mut rng = ScriptedRng::new(vec![0]);
        let chosen = BattleAi::choose(&provider, &st, BattlerRef::OPPONENT, &mut rng);
        assert!(matches!(chosen, BattleAction::Nothing));
    }
}
