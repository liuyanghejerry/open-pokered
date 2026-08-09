use serde::{Deserialize, Serialize};

pub const HOF_MON_SIZE: usize = 16;
pub const HOF_TEAM_SIZE: usize = 6 * HOF_MON_SIZE;
pub const HOF_TEAM_CAPACITY: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HofMon {
    pub species: u8,
    pub level: u8,
    pub nickname: Vec<u8>,
}

impl HofMon {
    pub fn new(species: u8, level: u8, nickname: Vec<u8>) -> Self {
        Self {
            species,
            level,
            nickname,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HofTeam {
    pub mons: Vec<HofMon>,
}

impl HofTeam {
    pub fn new() -> Self {
        Self { mons: Vec::new() }
    }

    pub fn add_mon(&mut self, mon: HofMon) {
        if self.mons.len() < 6 {
            self.mons.push(mon);
        }
    }
}

impl Default for HofTeam {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HallOfFame {
    teams: Vec<HofTeam>,
}

impl HallOfFame {
    pub fn new() -> Self {
        Self { teams: Vec::new() }
    }

    pub fn push_team(&mut self, team: HofTeam) {
        if self.teams.len() >= HOF_TEAM_CAPACITY {
            self.teams.remove(0);
        }
        self.teams.push(team);
    }

    pub fn team_count(&self) -> usize {
        self.teams.len()
    }

    pub fn get_team(&self, index: usize) -> Option<&HofTeam> {
        self.teams.get(index)
    }

    pub fn clear(&mut self) {
        self.teams.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &HofTeam> {
        self.teams.iter()
    }
}

impl Default for HallOfFame {
    fn default() -> Self {
        Self::new()
    }
}
