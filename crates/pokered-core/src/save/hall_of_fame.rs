use serde::{Deserialize, Serialize};

pub const HOF_MON_SIZE: usize = 16;
pub const HOF_TEAM_SIZE: usize = 6 * HOF_MON_SIZE;
pub const HOF_TEAM_CAPACITY: usize = 50;
/// Species (1) + level (1) leave 14 bytes for the nickname — the SRAM
/// `sHallOfFame` entry layout (see `save/sram_layout.rs`).
pub const NICKNAME_LEN: usize = HOF_MON_SIZE - 2;
const NICK_TERMINATOR: u8 = 0x50;

/// serde glue: the nickname serializes as the active bytes only (up to the
/// 0x50 terminator), matching the variable-length `Vec<u8>` JSON shape of
/// older saves.
mod nickname_serde {
    use super::NICK_TERMINATOR;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; super::NICKNAME_LEN], s: S) -> Result<S::Ok, S::Error> {
        let active = bytes
            .iter()
            .position(|&b| b == NICK_TERMINATOR)
            .unwrap_or(bytes.len());
        (&bytes[..active]).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; super::NICKNAME_LEN], D::Error> {
        let bytes = Vec::<u8>::deserialize(d)?;
        let mut out = [NICK_TERMINATOR; super::NICKNAME_LEN];
        let len = bytes.len().min(out.len());
        out[..len].copy_from_slice(&bytes[..len]);
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HofMon {
    pub species: u8,
    pub level: u8,
    /// Charmap-encoded nickname, 0x50-terminated, fixed to the SRAM entry
    /// width (14 bytes).
    #[serde(with = "nickname_serde")]
    pub nickname: [u8; NICKNAME_LEN],
}

impl HofMon {
    pub fn new(species: u8, level: u8, nickname: &[u8]) -> Self {
        let mut nick = [NICK_TERMINATOR; NICKNAME_LEN];
        let len = nickname.len().min(nick.len());
        nick[..len].copy_from_slice(&nickname[..len]);
        Self {
            species,
            level,
            nickname: nick,
        }
    }

    /// The active nickname bytes (up to the 0x50 terminator).
    pub fn nickname_bytes(&self) -> &[u8] {
        let active = self
            .nickname
            .iter()
            .position(|&b| b == NICK_TERMINATOR)
            .unwrap_or(self.nickname.len());
        &self.nickname[..active]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HofTeam {
    /// Active mons first (`count` of them), the rest blank.
    mons: [HofMon; 6],
    count: usize,
}

impl HofTeam {
    pub fn new() -> Self {
        Self {
            mons: [HofMon {
                species: 0,
                level: 0,
                nickname: [NICK_TERMINATOR; NICKNAME_LEN],
            }; 6],
            count: 0,
        }
    }

    pub fn add_mon(&mut self, mon: HofMon) {
        if self.count < 6 {
            self.mons[self.count] = mon;
            self.count += 1;
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// The active members as a slice.
    pub fn mons(&self) -> &[HofMon] {
        &self.mons[..self.count]
    }

    pub fn iter(&self) -> impl Iterator<Item = &HofMon> {
        self.mons().iter()
    }
}

impl Default for HofTeam {
    fn default() -> Self {
        Self::new()
    }
}

// JSON shape preserved from the `Vec` era: an array of the active mons.
impl Serialize for HofTeam {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.count))?;
        for mon in self.mons() {
            seq.serialize_element(mon)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for HofTeam {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mons = Vec::<HofMon>::deserialize(deserializer)?;
        let mut team = Self::new();
        for mon in mons.into_iter().take(6) {
            team.add_mon(mon);
        }
        Ok(team)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HallOfFame {
    /// Oldest first; the SRAM window holds `HOF_TEAM_CAPACITY` teams.
    teams: [HofTeam; HOF_TEAM_CAPACITY],
    count: usize,
}

impl HallOfFame {
    pub fn new() -> Self {
        Self {
            teams: [HofTeam::new(); HOF_TEAM_CAPACITY],
            count: 0,
        }
    }

    pub fn push_team(&mut self, team: HofTeam) {
        if self.count >= HOF_TEAM_CAPACITY {
            // Full window: evict the oldest team (shift left).
            self.teams.copy_within(1.., 0);
            self.teams[HOF_TEAM_CAPACITY - 1] = team;
        } else {
            self.teams[self.count] = team;
            self.count += 1;
        }
    }

    pub fn team_count(&self) -> usize {
        self.count
    }

    pub fn get_team(&self, index: usize) -> Option<&HofTeam> {
        if index >= self.count {
            None
        } else {
            Some(&self.teams[index])
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn iter(&self) -> impl Iterator<Item = &HofTeam> {
        self.teams[..self.count].iter()
    }
}

impl Default for HallOfFame {
    fn default() -> Self {
        Self::new()
    }
}

// JSON shape preserved from the `Vec` era: an array of teams.
impl Serialize for HallOfFame {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.count))?;
        for team in self.iter() {
            seq.serialize_element(team)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for HallOfFame {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let teams = Vec::<HofTeam>::deserialize(deserializer)?;
        let mut hof = Self::new();
        for team in teams.into_iter().take(HOF_TEAM_CAPACITY) {
            hof.push_team(team);
        }
        Ok(hof)
    }
}
