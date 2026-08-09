use crate::battle::state::Pokemon;
use crate::pokemon::party::{Party, PartyError};
use pokered_data::species::Species;
use serde::{Deserialize, Serialize};

pub const MONS_PER_BOX: usize = 20;
pub const NUM_BOXES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxError {
    BoxFull,
    BoxEmpty,
    IndexOutOfBounds,
    InvalidBoxNumber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcBox {
    mons: Vec<Pokemon>,
}

impl PcBox {
    pub fn new() -> Self {
        Self { mons: Vec::new() }
    }

    pub fn count(&self) -> usize {
        self.mons.len()
    }

    pub fn is_full(&self) -> bool {
        self.mons.len() >= MONS_PER_BOX
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

    pub fn deposit(&mut self, pokemon: Pokemon) -> Result<usize, BoxError> {
        if self.is_full() {
            return Err(BoxError::BoxFull);
        }
        let index = self.mons.len();
        self.mons.push(pokemon);
        Ok(index)
    }

    pub fn withdraw(&mut self, index: usize) -> Result<Pokemon, BoxError> {
        if index >= self.mons.len() {
            return Err(BoxError::IndexOutOfBounds);
        }
        Ok(self.mons.remove(index))
    }

    pub fn release(&mut self, index: usize) -> Result<Pokemon, BoxError> {
        self.withdraw(index)
    }

    pub fn species_list(&self) -> Vec<Species> {
        self.mons.iter().map(|p| p.species).collect()
    }

    pub fn find_species(&self, species: Species) -> Option<usize> {
        self.mons.iter().position(|p| p.species == species)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pokemon> {
        self.mons.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pokemon> {
        self.mons.iter_mut()
    }
}

impl Default for PcBox {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcStorage {
    boxes: Vec<PcBox>,
    current_box: usize,
}

impl PcStorage {
    pub fn new() -> Self {
        Self {
            boxes: (0..NUM_BOXES).map(|_| PcBox::new()).collect(),
            current_box: 0,
        }
    }

    pub fn current_box_index(&self) -> usize {
        self.current_box
    }

    pub fn current_box(&self) -> &PcBox {
        &self.boxes[self.current_box]
    }

    pub fn current_box_mut(&mut self) -> &mut PcBox {
        &mut self.boxes[self.current_box]
    }

    pub fn change_box(&mut self, box_num: usize) -> Result<(), BoxError> {
        if box_num >= NUM_BOXES {
            return Err(BoxError::InvalidBoxNumber);
        }
        self.current_box = box_num;
        Ok(())
    }

    pub fn get_box(&self, box_num: usize) -> Result<&PcBox, BoxError> {
        self.boxes.get(box_num).ok_or(BoxError::InvalidBoxNumber)
    }

    pub fn get_box_mut(&mut self, box_num: usize) -> Result<&mut PcBox, BoxError> {
        self.boxes
            .get_mut(box_num)
            .ok_or(BoxError::InvalidBoxNumber)
    }

    pub fn deposit_to_current(&mut self, pokemon: Pokemon) -> Result<usize, BoxError> {
        self.boxes[self.current_box].deposit(pokemon)
    }

    pub fn withdraw_from_current(&mut self, index: usize) -> Result<Pokemon, BoxError> {
        self.boxes[self.current_box].withdraw(index)
    }

    pub fn deposit_from_party(
        &mut self,
        party: &mut Party,
        party_index: usize,
    ) -> Result<usize, BoxError> {
        if self.boxes[self.current_box].is_full() {
            return Err(BoxError::BoxFull);
        }
        let pokemon = party.remove(party_index).map_err(|e| match e {
            PartyError::CannotRemoveLast => BoxError::BoxEmpty,
            PartyError::IndexOutOfBounds => BoxError::IndexOutOfBounds,
            _ => BoxError::IndexOutOfBounds,
        })?;
        self.boxes[self.current_box].deposit(pokemon)
    }

    pub fn withdraw_to_party(
        &mut self,
        box_index: usize,
        party: &mut Party,
    ) -> Result<usize, BoxError> {
        if party.is_full() {
            return Err(BoxError::BoxFull);
        }
        let pokemon = self.boxes[self.current_box].withdraw(box_index)?;
        let party_idx = party.add(pokemon).map_err(|e| match e {
            PartyError::PartyFull => BoxError::BoxFull,
            _ => BoxError::IndexOutOfBounds,
        })?;
        Ok(party_idx)
    }

    pub fn total_stored(&self) -> usize {
        self.boxes.iter().map(|b| b.count()).sum()
    }

    pub fn box_count(&self) -> usize {
        NUM_BOXES
    }
}

impl Default for PcStorage {
    fn default() -> Self {
        Self::new()
    }
}
