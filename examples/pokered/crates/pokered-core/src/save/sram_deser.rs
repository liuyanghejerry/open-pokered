use super::game_data::*;
use super::serialization::SaveError;

pub struct SramReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> SramReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn read_u8(&mut self) -> Result<u8, SaveError> {
        if self.pos >= self.data.len() {
            return Err(SaveError::DataTooShort);
        }
        let val = self.data[self.pos];
        self.pos += 1;
        Ok(val)
    }

    pub fn read_u16_be(&mut self) -> Result<u16, SaveError> {
        let hi = self.read_u8()? as u16;
        let lo = self.read_u8()? as u16;
        Ok((hi << 8) | lo)
    }

    pub fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], SaveError> {
        if self.pos + count > self.data.len() {
            return Err(SaveError::DataTooShort);
        }
        let slice = &self.data[self.pos..self.pos + count];
        self.pos += count;
        Ok(slice)
    }

    pub fn skip(&mut self, count: usize) -> Result<(), SaveError> {
        if self.pos + count > self.data.len() {
            return Err(SaveError::DataTooShort);
        }
        self.pos += count;
        Ok(())
    }

    pub fn read_name(&mut self) -> Result<Vec<u8>, SaveError> {
        let raw = self.read_bytes(NAME_LENGTH)?;
        let mut name = Vec::new();
        for &b in raw {
            if b == 0x50 {
                break;
            }
            name.push(b);
        }
        Ok(name)
    }

    pub fn read_bcd_money(&mut self) -> Result<u32, SaveError> {
        let b0 = self.read_u8()?;
        let b1 = self.read_u8()?;
        let b2 = self.read_u8()?;
        let d5 = (b0 >> 4) as u32;
        let d4 = (b0 & 0x0F) as u32;
        let d3 = (b1 >> 4) as u32;
        let d2 = (b1 & 0x0F) as u32;
        let d1 = (b2 >> 4) as u32;
        let d0 = (b2 & 0x0F) as u32;
        Ok(d5 * 100000 + d4 * 10000 + d3 * 1000 + d2 * 100 + d1 * 10 + d0)
    }

    pub fn read_bcd_coins(&mut self) -> Result<u16, SaveError> {
        let b0 = self.read_u8()?;
        let b1 = self.read_u8()?;
        let d3 = (b0 >> 4) as u16;
        let d2 = (b0 & 0x0F) as u16;
        let d1 = (b1 >> 4) as u16;
        let d0 = (b1 & 0x0F) as u16;
        Ok(d3 * 1000 + d2 * 100 + d1 * 10 + d0)
    }

    pub fn read_map_connection(&mut self) -> Result<MapConnection, SaveError> {
        Ok(MapConnection {
            connected_map: self.read_u8()?,
            strip_src: self.read_u16_be()?,
            strip_dest: self.read_u16_be()?,
            strip_length: self.read_u8()?,
            connected_width: self.read_u8()?,
            y_alignment: self.read_u8()?,
            x_alignment: self.read_u8()?,
            view_pointer: self.read_u16_be()?,
        })
    }

    pub fn read_inventory(
        &mut self,
        capacity: usize,
    ) -> Result<crate::items::inventory::Inventory, SaveError> {
        let count = self.read_u8()? as usize;
        let mut inv = crate::items::inventory::Inventory::new(capacity);
        for _ in 0..count {
            let item_id = self.read_u8()?;
            let qty = self.read_u8()?;
            let item = pokered_data::items::ItemId::from_id(item_id);
            let _ = inv.add_item(item, qty);
        }
        let _terminator = self.read_u8()?;
        let items_read = count * 2 + 1;
        let total_slots = capacity * 2 + 1;
        if total_slots > items_read {
            self.skip(total_slots - items_read)?;
        }
        Ok(inv)
    }

    pub fn read_exp24(&mut self) -> Result<u32, SaveError> {
        let b0 = self.read_u8()? as u32;
        let b1 = self.read_u8()? as u32;
        let b2 = self.read_u8()? as u32;
        Ok((b0 << 16) | (b1 << 8) | b2)
    }
}
