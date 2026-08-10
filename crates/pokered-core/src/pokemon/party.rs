use crate::battle::state::Pokemon;
use crate::pokemon::ask_name::AskNameState;
use crate::pokemon::stats::create_pokemon;
use pokered_data::moves::MoveId;
use pokered_data::species::Species;
use serde::{Deserialize, Serialize};

pub const PARTY_LENGTH: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyError {
    PartyFull,
    PartyEmpty,
    IndexOutOfBounds,
    CannotRemoveLast,
    SameIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Party {
    mons: [Pokemon; PARTY_LENGTH],
    count: usize,
}

impl Party {
    pub fn new() -> Self {
        Self {
            mons: [crate::battle::state::blank_pokemon(); PARTY_LENGTH],
            count: 0,
        }
    }

    pub fn from_pokemon(pokemon: Vec<Pokemon>) -> Result<Self, PartyError> {
        if pokemon.len() > PARTY_LENGTH {
            return Err(PartyError::PartyFull);
        }
        Ok(Self::from(pokemon))
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_full(&self) -> bool {
        self.count >= PARTY_LENGTH
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, index: usize) -> Option<&Pokemon> {
        self.mons.get(index).filter(|_| index < self.count)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Pokemon> {
        if index >= self.count {
            return None;
        }
        self.mons.get_mut(index)
    }

    /// Mutable references to two distinct members (indices must differ) —
    /// used by the SOFTBOILED field move, which touches the user and the
    /// target in one step.
    pub fn get_two_mut(&mut self, a: usize, b: usize) -> Option<(&mut Pokemon, &mut Pokemon)> {
        if a == b || a >= self.count || b >= self.count {
            return None;
        }
        if a < b {
            let (left, right) = self.mons.split_at_mut(b);
            Some((left.get_mut(a)?, right.first_mut()?))
        } else {
            let (left, right) = self.mons.split_at_mut(a);
            Some((right.first_mut()?, left.get_mut(b)?))
        }
    }

    pub fn leader(&self) -> Option<&Pokemon> {
        if self.count == 0 {
            None
        } else {
            Some(&self.mons[0])
        }
    }

    pub fn leader_level(&self) -> u8 {
        if self.count == 0 {
            0
        } else {
            self.mons[0].level
        }
    }

    pub fn add(&mut self, pokemon: Pokemon) -> Result<usize, PartyError> {
        if self.is_full() {
            return Err(PartyError::PartyFull);
        }
        let index = self.count;
        self.mons[index] = pokemon;
        self.count += 1;
        Ok(index)
    }

    pub fn add_with_naming(
        &mut self,
        species: Species,
        level: u8,
    ) -> Result<(usize, AskNameState), PartyError> {
        if self.is_full() {
            return Err(PartyError::PartyFull);
        }

        let pokemon =
            create_pokemon(species, level, [0xFF, 0xFF]).ok_or(PartyError::IndexOutOfBounds)?;

        let ask_name = AskNameState::new(species);
        let index = self.count;
        self.mons[index] = pokemon;
        self.count += 1;
        Ok((index, ask_name))
    }

    pub fn set_nickname(&mut self, index: usize, nickname: &str) -> Result<(), PartyError> {
        if index >= self.count {
            return Err(PartyError::IndexOutOfBounds);
        }
        self.mons[index].set_nickname(nickname);
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<Pokemon, PartyError> {
        if index >= self.count {
            return Err(PartyError::IndexOutOfBounds);
        }
        if self.count <= 1 {
            return Err(PartyError::CannotRemoveLast);
        }
        Ok(self.take(index))
    }

    /// Remove a party member WITHOUT the last-mon guard.
    ///
    /// Gen 1 allows trading away the last party member in the Cable Club: the
    /// trade menu lists every party slot (`engine/link/cable_club.asm`,
    /// `TradeCenter_SelectMon`: `wMaxMenuItem = wPartyCount`) and `.doTrade`
    /// calls `RemovePokemon` with no party-count check. Used by the link trade
    /// driver; every other removal path keeps the guard.
    pub fn remove_for_trade(&mut self, index: usize) -> Result<Pokemon, PartyError> {
        if index >= self.count {
            return Err(PartyError::IndexOutOfBounds);
        }
        Ok(self.take(index))
    }

    /// Pop `mons[index]` out of the active region, shifting later members
    /// left and blanking the vacated slot.
    fn take(&mut self, index: usize) -> Pokemon {
        let mon = self.mons[index]; // Pokemon: Copy
        for i in index..self.count - 1 {
            self.mons[i] = self.mons[i + 1];
        }
        self.count -= 1;
        self.mons[self.count] = crate::battle::state::blank_pokemon();
        mon
    }

    pub fn swap(&mut self, a: usize, b: usize) -> Result<(), PartyError> {
        if a == b {
            return Err(PartyError::SameIndex);
        }
        if a >= self.count || b >= self.count {
            return Err(PartyError::IndexOutOfBounds);
        }
        self.mons.swap(a, b);
        Ok(())
    }

    pub fn species_list(&self) -> Vec<Species> {
        self.iter().map(|p| p.species).collect()
    }

    pub fn find_species(&self, species: Species) -> Option<usize> {
        self.iter().position(|p| p.species == species)
    }

    pub fn alive_count(&self) -> usize {
        self.iter().filter(|p| p.hp > 0).count()
    }

    pub fn all_fainted(&self) -> bool {
        !self.is_empty() && self.iter().all(|p| p.hp == 0)
    }

    pub fn first_alive_index(&self) -> Option<usize> {
        self.iter().position(|p| p.hp > 0)
    }

    pub fn heal_all(&mut self) {
        for mon in self.iter_mut() {
            mon.hp = mon.max_hp;
            mon.status = crate::battle::state::StatusCondition::None;
            for i in 0..4 {
                if mon.moves[i] != MoveId::None {
                    mon.pp[i] = crate::items::pp_restore::get_max_pp_with_ups(
                        mon.moves[i],
                        mon.pp_ups[i],
                    );
                }
            }
        }
    }

    pub fn to_vec(&self) -> Vec<Pokemon> {
        self.mons[..self.count].to_vec()
    }

    pub fn into_vec(self) -> Vec<Pokemon> {
        self.mons[..self.count].to_vec()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pokemon> {
        self.mons[..self.count].iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pokemon> {
        let count = self.count;
        self.mons[..count].iter_mut()
    }
}

impl Default for Party {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<Pokemon>> for Party {
    fn from(mons: Vec<Pokemon>) -> Self {
        debug_assert!(mons.len() <= PARTY_LENGTH);
        let mut party = Self::new();
        for mon in mons.into_iter().take(PARTY_LENGTH) {
            party.mons[party.count] = mon;
            party.count += 1;
        }
        party
    }
}

impl From<Party> for Vec<Pokemon> {
    fn from(party: Party) -> Self {
        party.into_vec()
    }
}

// JSON shape preserved from the `Vec` era: a plain array of the active mons
// (no `count`/`mons` wrapper), so old snapshots and the editor tooling that
// reads/writes them are unaffected by the fixed-capacity storage.
impl Serialize for Party {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.count))?;
        for mon in self.iter() {
            seq.serialize_element(mon)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Party {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mons = Vec::<Pokemon>::deserialize(deserializer)?;
        if mons.len() > PARTY_LENGTH {
            return Err(serde::de::Error::custom("party exceeds 6 members"));
        }
        let mut party = Self::new();
        for mon in mons {
            party.mons[party.count] = mon;
            party.count += 1;
        }
        Ok(party)
    }
}
