use crate::maps::MapId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorFloor {
    B4F = 0x61,
    B2F = 0x54,
    B1F = 0x55,
    F1 = 0x56,
    F2 = 0x57,
    F3 = 0x58,
    F4 = 0x59,
    F5 = 0x5A,
    F6 = 0x5B,
    F7 = 0x5C,
    F8 = 0x5D,
    F9 = 0x5E,
    F10 = 0x5F,
    F11 = 0x60,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElevatorWarpEntry {
    pub warp_id: u8,
    pub map_id: MapId,
}

#[derive(Debug, Clone)]
pub struct ElevatorData {
    pub floors: &'static [ElevatorFloor],
    pub warp_maps: &'static [ElevatorWarpEntry],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevatorId {
    RocketHideout,
    CeladonMart,
    SilphCo,
}

pub fn elevator_data(id: ElevatorId) -> ElevatorData {
    match id {
        ElevatorId::RocketHideout => ElevatorData {
            floors: &[ElevatorFloor::B1F, ElevatorFloor::B2F, ElevatorFloor::B4F],
            warp_maps: &[
                ElevatorWarpEntry {
                    warp_id: 4,
                    map_id: MapId::RocketHideoutB1F,
                },
                ElevatorWarpEntry {
                    warp_id: 4,
                    map_id: MapId::RocketHideoutB2F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::RocketHideoutB4F,
                },
            ],
        },
        ElevatorId::CeladonMart => ElevatorData {
            floors: &[
                ElevatorFloor::F1,
                ElevatorFloor::F2,
                ElevatorFloor::F3,
                ElevatorFloor::F4,
                ElevatorFloor::F5,
            ],
            warp_maps: &[
                ElevatorWarpEntry {
                    warp_id: 5,
                    map_id: MapId::CeladonMart1F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::CeladonMart2F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::CeladonMart3F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::CeladonMart4F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::CeladonMart5F,
                },
            ],
        },
        ElevatorId::SilphCo => ElevatorData {
            floors: &[
                ElevatorFloor::F1,
                ElevatorFloor::F2,
                ElevatorFloor::F3,
                ElevatorFloor::F4,
                ElevatorFloor::F5,
                ElevatorFloor::F6,
                ElevatorFloor::F7,
                ElevatorFloor::F8,
                ElevatorFloor::F9,
                ElevatorFloor::F10,
                ElevatorFloor::F11,
            ],
            warp_maps: &[
                ElevatorWarpEntry {
                    warp_id: 3,
                    map_id: MapId::SilphCo1F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo2F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo3F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo4F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo5F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo6F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo7F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo8F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo9F,
                },
                ElevatorWarpEntry {
                    warp_id: 2,
                    map_id: MapId::SilphCo10F,
                },
                ElevatorWarpEntry {
                    warp_id: 1,
                    map_id: MapId::SilphCo11F,
                },
            ],
        },
    }
}

pub fn elevator_for_map(map_id: MapId) -> Option<ElevatorId> {
    match map_id {
        MapId::RocketHideoutElevator => Some(ElevatorId::RocketHideout),
        MapId::CeladonMartElevator => Some(ElevatorId::CeladonMart),
        MapId::SilphCoElevator => Some(ElevatorId::SilphCo),
        _ => None,
    }
}
