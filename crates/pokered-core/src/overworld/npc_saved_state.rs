//! Game-owned sprite bytes, map lifecycle, and native/source coordinate bridge.
//! These are functional state projections, not instruction timing evidence.
use super::{npc_movement::NpcRuntimeState, Direction, NpcMovementType};
use crate::save::sprite_data::SpriteData;
use pokered_data::maps::MapId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NpcSavedState {
    map: Option<MapId>,
    pub data: SpriteData,
}

fn facing_byte(facing: Direction) -> u8 {
    match facing {
        Direction::Down => 0,
        Direction::Up => 4,
        Direction::Left => 8,
        Direction::Right => 12,
    }
}
fn facing_direction(value: u8) -> Direction {
    match value & 12 {
        4 => Direction::Up,
        8 => Direction::Left,
        12 => Direction::Right,
        _ => Direction::Down,
    }
}

impl NpcSavedState {
    pub fn restore_from_save(
        &mut self,
        map: MapId,
        save: &crate::save::SaveData,
        npcs: &mut [NpcRuntimeState],
    ) {
        if save.game_data.position.map_id != map as u8 {
            // An explicit editor/startup map override loads that map's actors,
            // not the saved map's sprite slots at unrelated coordinates.
            self.prepare(map, npcs);
            return;
        }
        self.restore(map, &save.sprite_data, npcs);
        for (index, (npc, &(selector, text_id))) in npcs
            .iter_mut()
            .zip(&save.game_data.map_sprite_data)
            .take(usize::from(save.game_data.num_sprites).min(15))
            .enumerate()
        {
            // Older native files can carry stale map-header/object metadata
            // while their entire sprite table is zero. That metadata has no
            // authority to replace the freshly loaded map's object defaults.
            if save.sprite_data.first[index + 1][0] == 0 {
                continue;
            }
            npc.range = selector;
            npc.wander_axis = super::npc_movement::convert_wander_axis(selector);
            npc.text_id = text_id;
        }
    }

    pub fn prepare(&mut self, map: MapId, npcs: &[NpcRuntimeState]) {
        if self.map != Some(map) {
            self.map = Some(map);
            self.data = SpriteData::default();
        }
        for (index, npc) in npcs.iter().take(15).enumerate() {
            let slot = index + 1;
            if self.data.first[slot][0] == 0 {
                self.data.first[slot][0] = npc.sprite_id;
                self.data.second[slot][2] = 8;
                self.data.second[slot][3] = 8;
                self.data.second[slot][6] = if npc.movement_type == NpcMovementType::Wander {
                    0xfe
                } else {
                    0xff
                };
            }
        }
    }

    /// Decode a normal CONTINUE. All-zero older native saves retain map
    /// defaults. Visibility continues to come from the saved object flags:
    /// ImageIndex=$ff can merely mean that an otherwise live actor is offscreen.
    pub fn restore(&mut self, map: MapId, saved: &SpriteData, npcs: &mut [NpcRuntimeState]) {
        self.map = Some(map);
        self.data = saved.clone();
        for (index, npc) in npcs.iter_mut().take(15).enumerate() {
            let a = &saved.first[index + 1];
            let b = &saved.second[index + 1];
            if a[0] == 0 {
                continue;
            }
            if a[1] & 3 == 0 {
                // InitializeSpriteStatus precedes CheckSpriteAvailability.
                // Zero counters in an uninitialized source slot are not an
                // exhausted walker's counters, even when its picture is set.
                self.data.second[index + 1][2] = 8;
                self.data.second[index + 1][3] = 8;
            }
            npc.sprite_id = a[0];
            npc.facing = facing_direction(a[9]);
            let direction = match (a[5] as i8, a[3] as i8) {
                (-1, 0) => Some(Direction::Left),
                (1, 0) => Some(Direction::Right),
                (0, -1) => Some(Direction::Up),
                (0, 1) => Some(Direction::Down),
                _ => None,
            };
            let walking = a[1] & 3 == 3 && (1..=16).contains(&b[0]) && direction.is_some();
            let (dx, dy) = if walking {
                dotzuki_engine::overworld::player_movement::direction_delta(direction.unwrap())
            } else {
                (0, 0)
            };
            // The original commits map coordinates at stride start; the
            // generic engine retains the origin until its counter reaches zero.
            npc.x = (i32::from(b[5].wrapping_sub(4)) - i32::from(dx)).max(0) as u16;
            npc.y = (i32::from(b[4].wrapping_sub(4)) - i32::from(dy)).max(0) as u16;
            npc.walk_counter = if walking { b[0] } else { 0 };
            npc.step_direction = if walking { direction } else { None };
            npc.delay_counter = if walking || a[1] & 3 == 2 {
                if b[8] == 0 {
                    256
                } else {
                    u16::from(b[8])
                }
            } else {
                0
            };
            if b[6] == 0xfe {
                npc.movement_type = NpcMovementType::Wander;
            } else if b[6] == 0xff && npc.movement_type == NpcMovementType::Wander {
                npc.movement_type = NpcMovementType::Stationary;
            }
        }
        self.prepare(map, npcs);
    }

    pub fn capture(&self, map: MapId, npcs: &[NpcRuntimeState], player: (u16, u16)) -> SpriteData {
        let mut state = self.clone();
        state.prepare(map, npcs);
        for (index, npc) in npcs.iter().take(15).enumerate() {
            let slot = index + 1;
            let a = &mut state.data.first[slot];
            let b = &mut state.data.second[slot];
            let walking = npc.walk_counter != 0;
            let (dx, dy) = if walking {
                dotzuki_engine::overworld::player_movement::direction_delta(npc.walking_direction())
            } else {
                (0, 0)
            };
            a[0] = npc.sprite_id;
            a[1] = if walking {
                3
            } else if npc.delay_counter != 0 {
                2
            } else {
                1
            };
            a[3] = dy as u8;
            a[5] = dx as u8;
            let progress = if walking {
                16u8.saturating_sub(npc.walk_counter)
            } else {
                0
            };
            let screen = |origin: u16, player: u16, delta: i8| {
                (origin as u8)
                    .wrapping_add(4)
                    .wrapping_sub(player as u8)
                    .rotate_left(4)
                    .wrapping_add((i16::from(delta) * i16::from(progress)) as u8)
            };
            a[4] = screen(npc.y, player.1, dy).wrapping_sub(4);
            a[6] = screen(npc.x, player.0, dx);
            a[7] = progress & 3;
            a[8] = (progress / 4) & 3;
            a[9] = facing_byte(npc.facing);
            b[0] = npc.walk_counter;
            b[4] = (npc.y as u8).wrapping_add(4).wrapping_add(dy as u8);
            b[5] = (npc.x as u8).wrapping_add(4).wrapping_add(dx as u8);
            b[6] = match npc.movement_type {
                NpcMovementType::Wander => 0xfe,
                NpcMovementType::FixedPath if !npc.scripted_path.is_empty() => b[6],
                _ => 0xff,
            };
            b[8] = npc.delay_counter as u8;
            b[9] = a[9];
        }
        state.data
    }
}

#[cfg(test)]
mod tests;
