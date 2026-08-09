//! Trainer card screen state machine.
//!
//! Replicates `StartMenu_TrainerInfo` / `DrawTrainerInfo`
//! (engine/menus/start_sub_menus.asm:453-565): a read-only card showing the
//! player name, money, play time (hours:minutes), and the eight gym badges
//! (`DrawBadges`, engine/menus/draw_badges.asm — a badge slot shows the gym
//! leader's face until the badge is owned). A or B dismisses it
//! (`WaitForTextScrollButtonPress`).
//!
//! The card has no cursor, so the state only tracks open/closed; the frontends
//! pull name/money/time/badges from the save at render time.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrainerCardInput {
    pub a: bool,
    pub b: bool,
}

impl TrainerCardInput {
    pub fn none() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainerCardAction {
    /// Still open.
    Active,
    /// Dismissed — back to the start menu (`RedisplayStartMenu`).
    Closed,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerCardScreenState;

impl TrainerCardScreenState {
    pub fn new() -> Self {
        Self
    }

    pub fn update_frame(&mut self, input: TrainerCardInput) -> TrainerCardAction {
        if input.a || input.b {
            TrainerCardAction::Closed
        } else {
            TrainerCardAction::Active
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_or_b_closes() {
        let mut s = TrainerCardScreenState::new();
        assert_eq!(
            s.update_frame(TrainerCardInput::none()),
            TrainerCardAction::Active
        );
        assert_eq!(
            s.update_frame(TrainerCardInput {
                a: true,
                ..Default::default()
            }),
            TrainerCardAction::Closed
        );
        let mut s = TrainerCardScreenState::new();
        assert_eq!(
            s.update_frame(TrainerCardInput {
                b: true,
                ..Default::default()
            }),
            TrainerCardAction::Closed
        );
    }
}
