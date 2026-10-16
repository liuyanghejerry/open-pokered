//! Original faint-handler entry, separate from the structural HP-zero receipt.
//! e533 StackDriver diffs the actual action target before its actor. The first
//! Fainted receipt is therefore the core.asm handler entered, not FirstMover.
use super::pokered_rules::PokeredRules;
use crate::alloc_prelude::*;
use dotzuki_engine::battle::stack::{TurnEvent, TurnLog};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaintSide {
    Player,
    Enemy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaintSound {
    OwnCry,
    EnemyFall,
    WildVictory,
}

/// One existing presentation action. Text may belong to a different side:
/// HandleEnemyMonFainted removes the player silently BEFORE EnemyFaintedText.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaintCue {
    pub side: FaintSide,
    pub sound: Option<FaintSound>,
    pub text: Option<FaintSide>,
}

pub(super) struct FaintPlan {
    // None means that original control flow never enters this faint routine.
    entries: Vec<(usize, Option<FaintCue>)>,
}

impl FaintPlan {
    pub(super) fn from_turn(
        log: &TurnLog<PokeredRules>,
        player_party_alive: bool,
        wild: bool,
    ) -> Self {
        let receipts: Vec<_> = log
            .events
            .iter()
            .enumerate()
            .filter_map(|(i, event)| {
                if let TurnEvent::Fainted { who } = event {
                    Some((
                        i,
                        if who.side == 0 {
                            FaintSide::Player
                        } else {
                            FaintSide::Enemy
                        },
                    ))
                } else {
                    None
                }
            })
            .collect();
        let enemy_sound = if wild {
            FaintSound::WildVictory
        } else {
            FaintSound::EnemyFall
        };
        let mut entries: Vec<_> = receipts
            .iter()
            .map(|&(i, side)| {
                (
                    i,
                    Some(FaintCue {
                        side,
                        sound: Some(if side == FaintSide::Player {
                            FaintSound::OwnCry
                        } else {
                            enemy_sound
                        }),
                        text: Some(side),
                    }),
                )
            })
            .collect();
        // This is an actual ordered pair of faint-handler receipts, not a test
        // of final HP or a move-name/cause guess. Residual single-KOs keep their
        // own source-tagged line and ordinary one-side handler unchanged.
        if let [(first_i, first), (second_i, second)] = receipts.as_slice() {
            let same_handler = !log.events[*first_i + 1..*second_i].iter().any(|event| {
                matches!(
                    event,
                    TurnEvent::MoveUsed { .. } | TurnEvent::Blocked { .. }
                )
            });
            if same_handler && first != second {
                if *first == FaintSide::Enemy {
                    // FaintEnemyPokemon -> RemoveFaintedPlayerMon(flag=0) ->
                    // AnyPartyAlive -> EnemyFaintedText, only if somebody lives.
                    entries[0].1.as_mut().unwrap().text = None;
                    let own = entries[1].1.as_mut().unwrap();
                    own.sound = None;
                    own.text = player_party_alive.then_some(FaintSide::Enemy);
                } else if !player_party_alive {
                    // HandlePlayerMonFainted blackouts BEFORE FaintEnemyPokemon.
                    entries[1].1 = None;
                }
            }
        }
        Self { entries }
    }

    pub(super) fn at(&self, event: usize) -> Option<FaintCue> {
        self.entries
            .iter()
            .find(|(i, _)| *i == event)
            .and_then(|(_, cue)| *cue)
    }
}
