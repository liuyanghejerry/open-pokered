//! LoadCurrentMapView / DrawTileBlock over the native map's immutable tile
//! input. Each WRAM load/store/branch runs on the common instruction clock;
//! no captured frame total or original WRAM snapshot is used. The native
//! input already resolves map connections and live block replacements.
use crate::alloc_prelude::*;
use super::display_lcd_work::{DisplayAccess, DisplayInstruction, DisplayInstructionBus};
use super::far_pattern_copy_work::{LOADED_ROM_BANK, ROM_BANK_SELECT};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase { Setup(u8), Row(u8), Block(u8), Draw(u8), Tiles(u8), BlockEnd(u8), RowEnd(u8), Visible(u8), Copy(u8), CopyEnd(u8), Return(u8), Done }

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MapViewWork {
    phase: Phase,
    input: Vec<u8>,
    surrounding: Vec<u8>,
    visible: Vec<u8>,
    width: u8,
    x_half: bool,
    y_half: bool,
    bank: u8,
    saved_bank: u8,
    row: u8,
    column: u8,
    tile_row: u8,
    tile_column: u8,
    block_pointer: u16,
    row_pointer: u16,
    read_pointer: u16,
    a: u8,
    carry: bool,
}

impl MapViewWork {
    /// 24×20 surrounding tiles in row order, aligned to a 4×4 map block.
    /// Half-block offsets select the visible 20×18 window inside it.
    pub fn new(width: u8, origin: (i32, i32), bank: u8, tiles: Vec<u8>) -> Option<Self> {
        if tiles.len() != 24 * 20 || origin.0 % 2 != 0 || origin.1 % 2 != 0 { return None; }
        let block_pointer = 0xc6e8i32 + (origin.1.div_euclid(4) + 3) * (i32::from(width) + 6)
            + origin.0.div_euclid(4) + 3;
        Some(Self {
            phase: Phase::Setup(0), input: tiles, surrounding: vec![0; 480], visible: Vec::with_capacity(360),
            width, x_half: origin.0.rem_euclid(4) != 0, y_half: origin.1.rem_euclid(4) != 0,
            bank, saved_bank: 0, row: 0, column: 0, tile_row: 0, tile_column: 0,
            block_pointer: u16::try_from(block_pointer).ok()?, row_pointer: 0xc508, read_pointer: 0xc508,
            a: 0, carry: false,
        })
    }
    pub fn finished(&self) -> bool { self.phase == Phase::Done }
    pub fn visible_tiles(&self) -> Option<&[u8]> { self.finished().then_some(&self.visible) }

    pub fn step<B: DisplayInstructionBus>(&mut self, bus: &mut B) -> Result<bool, B::Error> {
        use Phase::*;
        let tcycles = match self.phase {
            Setup(n) => [24, 12, 16, 16, 12, 16, 16, 4, 16, 4, 12, 8][usize::from(n)],
            Row(n) => [16, 16, 8][usize::from(n)],
            Block(n) => [16, 16, 16, 8, 4, 24][usize::from(n)],
            Draw(n) => [16, 16, 4, 16, 4, 4, 8, 4, 8, 4, 4, 8, 4, 8, 4, 4, 12, 8][usize::from(n)],
            Tiles(n) => match n {
                0 => 16, 1..=3 => 8, 4 => 12, 5 => 8, 6 => 12, 7 => 4,
                8 => if self.tile_row < 4 { 12 } else { 8 }, 9 => 16, _ => unreachable!(),
            },
            BlockEnd(n) => match n { 0..=2 => 12, 3..=7 => 8, 8 => 4,
                9 => if self.column < 6 { 12 } else { 8 }, _ => unreachable!() },
            RowEnd(n) => match n {
                0 | 7 => 12, 1 => 16, 2 | 8 => 8,
                5 | 11 => if self.carry { 8 } else { 12 },
                14 => if self.row < 5 { 12 } else { 8 }, _ => 4,
            },
            Visible(n) => match n {
                0 | 1 | 5 | 10 | 12 => 12, 2 | 7 => 16, 6 => 8,
                4 => if self.y_half { 8 } else { 12 },
                9 => if self.x_half { 8 } else { 12 },
                11 | 13 => 8, _ => 4,
            },
            Copy(n) => match n { 0..=3 => 8, 4 => 4,
                5 => if self.column < 20 { 12 } else { 8 }, _ => unreachable!() },
            CopyEnd(n) => match n { 0 => 8, 3 => if self.carry { 8 } else { 12 },
                6 => if self.row < 18 { 12 } else { 8 }, _ => 4 },
            Return(n) => [12, 12, 16, 16][usize::from(n)],
            Done => return Ok(false),
        };
        let access = match self.phase {
            Setup(1) => Some(DisplayAccess::Read { port: LOADED_ROM_BANK, at_tcycle: 8 }),
            Setup(4) | Return(1) => Some(DisplayAccess::Write { port: LOADED_ROM_BANK,
                value: if matches!(self.phase, Setup(_)) { self.bank } else { self.saved_bank }, at_tcycle: 8 }),
            Setup(5) | Return(2) => Some(DisplayAccess::Write { port: ROM_BANK_SELECT,
                value: if matches!(self.phase, Setup(_)) { self.bank } else { self.saved_bank }, at_tcycle: 12 }),
            _ => None,
        };
        let read = bus.execute(DisplayInstruction { tcycles, access })?;
        self.phase = match self.phase {
            Setup(n) => {
                if n == 1 { self.saved_bank = read.expect("loaded bank read"); }
                if n == 11 { Row(0) } else { Setup(n + 1) }
            }
            Row(n) => if n == 2 { self.column = 0; Block(0) } else { Row(n + 1) },
            Block(n) => if n == 5 { Draw(0) } else { Block(n + 1) },
            Draw(n) => if n == 17 { self.tile_row = 0; self.tile_column = 0; Tiles(0) } else { Draw(n + 1) },
            Tiles(n) => {
                let at = (usize::from(self.row) * 4 + usize::from(self.tile_row)) * 24
                    + usize::from(self.column) * 4 + usize::from(self.tile_column);
                match n {
                    1 => { self.a = self.input[at]; Tiles(2) }
                    2 => { self.surrounding[at] = self.a; Tiles(3) }
                    3 => { self.tile_column += 1; if self.tile_column == 4 { Tiles(4) } else { Tiles(1) } }
                    7 => { self.tile_row += 1; Tiles(8) }
                    8 => if self.tile_row == 4 { Tiles(9) } else { self.tile_column = 0; Tiles(0) },
                    9 => BlockEnd(0),
                    _ => Tiles(n + 1),
                }
            }
            BlockEnd(n) => match n {
                8 => { self.column += 1; BlockEnd(9) }
                9 => if self.column == 6 { RowEnd(0) } else { Block(0) },
                _ => BlockEnd(n + 1),
            },
            RowEnd(n) => match n {
                3 => { self.carry = u16::from(self.block_pointer as u8) + u16::from(self.width.wrapping_add(6)) > 255; RowEnd(4) }
                4 => { self.block_pointer = self.block_pointer.wrapping_add(u16::from(self.width.wrapping_add(6))); RowEnd(5) }
                5 => RowEnd(if self.carry { 6 } else { 7 }),
                9 => { self.carry = u16::from(self.row_pointer as u8) + 96 > 255; RowEnd(10) }
                10 => { self.row_pointer = self.row_pointer.wrapping_add(96); RowEnd(11) }
                11 => RowEnd(if self.carry { 12 } else { 13 }),
                13 => { self.row += 1; RowEnd(14) }
                14 => if self.row == 5 { Visible(0) } else { Row(0) },
                _ => RowEnd(n + 1),
            },
            Visible(n) => match n {
                0 => { self.read_pointer = 0xc508; Visible(1) }
                4 => Visible(if self.y_half { 5 } else { 7 }),
                6 => { self.read_pointer += 48; Visible(7) }
                9 => Visible(if self.x_half { 10 } else { 12 }),
                11 => { self.read_pointer += 2; Visible(12) }
                13 => { self.row = 0; self.column = 0; Copy(0) }
                _ => Visible(n + 1),
            },
            Copy(n) => match n {
                1 => { self.a = self.surrounding[usize::from(self.read_pointer - 0xc508)]; self.read_pointer += 1; Copy(2) }
                2 => { self.visible.push(self.a); Copy(3) }
                4 => { self.column += 1; Copy(5) }
                5 => if self.column == 20 { CopyEnd(0) } else { Copy(1) },
                _ => Copy(n + 1),
            },
            CopyEnd(n) => match n {
                1 => { self.carry = u16::from(self.read_pointer as u8) + 4 > 255; CopyEnd(2) }
                2 => { self.read_pointer += 4; CopyEnd(3) }
                3 => CopyEnd(if self.carry { 4 } else { 5 }),
                5 => { self.row += 1; CopyEnd(6) }
                6 => if self.row == 18 { Return(0) } else { self.column = 0; Copy(0) },
                _ => CopyEnd(n + 1),
            },
            Return(n) => if n == 3 { Done } else { Return(n + 1) },
            Done => unreachable!(),
        };
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Bus { cycles: u32, bank: u8, selected: u8, grant: usize }
    impl DisplayInstructionBus for Bus {
        type Error = ();
        fn execute(&mut self, i: DisplayInstruction) -> Result<Option<u8>, ()> {
            if self.grant == 0 { return Err(()); }
            self.grant -= 1;
            self.cycles += u32::from(i.tcycles);
            Ok(match i.access {
                Some(DisplayAccess::Read { port: LOADED_ROM_BANK, .. }) => Some(self.bank),
                Some(DisplayAccess::Write { port: LOADED_ROM_BANK, value, .. }) => { self.bank = value; None }
                Some(DisplayAccess::Write { port: ROM_BANK_SELECT, value, .. }) => { self.selected = value; None }
                Some(_) => panic!("unexpected map-view IO"),
                None => None,
            })
        }
    }
    #[test]
    fn map_rebuild_matches_original_bytes_and_instruction_duration_with_mid_child_yields() {
        let golden: serde_json::Value = serde_json::from_str(include_str!("map_view_reference_1390.json")).unwrap();
        let case = &golden["case"];
        let input: Vec<u8> = serde_json::from_value(case["tiles"].clone()).unwrap();
        let output: Vec<u8> = serde_json::from_value(case["visible"].clone()).unwrap();
        for grant in [usize::MAX, 503] {
            let mut work = MapViewWork::new(case["width"].as_u64().unwrap() as u8,
                (case["x"].as_i64().unwrap() as i32 * 2 - 8, case["y"].as_i64().unwrap() as i32 * 2 - 8),
                case["bank"].as_u64().unwrap() as u8, input.clone()).unwrap();
            assert_eq!(u64::from(work.block_pointer), case["pointer"].as_u64().unwrap());
            let bank = case["saved_bank"].as_u64().unwrap() as u8;
            let mut bus = Bus { cycles: 0, bank, selected: bank, grant };
            loop {
                match work.step(&mut bus) {
                    Ok(true) => (),
                    Ok(false) => break,
                    Err(()) => {
                        let before = work.clone();
                        assert_eq!(work.step(&mut bus), Err(()));
                        assert_eq!(work, before);
                        work = serde_json::from_str(&serde_json::to_string(&work).unwrap()).unwrap();
                        bus.grant = grant;
                    }
                }
            }
            assert_eq!(work.visible_tiles().unwrap(), output);
            assert_eq!(u64::from(bus.cycles), case["cycles"].as_u64().unwrap());
            assert_eq!(bus.bank, bank);
            assert_eq!(bus.selected, bank);
        }
    }
    #[test]
    fn map_view_selects_each_half_block_without_touching_the_input_or_skipping_work() {
        let input: Vec<u8> = (0..480).map(|n| (n % 251) as u8).collect();
        for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
            let mut work = MapViewWork::new(20, (x, y), 25, input.clone()).unwrap();
            let mut bus = Bus { cycles: 0, bank: 9, selected: 9, grant: usize::MAX };
            while work.step(&mut bus).unwrap() {}
            let expected: Vec<_> = (0..18).flat_map(|row| input[(row+y as usize)*24+x as usize..(row+y as usize)*24+x as usize+20].iter().copied()).collect();
            assert_eq!(work.visible_tiles().unwrap(), expected);
            assert_eq!(work.input, input);
            assert!(bus.cycles > 45000);
            assert_eq!(bus.bank, 9);
        }
    }
}
