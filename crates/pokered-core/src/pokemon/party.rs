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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Party {
    mons: Vec<Pokemon>,
}

impl Party {
    pub fn new() -> Self {
        Self { mons: Vec::new() }
    }

    pub fn from_pokemon(pokemon: Vec<Pokemon>) -> Result<Self, PartyError> {
        if pokemon.len() > PARTY_LENGTH {
            return Err(PartyError::PartyFull);
        }
        Ok(Self { mons: pokemon })
    }

    pub fn count(&self) -> usize {
        self.mons.len()
    }

    pub fn is_full(&self) -> bool {
        self.mons.len() >= PARTY_LENGTH
    }

    pub fn is_empty(&self) -> bool {
        self.mons.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Pokemon> {
        self.mons.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Pokemon> {
        self.mons.get_mut(index)
    }

    /// Mutable references to two distinct members (indices must differ) —
    /// used by the SOFTBOILED field move, which touches the user and the
    /// target in one step.
    pub fn get_two_mut(&mut self, a: usize, b: usize) -> Option<(&mut Pokemon, &mut Pokemon)> {
        if a == b {
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
        self.mons.first()
    }

    pub fn leader_level(&self) -> u8 {
        self.mons.first().map_or(0, |p| p.level)
    }

    pub fn add(&mut self, pokemon: Pokemon) -> Result<usize, PartyError> {
        if self.is_full() {
            return Err(PartyError::PartyFull);
        }
        let index = self.mons.len();
        self.mons.push(pokemon);
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
        let index = self.mons.len();
        self.mons.push(pokemon);
        Ok((index, ask_name))
    }

    pub fn set_nickname(&mut self, index: usize, nickname: String) -> Result<(), PartyError> {
        if index >= self.mons.len() {
            return Err(PartyError::IndexOutOfBounds);
        }
        self.mons[index].set_nickname(nickname);
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<Pokemon, PartyError> {
        if index >= self.mons.len() {
            return Err(PartyError::IndexOutOfBounds);
        }
        if self.mons.len() <= 1 {
            return Err(PartyError::CannotRemoveLast);
        }
        Ok(self.mons.remove(index))
    }

    /// Remove a party member WITHOUT the last-mon guard.
    ///
    /// Gen 1 allows trading away the last party member in the Cable Club: the
    /// trade menu lists every party slot (`engine/link/cable_club.asm`,
    /// `TradeCenter_SelectMon`: `wMaxMenuItem = wPartyCount`) and `.doTrade`
    /// calls `RemovePokemon` with no party-count check. Used by the link trade
    /// driver; every other removal path keeps the guard.
    pub fn remove_for_trade(&mut self, index: usize) -> Result<Pokemon, PartyError> {
        if index >= self.mons.len() {
            return Err(PartyError::IndexOutOfBounds);
        }
        Ok(self.mons.remove(index))
    }

    pub fn swap(&mut self, a: usize, b: usize) -> Result<(), PartyError> {
        if a == b {
            return Err(PartyError::SameIndex);
        }
        let len = self.mons.len();
        if a >= len || b >= len {
            return Err(PartyError::IndexOutOfBounds);
        }
        self.mons.swap(a, b);
        Ok(())
    }

    pub fn species_list(&self) -> Vec<Species> {
        self.mons.iter().map(|p| p.species).collect()
    }

    pub fn find_species(&self, species: Species) -> Option<usize> {
        self.mons.iter().position(|p| p.species == species)
    }

    pub fn alive_count(&self) -> usize {
        self.mons.iter().filter(|p| p.hp > 0).count()
    }

    pub fn all_fainted(&self) -> bool {
        !self.mons.is_empty() && self.mons.iter().all(|p| p.hp == 0)
    }

    pub fn first_alive_index(&self) -> Option<usize> {
        self.mons.iter().position(|p| p.hp > 0)
    }

    pub fn heal_all(&mut self) {
        for mon in &mut self.mons {
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
        self.mons.clone()
    }

    pub fn into_vec(self) -> Vec<Pokemon> {
        self.mons
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pokemon> {
        self.mons.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pokemon> {
        self.mons.iter_mut()
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
        Self { mons }
    }
}

impl From<Party> for Vec<Pokemon> {
    fn from(party: Party) -> Self {
        party.mons
    }
}
