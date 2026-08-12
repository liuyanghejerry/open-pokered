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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcBox {
    mons: [Pokemon; MONS_PER_BOX],
    count: usize,
}

impl PcBox {
    pub fn new() -> Self {
        Self {
            mons: [crate::battle::state::blank_pokemon(); MONS_PER_BOX],
            count: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_full(&self) -> bool {
        self.count >= MONS_PER_BOX
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

    pub fn deposit(&mut self, pokemon: Pokemon) -> Result<usize, BoxError> {
        if self.is_full() {
            return Err(BoxError::BoxFull);
        }
        let index = self.count;
        self.mons[index] = pokemon;
        self.count += 1;
        Ok(index)
    }

    pub fn withdraw(&mut self, index: usize) -> Result<Pokemon, BoxError> {
        if index >= self.count {
            return Err(BoxError::IndexOutOfBounds);
        }
        let mon = self.mons[index]; // Pokemon: Copy
        for i in index..self.count - 1 {
            self.mons[i] = self.mons[i + 1];
        }
        self.count -= 1;
        self.mons[self.count] = crate::battle::state::blank_pokemon();
        Ok(mon)
    }

    pub fn release(&mut self, index: usize) -> Result<Pokemon, BoxError> {
        self.withdraw(index)
    }

    pub fn species_list(&self) -> Vec<Species> {
        self.iter().map(|p| p.species).collect()
    }

    pub fn find_species(&self, species: Species) -> Option<usize> {
        self.iter().position(|p| p.species == species)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pokemon> {
        self.mons[..self.count].iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pokemon> {
        let count = self.count;
        self.mons[..count].iter_mut()
    }
}

impl Default for PcBox {
    fn default() -> Self {
        Self::new()
    }
}

// JSON shape preserved from the `Vec` era: a plain array of the active mons.
impl Serialize for PcBox {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.count))?;
        for mon in self.iter() {
            seq.serialize_element(mon)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for PcBox {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mons = Vec::<Pokemon>::deserialize(deserializer)?;
        if mons.len() > MONS_PER_BOX {
            return Err(serde::de::Error::custom("box exceeds 20 members"));
        }
        let mut box_data = Self::new();
        for mon in mons {
            box_data.mons[box_data.count] = mon;
            box_data.count += 1;
        }
        Ok(box_data)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PcStorage {
    boxes: [PcBox; NUM_BOXES],
    current_box: usize,
}

impl PcStorage {
    pub fn new() -> Self {
        Self {
            boxes: [PcBox::new(); NUM_BOXES],
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
