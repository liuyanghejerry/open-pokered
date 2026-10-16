//! One foreground Joypad history, distinct from the VBlank/raw input sample.
//!
//! The byte uses the original logical order A/B/SELECT/START/RIGHT/LEFT/UP/DOWN.
//! It is deliberately not a byte copy of the overworld's FieldInputSample.

use dotzuki_engine::overworld::OverworldInput;
use serde::{Deserialize, Serialize};

pub const A: u8 = 1;
pub const B: u8 = 1 << 1;
pub const SELECT: u8 = 1 << 2;
pub const START: u8 = 1 << 3;
pub const RIGHT: u8 = 1 << 4;
pub const LEFT: u8 = 1 << 5;
pub const UP: u8 = 1 << 6;
pub const DOWN: u8 = 1 << 7;

/// Lifetime of the one serialized foreground ledger. Observed is a migration
/// boundary: its real field polls are recorded, but an unmodeled screen may
/// still use its old input API. Field is the real Bike/text/menu continuation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForegroundMode {
    #[default]
    Observed,
    Field,
}

/// Unknown is a real migration/snapshot boundary, never neutral input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForegroundInputError {
    UnknownBeforePrompt,
    UnknownReturningField,
    MissingFieldOwner,
}

/// A receipt of a real foreground poll, including keys the caller may ignore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForegroundPoll {
    pub held: u8,
    /// Unknown preceding history is not a neutral preceding sample.
    pub pressed: Option<u8>,
    pub released: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForegroundJoypad {
    raw: u8,
    last: Option<u8>,
    pressed: Option<u8>,
    released: Option<u8>,
}

impl ForegroundJoypad {
    /// ReadJoypad/VBlank refreshes raw; it does not execute Joypad.
    pub fn read_raw(&mut self, held: u8) {
        self.raw = held;
    }

    /// Call only at an actual foreground Joypad instruction. A held-only
    /// PrintLetterDelay poll can establish unknown history without an edge.
    pub fn poll(&mut self) -> ForegroundPoll {
        self.pressed = self.last.map(|last| self.raw & !last);
        self.released = self.last.map(|last| last & !self.raw);
        self.last = Some(self.raw);
        ForegroundPoll {
            held: self.raw,
            pressed: self.pressed,
            released: self.released,
        }
    }

    /// Entry to an unmodeled foreground owner loses knowledge, not raw input.
    /// A later held-only real poll must establish history before an edge owner.
    pub fn retire_history(&mut self) {
        self.last = None;
        self.pressed = None;
        self.released = None;
    }

    pub fn raw(&self) -> u8 {
        self.raw
    }
    pub fn last(&self) -> Option<u8> {
        self.last
    }
    pub fn pressed(&self) -> Option<u8> {
        self.pressed
    }
    pub fn released(&self) -> Option<u8> {
        self.released
    }

    /// Explicit conversion; the existing field sampler uses another bit order.
    pub fn field_buttons(input: OverworldInput) -> u8 {
        u8::from(input.a)
            | (u8::from(input.b) << 1)
            | (u8::from(input.select) << 2)
            | (u8::from(input.start) << 3)
            | (u8::from(input.right) << 4)
            | (u8::from(input.left) << 5)
            | (u8::from(input.up) << 6)
            | (u8::from(input.down) << 7)
    }

    pub fn as_field_input(buttons: u8) -> OverworldInput {
        OverworldInput::new(
            buttons & UP != 0,
            buttons & DOWN != 0,
            buttons & LEFT != 0,
            buttons & RIGHT != 0,
            buttons & A != 0,
            buttons & B != 0,
            buttons & START != 0,
            buttons & SELECT != 0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_waits_do_not_poll_and_all_eight_keys_round_trip() {
        for buttons in 0..=u8::MAX {
            assert_eq!(
                ForegroundJoypad::field_buttons(ForegroundJoypad::as_field_input(buttons)),
                buttons
            );
            let mut owner = ForegroundJoypad::default();
            owner.read_raw(buttons);
            assert_eq!(owner.last(), None);
            assert_eq!(
                owner.poll(),
                ForegroundPoll {
                    held: buttons,
                    pressed: None,
                    released: None,
                }
            );
            owner.read_raw(!buttons);
            assert_eq!(owner.last(), Some(buttons));
            let restored: ForegroundJoypad =
                serde_json::from_str(&serde_json::to_string(&owner).unwrap()).unwrap();
            assert_eq!(restored, owner);
            assert_eq!(
                owner.poll(),
                ForegroundPoll {
                    held: !buttons,
                    pressed: Some(!buttons),
                    released: Some(buttons),
                }
            );
        }
    }

    #[test]
    fn actual_overworld_snapshot_keeps_one_owner_and_legacy_absence() {
        use crate::overworld::OverworldScreen;
        use crate::snapshot::OverworldSnapshot;
        use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
        let mut overworld = OverworldScreen::new(MapId::CinnabarGym, None, PokemonRedData);
        let owner = overworld.begin_source_field();
        owner.read_raw(A | SELECT | LEFT | DOWN);
        owner.poll();
        owner.read_raw(B | START | RIGHT | UP);
        let expected = owner.clone();
        let json = serde_json::to_value(OverworldSnapshot::capture(&overworld)).unwrap();
        let snapshot: OverworldSnapshot = serde_json::from_value(json.clone()).unwrap();
        overworld.source_foreground = None;
        snapshot.restore_into(&mut overworld);
        assert_eq!(overworld.source_foreground_mut().unwrap().clone(), expected);
        let mut legacy = json;
        legacy.as_object_mut().unwrap().remove("source_foreground");
        let legacy: OverworldSnapshot = serde_json::from_value(legacy).unwrap();
        legacy.restore_into(&mut overworld);
        assert!(overworld.source_foreground_mut().is_none());
    }
}
