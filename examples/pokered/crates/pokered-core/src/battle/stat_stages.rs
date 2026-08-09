use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatStages {
    pub attack: i8,
    pub defense: i8,
    pub speed: i8,
    pub special: i8,
    pub accuracy: i8,
    pub evasion: i8,
}

impl Default for StatStages {
    fn default() -> Self {
        Self {
            attack: 0,
            defense: 0,
            speed: 0,
            special: 0,
            accuracy: 0,
            evasion: 0,
        }
    }
}

const STAGE_MULTIPLIERS_NUMERATOR: [u16; 13] =
    [25, 28, 33, 40, 50, 66, 100, 150, 200, 250, 300, 350, 400];
const STAGE_MULTIPLIERS_DENOMINATOR: u16 = 100;

impl StatStages {
    pub fn modify(&mut self, stat: StatIndex, delta: i8) -> bool {
        let stage = self.get_mut(stat);
        let old = *stage;
        *stage = (*stage + delta).clamp(-6, 6);
        *stage != old
    }

    pub fn get(&self, stat: StatIndex) -> i8 {
        match stat {
            StatIndex::Attack => self.attack,
            StatIndex::Defense => self.defense,
            StatIndex::Speed => self.speed,
            StatIndex::Special => self.special,
            StatIndex::Accuracy => self.accuracy,
            StatIndex::Evasion => self.evasion,
        }
    }

    fn get_mut(&mut self, stat: StatIndex) -> &mut i8 {
        match stat {
            StatIndex::Attack => &mut self.attack,
            StatIndex::Defense => &mut self.defense,
            StatIndex::Speed => &mut self.speed,
            StatIndex::Special => &mut self.special,
            StatIndex::Accuracy => &mut self.accuracy,
            StatIndex::Evasion => &mut self.evasion,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn apply_stage(base_stat: u16, stage: i8) -> u16 {
    let index = (stage + 6) as usize;
    let num = STAGE_MULTIPLIERS_NUMERATOR[index];
    (base_stat as u32 * num as u32 / STAGE_MULTIPLIERS_DENOMINATOR as u32) as u16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatIndex {
    Attack,
    Defense,
    Speed,
    Special,
    Accuracy,
    Evasion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stages_are_zero() {
        let stages = StatStages::default();
        assert_eq!(stages.attack, 0);
        assert_eq!(stages.defense, 0);
        assert_eq!(stages.speed, 0);
        assert_eq!(stages.special, 0);
        assert_eq!(stages.accuracy, 0);
        assert_eq!(stages.evasion, 0);
    }

    #[test]
    fn apply_stage_zero_returns_base() {
        assert_eq!(apply_stage(100, 0), 100);
    }

    #[test]
    fn apply_stage_positive() {
        assert_eq!(apply_stage(100, 1), 150);
        assert_eq!(apply_stage(100, 2), 200);
        assert_eq!(apply_stage(100, 6), 400);
    }

    #[test]
    fn apply_stage_negative() {
        assert_eq!(apply_stage(100, -1), 66);
        assert_eq!(apply_stage(100, -2), 50);
        assert_eq!(apply_stage(100, -6), 25);
    }

    #[test]
    fn modify_clamps_at_bounds() {
        let mut stages = StatStages::default();
        stages.attack = 5;
        let changed = stages.modify(StatIndex::Attack, 2);
        assert!(changed);
        assert_eq!(stages.attack, 6);

        let changed = stages.modify(StatIndex::Attack, 1);
        assert!(!changed);
        assert_eq!(stages.attack, 6);
    }

    #[test]
    fn modify_clamps_at_lower_bound() {
        let mut stages = StatStages::default();
        stages.defense = -5;
        let changed = stages.modify(StatIndex::Defense, -2);
        assert!(changed);
        assert_eq!(stages.defense, -6);

        let changed = stages.modify(StatIndex::Defense, -1);
        assert!(!changed);
        assert_eq!(stages.defense, -6);
    }

    #[test]
    fn reset_clears_all() {
        let mut stages = StatStages::default();
        stages.attack = 3;
        stages.special = -2;
        stages.reset();
        assert_eq!(stages.attack, 0);
        assert_eq!(stages.special, 0);
    }
}
