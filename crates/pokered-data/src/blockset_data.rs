use crate::tilesets::TilesetId;

static OVERWORLD_BST: &[u8] = include_bytes!("../../../gfx/blocksets/overworld.bst");
static REDS_HOUSE_BST: &[u8] = include_bytes!("../../../gfx/blocksets/reds_house.bst");
static FOREST_BST: &[u8] = include_bytes!("../../../gfx/blocksets/forest.bst");
static GYM_BST: &[u8] = include_bytes!("../../../gfx/blocksets/gym.bst");
static POKECENTER_BST: &[u8] = include_bytes!("../../../gfx/blocksets/pokecenter.bst");
static HOUSE_BST: &[u8] = include_bytes!("../../../gfx/blocksets/house.bst");
static GATE_BST: &[u8] = include_bytes!("../../../gfx/blocksets/gate.bst");
static UNDERGROUND_BST: &[u8] = include_bytes!("../../../gfx/blocksets/underground.bst");
static SHIP_BST: &[u8] = include_bytes!("../../../gfx/blocksets/ship.bst");
static SHIP_PORT_BST: &[u8] = include_bytes!("../../../gfx/blocksets/ship_port.bst");
static CEMETERY_BST: &[u8] = include_bytes!("../../../gfx/blocksets/cemetery.bst");
static INTERIOR_BST: &[u8] = include_bytes!("../../../gfx/blocksets/interior.bst");
static CAVERN_BST: &[u8] = include_bytes!("../../../gfx/blocksets/cavern.bst");
static LOBBY_BST: &[u8] = include_bytes!("../../../gfx/blocksets/lobby.bst");
static MANSION_BST: &[u8] = include_bytes!("../../../gfx/blocksets/mansion.bst");
static LAB_BST: &[u8] = include_bytes!("../../../gfx/blocksets/lab.bst");
static CLUB_BST: &[u8] = include_bytes!("../../../gfx/blocksets/club.bst");
static FACILITY_BST: &[u8] = include_bytes!("../../../gfx/blocksets/facility.bst");
static PLATEAU_BST: &[u8] = include_bytes!("../../../gfx/blocksets/plateau.bst");

pub const BLOCK_SIZE: usize = 16;
pub const BLOCK_TILES_W: usize = 4;
pub const BLOCK_TILES_H: usize = 4;

pub fn blockset_for_tileset(tileset: TilesetId) -> &'static [u8] {
    match tileset {
        TilesetId::Overworld => OVERWORLD_BST,
        TilesetId::RedsHouse1 => REDS_HOUSE_BST,
        TilesetId::Mart => POKECENTER_BST,
        TilesetId::Forest => FOREST_BST,
        TilesetId::RedsHouse2 => REDS_HOUSE_BST,
        TilesetId::Dojo => GYM_BST,
        TilesetId::Pokecenter => POKECENTER_BST,
        TilesetId::Gym => GYM_BST,
        TilesetId::House => HOUSE_BST,
        TilesetId::ForestGate => GATE_BST,
        TilesetId::Museum => GATE_BST,
        TilesetId::Underground => UNDERGROUND_BST,
        TilesetId::Gate => GATE_BST,
        TilesetId::Ship => SHIP_BST,
        TilesetId::ShipPort => SHIP_PORT_BST,
        TilesetId::Cemetery => CEMETERY_BST,
        TilesetId::Interior => INTERIOR_BST,
        TilesetId::Cavern => CAVERN_BST,
        TilesetId::Lobby => LOBBY_BST,
        TilesetId::Mansion => MANSION_BST,
        TilesetId::Lab => LAB_BST,
        TilesetId::Club => CLUB_BST,
        TilesetId::Facility => FACILITY_BST,
        TilesetId::Plateau => PLATEAU_BST,
        // Custom tilesets carry their own .bst bytes embedded by build.rs.
        // Fall back to the base tileset's blockset if the slot is unregistered.
        TilesetId::Custom(slot) => match crate::tilesets::custom::blockset_for(slot) {
            Some(bytes) => bytes,
            None => blockset_for_tileset(tileset.base()),
        },
    }
}

pub fn block_tiles(tileset: TilesetId, block_id: u8) -> Option<&'static [u8; 16]> {
    let data = blockset_for_tileset(tileset);
    let offset = block_id as usize * BLOCK_SIZE;
    if offset + BLOCK_SIZE > data.len() {
        return None;
    }
    data[offset..offset + BLOCK_SIZE].try_into().ok()
}
