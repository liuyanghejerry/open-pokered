//! Trait implementations bridging `pokered-data` types with `engine-core` traits.
//!
//! This module is the single point where concrete pokered data types implement
//! the generic provider traits defined in `engine-core`. Keeping all trait impls
//! here avoids scattering `use jrpg_engine::…` imports across the data crate
//! and makes it easy to see which providers are available at a glance.

use crate::blockset_data::{self, BLOCK_SIZE, BLOCK_TILES_H, BLOCK_TILES_W};
use crate::collision;
use crate::map_data_loader;
use crate::maps::MapId;
use crate::tileset_data;
use crate::tilesets::{TilesetId, NUM_BUILTIN_TILESETS};
use jrpg_engine::map::{MapConnection, MapProvider, MapTrait};
use jrpg_engine::overworld::map_transitions::MapTransitionProvider;
use jrpg_engine::tile_meta::{CollisionType, TileMetadata, TileMetaTrait};
use jrpg_engine::tileset::TilesetProvider;
use jrpg_engine::GameData;

/// Provider of tileset data backed by the built-in Pokemon Red/Blue tilesets.
pub struct PokemonTilesetData;

impl TilesetProvider<TilesetId> for PokemonTilesetData {
    fn tileset_count(&self) -> usize {
        NUM_BUILTIN_TILESETS
    }

    fn tileset_by_id(&self, id: u8) -> Option<TilesetId> {
        TilesetId::from_u8(id)
    }

    fn tileset_by_name(&self, name: &str) -> Option<TilesetId> {
        TilesetId::from_name(name)
    }

    fn blockset_for(&self, tileset: TilesetId) -> &[u8] {
        blockset_data::blockset_for_tileset(tileset)
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn tiles_per_block(&self) -> (usize, usize) {
        (BLOCK_TILES_W, BLOCK_TILES_H)
    }
}

/// Marker trait: MapId already satisfies Copy + Eq + Hash + Debug + 'static.
impl MapTrait for MapId {}

/// Provider that supplies map data to the engine via MapProvider.
pub struct PokemonMapData;

impl MapProvider<MapId> for PokemonMapData {
    fn dimensions(&self, map: MapId) -> (u8, u8) {
        map.dimensions()
    }

    fn tileset(&self, map: MapId) -> u8 {
        map_data_loader::get_map_json(map)
            .and_then(|j| TilesetId::from_name(&j.header.tileset))
            .map(|t| t.to_u8())
            .unwrap_or(0)
    }

    fn block_data(&self, map: MapId) -> &[u8] {
        map_data_loader::get_block_data(map)
    }

    fn border_block(&self, map: MapId) -> u8 {
        map_data_loader::get_map_json(map)
            .map(|j| j.header.border_block)
            .unwrap_or(0)
    }

    fn connections(&self, map: MapId) -> Vec<MapConnection<MapId>> {
        let json = match map_data_loader::get_map_json(map) {
            Some(j) => j,
            None => return Vec::new(),
        };
        let c = &json.connections;
        let mut conns = Vec::with_capacity(4);
        if let Some(ref entry) = c.north {
            if let Some(target) = map_data_loader::resolve_map_id(&entry.target_map) {
                conns.push(MapConnection {
                    direction: "north".to_string(),
                    map: target,
                    offset: entry.offset,
                });
            }
        }
        if let Some(ref entry) = c.south {
            if let Some(target) = map_data_loader::resolve_map_id(&entry.target_map) {
                conns.push(MapConnection {
                    direction: "south".to_string(),
                    map: target,
                    offset: entry.offset,
                });
            }
        }
        if let Some(ref entry) = c.west {
            if let Some(target) = map_data_loader::resolve_map_id(&entry.target_map) {
                conns.push(MapConnection {
                    direction: "west".to_string(),
                    map: target,
                    offset: entry.offset,
                });
            }
        }
        if let Some(ref entry) = c.east {
            if let Some(target) = map_data_loader::resolve_map_id(&entry.target_map) {
                conns.push(MapConnection {
                    direction: "east".to_string(),
                    map: target,
                    offset: entry.offset,
                });
            }
        }
        conns
    }
}

impl MapTransitionProvider<MapId> for PokemonMapData {
    fn resolve_map_id(&self, name: &str) -> Option<MapId> {
        map_data_loader::resolve_map_id(name)
    }

    fn get_map_dimensions(&self, map: MapId) -> (u8, u8) {
        map.dimensions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> PokemonTilesetData {
        PokemonTilesetData
    }

    #[test]
    fn tileset_count_matches_expected() {
        assert_eq!(provider().tileset_count(), 24);
    }

    #[test]
    fn tileset_by_id_0_returns_overworld() {
        assert_eq!(provider().tileset_by_id(0), Some(TilesetId::Overworld));
    }

    #[test]
    fn tileset_by_id_23_returns_plateau() {
        assert_eq!(provider().tileset_by_id(23), Some(TilesetId::Plateau));
    }

    #[test]
    fn tileset_by_id_255_returns_none() {
        assert_eq!(provider().tileset_by_id(255), None);
    }

    #[test]
    fn tileset_by_name_overworld_matches() {
        assert_eq!(provider().tileset_by_name("Overworld"), Some(TilesetId::Overworld));
    }

    #[test]
    fn tileset_by_name_not_found_returns_none() {
        assert_eq!(provider().tileset_by_name("NotFound"), None);
    }

    #[test]
    fn blockset_for_overworld_is_non_empty() {
        let p = provider();
        let blocks = p.blockset_for(TilesetId::Overworld);
        assert!(!blocks.is_empty());
        assert_eq!(blocks.len() % BLOCK_SIZE, 0);
    }

    #[test]
    fn block_size_constant() {
        assert_eq!(provider().block_size(), 16);
    }

    #[test]
    fn tiles_per_block_dims() {
        assert_eq!(provider().tiles_per_block(), (4, 4));
    }

    #[test]
    fn all_builtin_ids_resolve_by_variant_name() {
        let p = provider();
        for id in 0..=23u8 {
            let by_id = p.tileset_by_id(id).unwrap();
            let variant = by_id.variant_name();
            let by_name = p.tileset_by_name(variant);
            assert_eq!(by_name, Some(by_id),
                "variant_name {:?} did not round-trip for id {}", variant, id);
        }
    }

    fn map_provider() -> PokemonMapData {
        PokemonMapData
    }

    #[test]
    fn map_trait_is_implemented() {
        fn _assert_trait(m: MapId) -> impl MapTrait { m }
        let _ = _assert_trait(MapId::PalletTown);
    }

    #[test]
    fn map_trait_satisfies_bounds() {
        let a = MapId::Route1;
        let _b = a;
        let _c = a;
        assert_eq!(MapId::Route1, MapId::Route1);
        assert_ne!(MapId::Route1, MapId::Route2);
    }

    #[test]
    fn provider_dimensions() {
        let p = map_provider();
        assert_eq!(p.dimensions(MapId::PalletTown), (10, 9));
        assert_eq!(p.dimensions(MapId::Route1), (10, 18));
        assert_eq!(p.dimensions(MapId::OaksLab), (5, 6));
    }

    #[test]
    fn provider_tileset() {
        let p = map_provider();
        // PalletTown uses Overworld tileset (0)
        assert_eq!(p.tileset(MapId::PalletTown), 0);
        // OaksLab uses the Dojo tileset per its JSON header
        assert_eq!(p.tileset(MapId::OaksLab), TilesetId::Dojo.to_u8());
    }

    #[test]
    fn provider_border_block() {
        assert!(map_provider().border_block(MapId::PalletTown) > 0);
    }

    #[test]
    fn provider_block_data() {
        let p = map_provider();
        let blocks = p.block_data(MapId::PalletTown);
        assert!(!blocks.is_empty());
        assert_eq!(blocks.len(), 90);
    }

    #[test]
    fn provider_connections() {
        let conns = map_provider().connections(MapId::PalletTown);
        assert!(!conns.is_empty());
        assert!(conns.iter().any(|c| c.map == MapId::Route1));
    }
}

// ============================================================================
// PaletteTrait and PaletteProvider implementations
// ============================================================================

use crate::sgb_palettes::{self, SgbPaletteId};
use jrpg_engine::palette::{PaletteProvider, PaletteTrait, SgbPaletteEntry};

// PaletteTrait is already implemented for SgbPaletteId in jrpg-engine.

/// Palette data provider for Pokemon Red/Blue.
pub struct PokemonPaletteData {
    /// Whether this is Pokemon Red (true) or Blue (false).
    pub is_red: bool,
}

impl PokemonPaletteData {
    pub const fn new(is_red: bool) -> Self {
        Self { is_red }
    }
}

impl PaletteProvider<SgbPaletteId> for PokemonPaletteData {
    fn bg_palette(&self, _palette: SgbPaletteId) -> [u8; 4] {
        [0, 1, 2, 3]
    }

    fn obj_palette0(&self, _palette: SgbPaletteId) -> [u8; 4] {
        [0, 1, 2, 3]
    }

    fn obj_palette1(&self, _palette: SgbPaletteId) -> [u8; 4] {
        [0, 1, 2, 3]
    }

    fn overworld_palette_for(&self, tileset_id: u8, map_id: u8, last_map: u8) -> SgbPaletteId {
        sgb_palettes::overworld_palette_for_map(tileset_id, map_id, last_map)
    }

    fn monster_palette(&self, species_index: u8) -> SgbPaletteId {
        sgb_palettes::monster_palette(species_index)
    }

    fn sgb_palette_data(&self, id: SgbPaletteId, _is_red: bool) -> SgbPaletteEntry {
        *sgb_palettes::lookup_sgb_palette(id, self.is_red)
    }

    fn hp_bar_to_palette_id(&self, hp_bar_color: u8) -> SgbPaletteId {
        sgb_palettes::hp_bar_to_sgb_palette(hp_bar_color)
    }
}

// ============================================================================
// TileMetaTrait and TileMetadata implementations
// ============================================================================

impl TileMetaTrait for TilesetId {}

/// Provides tile collision and terrain metadata for Pokémon Red/Blue
/// tilesets.
///
/// All lookups use the static data tables in [`crate::collision`] and
/// [`crate::tileset_data`].
pub struct PokemonTileMetadata;

impl TileMetadata<TilesetId> for PokemonTileMetadata {
    fn is_passable(&self, tileset: TilesetId, tile_id: u8) -> bool {
        collision::is_tile_passable(tileset, tile_id)
    }

    fn collision_type(&self, tileset: TilesetId, tile_id: u8) -> CollisionType {
        // A tile can be both a ledge and passable; check ledge first.
        if self.is_ledge(tileset, tile_id) {
            let direction = collision::LEDGE_TILES
                .iter()
                .find(|l| l.ledge_tile == tile_id)
                .map(|l| l.direction)
                .unwrap_or(0);
            return CollisionType::Ledge { direction };
        }
        if self.is_counter(tileset, tile_id) {
            return CollisionType::Counter;
        }
        if self.is_grass(tileset, tile_id) {
            return CollisionType::Grass(tileset_data::get_grass_tile(tileset));
        }
        if self.is_passable(tileset, tile_id) {
            return CollisionType::Passable;
        }
        CollisionType::Impassable
    }

    fn is_ledge(&self, _tileset: TilesetId, tile_id: u8) -> bool {
        collision::LEDGE_TILES
            .iter()
            .any(|l| l.ledge_tile == tile_id)
    }

    fn is_counter(&self, tileset: TilesetId, tile_id: u8) -> bool {
        tileset_data::get_tileset_header(tileset).is_counter_tile(tile_id)
    }

    fn is_grass(&self, tileset: TilesetId, tile_id: u8) -> bool {
        tileset_data::get_tileset_header(tileset).is_grass_tile(tile_id)
    }

    fn get_grass_tile(&self, tileset: TilesetId) -> Option<u8> {
        tileset_data::get_grass_tile(tileset)
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn test_sgb_palette_id_is_palette_trait() {
        fn assert_palette_trait<T: PaletteTrait>() {}
        assert_palette_trait::<SgbPaletteId>();
    }

    #[test]
    fn test_bg_palette_identity() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.bg_palette(SgbPaletteId::Route), [0, 1, 2, 3]);
    }

    #[test]
    fn test_obj_palette0_identity() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.obj_palette0(SgbPaletteId::RedMon), [0, 1, 2, 3]);
    }

    #[test]
    fn test_obj_palette1_identity() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.obj_palette1(SgbPaletteId::GreenMon), [0, 1, 2, 3]);
    }

    #[test]
    fn test_overworld_palette_outdoor_route() {
        let provider = PokemonPaletteData::new(true);
        let pal = provider.overworld_palette_for(0, 0x0B, 0);
        assert_eq!(pal, SgbPaletteId::Route);
    }

    #[test]
    fn test_overworld_palette_town() {
        let provider = PokemonPaletteData::new(true);
        // Pallet Town (map_id=0x00) → (0+1)=1 → PAL_PALLET
        let pal = provider.overworld_palette_for(0, 0x00, 0);
        assert_eq!(pal, SgbPaletteId::Pallet);
    }

    #[test]
    fn test_overworld_palette_cemetery() {
        let provider = PokemonPaletteData::new(true);
        let pal = provider.overworld_palette_for(15, 0, 0);
        assert_eq!(pal, SgbPaletteId::GrayMon);
    }

    #[test]
    fn test_overworld_palette_cavern() {
        let provider = PokemonPaletteData::new(true);
        let pal = provider.overworld_palette_for(17, 0, 0);
        assert_eq!(pal, SgbPaletteId::Cave);
    }

    #[test]
    fn test_overworld_palette_lorelei() {
        let provider = PokemonPaletteData::new(true);
        let pal = provider.overworld_palette_for(0, 0xF5, 0);
        assert_eq!(pal, SgbPaletteId::Pallet);
    }

    #[test]
    fn test_overworld_palette_bruno() {
        let provider = PokemonPaletteData::new(true);
        let pal = provider.overworld_palette_for(0, 0xF6, 0);
        assert_eq!(pal, SgbPaletteId::Cave);
    }

    #[test]
    fn test_monster_palette_bulbasaur() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.monster_palette(1), SgbPaletteId::GreenMon);
    }

    #[test]
    fn test_monster_palette_charmander() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.monster_palette(4), SgbPaletteId::RedMon);
    }

    #[test]
    fn test_monster_palette_squirtle() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.monster_palette(7), SgbPaletteId::CyanMon);
    }

    #[test]
    fn test_monster_palette_out_of_bounds() {
        let provider = PokemonPaletteData::new(true);
        assert_eq!(provider.monster_palette(255), SgbPaletteId::MewMon);
    }

    #[test]
    fn test_overworld_palette_blue_version() {
        let provider = PokemonPaletteData::new(false);
        let pal = provider.overworld_palette_for(0, 0x0B, 0);
        assert_eq!(pal, SgbPaletteId::Route);
    }
}

// ============================================================================
// RenderData implementation
// ============================================================================

use crate::item_data;
use crate::items::ItemId;
use crate::lang_data;
use crate::move_data;
use crate::moves::MoveId;
use crate::species::Species;
use jrpg_engine::render_data::RenderData;

pub struct PokemonRenderData {
    pub is_zh: bool,
}

impl PokemonRenderData {
    pub fn new(is_zh: bool) -> Self {
        Self { is_zh }
    }
}

impl RenderData for PokemonRenderData {
    type Move = MoveId;
    type Item = ItemId;
    type Species = Species;

    fn move_name(&self, m: Self::Move) -> &str {
        lang_data::move_name(m, self.is_zh)
    }

    fn move_pp(&self, m: Self::Move) -> (u8, u8) {
        move_data::MoveData::get(m)
            .map(|data| (data.pp, data.pp))
            .unwrap_or((0, 0))
    }

    fn move_type(&self, m: Self::Move) -> u8 {
        move_data::MoveData::get(m)
            .map(|data| data.move_type as u8)
            .unwrap_or(0)
    }

    fn item_name(&self, i: Self::Item) -> &str {
        item_data::get_item_data(i)
            .map(|data| data.name)
            .unwrap_or("???")
    }

    fn species_name(&self, s: Self::Species) -> &str {
        lang_data::species_name(s, self.is_zh)
    }
}

#[cfg(test)]
mod render_data_tests {
    use super::*;

    fn render_data_en() -> PokemonRenderData {
        PokemonRenderData::new(false)
    }

    fn render_data_zh() -> PokemonRenderData {
        PokemonRenderData::new(true)
    }

    #[test]
    fn move_name_en_spot_checks() {
        let rd = render_data_en();
        assert_eq!(rd.move_name(MoveId::Tackle), "TACKLE");
        assert_eq!(rd.move_name(MoveId::Thunder), "THUNDER");
        assert_eq!(rd.move_name(MoveId::Surf), "SURF");
        assert_eq!(rd.move_name(MoveId::None), "---");
    }

    #[test]
    fn move_name_zh_spot_checks() {
        let rd = render_data_zh();
        assert_eq!(rd.move_name(MoveId::Tackle), "撞击");
        assert_eq!(rd.move_name(MoveId::Thunder), "打雷");
        assert_eq!(rd.move_name(MoveId::Surf), "冲浪");
    }

    #[test]
    fn move_pp_tackle_is_35() {
        let rd = render_data_en();
        assert_eq!(rd.move_pp(MoveId::Tackle), (35, 35));
    }

    #[test]
    fn move_pp_thunder_is_10() {
        let rd = render_data_en();
        assert_eq!(rd.move_pp(MoveId::Thunder), (10, 10));
    }

    #[test]
    fn move_pp_hydro_pump_is_5() {
        let rd = render_data_en();
        assert_eq!(rd.move_pp(MoveId::HydroPump), (5, 5));
    }

    #[test]
    fn move_pp_none_is_zero() {
        let rd = render_data_en();
        assert_eq!(rd.move_pp(MoveId::None), (0, 0));
    }

    #[test]
    fn move_type_tackle_is_normal() {
        let rd = render_data_en();
        assert_eq!(rd.move_type(MoveId::Tackle), 0x00);
    }

    #[test]
    fn move_type_thunder_is_electric() {
        let rd = render_data_en();
        assert_eq!(rd.move_type(MoveId::Thunder), 0x17);
    }

    #[test]
    fn move_type_surf_is_water() {
        let rd = render_data_en();
        assert_eq!(rd.move_type(MoveId::Surf), 0x15);
    }

    #[test]
    fn item_name_master_ball() {
        let rd = render_data_en();
        assert_eq!(rd.item_name(ItemId::MasterBall), "MASTER BALL");
    }

    #[test]
    fn item_name_potion() {
        let rd = render_data_en();
        assert_eq!(rd.item_name(ItemId::Potion), "POTION");
    }

    #[test]
    fn item_name_bicycle() {
        let rd = render_data_en();
        assert_eq!(rd.item_name(ItemId::Bicycle), "BICYCLE");
    }

    #[test]
    fn item_name_no_item_fallback() {
        let rd = render_data_en();
        assert_eq!(rd.item_name(ItemId::NoItem), "???");
    }

    #[test]
    fn species_name_en_spot_checks() {
        let rd = render_data_en();
        assert_eq!(rd.species_name(Species::Bulbasaur), "BULBASAUR");
        assert_eq!(rd.species_name(Species::Pikachu), "PIKACHU");
        assert_eq!(rd.species_name(Species::Mewtwo), "MEWTWO");
        assert_eq!(rd.species_name(Species::Mew), "MEW");
        assert_eq!(rd.species_name(Species::None), "---");
    }

    #[test]
    fn species_name_zh_spot_checks() {
        let rd = render_data_zh();
        assert_eq!(rd.species_name(Species::Bulbasaur), "妙蛙种子");
        assert_eq!(rd.species_name(Species::Pikachu), "皮卡丘");
        assert_eq!(rd.species_name(Species::Mewtwo), "超梦");
        assert_eq!(rd.species_name(Species::Mew), "梦幻");
    }

    #[test]
    fn all_valid_moves_have_positive_pp() {
        let rd = render_data_en();
        use strum::IntoEnumIterator;
        for m in MoveId::iter() {
            if m == MoveId::None {
                continue;
            }
            let (pp, _) = rd.move_pp(m);
            assert!(pp > 0, "{:?} has 0 PP", m);
        }
    }

    #[test]
    fn all_defined_items_have_nonempty_names() {
        let rd = render_data_en();
        use strum::IntoEnumIterator;
        for item in ItemId::iter() {
            if item == ItemId::NoItem {
                continue;
            }
            let val = item as u8;
            if val > 83 {
                continue;
            }
            let name = rd.item_name(item);
            assert!(!name.is_empty(), "{:?} has empty name", item);
        }
    }
}

#[cfg(test)]
mod tile_meta_tests {
    use super::*;
    use crate::tilesets::TilesetId;

    fn meta() -> PokemonTileMetadata {
        PokemonTileMetadata
    }

    // -- TileMetaTrait --------------------------------------------------

    #[test]
    fn tileset_id_is_tile_meta_trait() {
        fn _assert_trait(t: TilesetId) -> impl TileMetaTrait {
            t
        }
        let _ = _assert_trait(TilesetId::Overworld);
    }

    #[test]
    fn tileset_id_satisfies_trait_bounds() {
        // Copy
        let a = TilesetId::Overworld;
        let _b = a;
        let _c = a;
        // Eq
        assert_eq!(TilesetId::Forest, TilesetId::Forest);
        assert_ne!(TilesetId::Forest, TilesetId::Cavern);
        // Hash
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TilesetId::Overworld);
        assert!(set.contains(&TilesetId::Overworld));
        // Debug
        let _ = format!("{:?}", TilesetId::House);
    }

    // -- is_passable ----------------------------------------------------

    #[test]
    fn is_passable_known_passable_overworld() {
        let m = meta();
        assert!(m.is_passable(TilesetId::Overworld, 0x00));
        assert!(m.is_passable(TilesetId::Overworld, 0x10));
    }

    #[test]
    fn is_passable_known_impassable_overworld() {
        let m = meta();
        assert!(!m.is_passable(TilesetId::Overworld, 0x01));
        assert!(!m.is_passable(TilesetId::Overworld, 0x0F));
    }

    #[test]
    fn is_passable_mart_tiles() {
        let m = meta();
        assert!(m.is_passable(TilesetId::Mart, 0x11));
        assert!(!m.is_passable(TilesetId::Mart, 0x00));
    }

    // -- is_ledge -------------------------------------------------------

    #[test]
    fn is_ledge_known_ledge_tiles() {
        let m = meta();
        assert!(m.is_ledge(TilesetId::Overworld, 0x37));
        assert!(m.is_ledge(TilesetId::Overworld, 0x36));
        assert!(m.is_ledge(TilesetId::Overworld, 0x27));
        assert!(m.is_ledge(TilesetId::Overworld, 0x0D));
    }

    #[test]
    fn is_ledge_non_ledge_tiles() {
        let m = meta();
        assert!(!m.is_ledge(TilesetId::Overworld, 0x00));
        assert!(!m.is_ledge(TilesetId::Overworld, 0x01));
    }

    // -- is_counter -----------------------------------------------------

    #[test]
    fn is_counter_mart_counter_tiles() {
        let m = meta();
        assert!(m.is_counter(TilesetId::Mart, 0x18));
        assert!(m.is_counter(TilesetId::Mart, 0x19));
        assert!(m.is_counter(TilesetId::Mart, 0x1E));
    }

    #[test]
    fn is_counter_non_counter_tile() {
        let m = meta();
        assert!(!m.is_counter(TilesetId::Mart, 0x11));
        assert!(!m.is_counter(TilesetId::Overworld, 0x00));
    }

    // -- is_grass -------------------------------------------------------

    #[test]
    fn is_grass_overworld_grass_tile() {
        let m = meta();
        assert!(m.is_grass(TilesetId::Overworld, 0x52));
        assert!(!m.is_grass(TilesetId::Overworld, 0x00));
    }

    #[test]
    fn is_grass_no_grass_in_mart() {
        let m = meta();
        assert!(!m.is_grass(TilesetId::Mart, 0x00));
        assert!(!m.is_grass(TilesetId::Mart, 0x11));
    }

    // -- get_grass_tile -------------------------------------------------

    #[test]
    fn get_grass_tile_overworld() {
        let m = meta();
        assert_eq!(m.get_grass_tile(TilesetId::Overworld), Some(0x52));
    }

    #[test]
    fn get_grass_tile_mart_has_none() {
        let m = meta();
        assert_eq!(m.get_grass_tile(TilesetId::Mart), None);
    }

    // -- collision_type -------------------------------------------------

    #[test]
    fn collision_type_passable_floor() {
        let m = meta();
        assert_eq!(
            m.collision_type(TilesetId::Overworld, 0x00),
            CollisionType::Passable
        );
    }

    #[test]
    fn collision_type_impassable_wall() {
        let m = meta();
        assert_eq!(
            m.collision_type(TilesetId::Overworld, 0x01),
            CollisionType::Impassable
        );
    }

    #[test]
    fn collision_type_ledge() {
        let m = meta();
        assert_eq!(
            m.collision_type(TilesetId::Overworld, 0x37),
            CollisionType::Ledge { direction: 0x00 }
        );
        assert_eq!(
            m.collision_type(TilesetId::Overworld, 0x27),
            CollisionType::Ledge { direction: 0x08 }
        );
    }

    #[test]
    fn collision_type_counter() {
        let m = meta();
        assert_eq!(
            m.collision_type(TilesetId::Mart, 0x18),
            CollisionType::Counter
        );
    }

    #[test]
    fn collision_type_grass() {
        let m = meta();
        assert_eq!(
            m.collision_type(TilesetId::Overworld, 0x52),
            CollisionType::Grass(Some(0x52))
        );
    }

    #[test]
    fn collision_type_plateau_grass() {
        let m = meta();
        assert_eq!(
            m.collision_type(TilesetId::Plateau, 0x45),
            CollisionType::Grass(Some(0x45))
        );
    }

    // -- Custom tileset delegation --------------------------------------

    #[test]
    fn custom_tileset_delegates_to_base_for_counter() {
        let m = meta();
        let custom = TilesetId::Custom(255);
        assert!(!m.is_counter(custom, 0x18));
    }

    #[test]
    fn custom_tileset_delegates_to_base_for_grass() {
        let m = meta();
        let custom = TilesetId::Custom(255);
        assert_eq!(m.get_grass_tile(custom), Some(0x52));
    }
}

// ============================================================================
// GameData master trait implementation
// ============================================================================

/// Master game data implementation for Pokémon Red.
///
/// `PokemonRedData` is a zero-sized type that implements the [`GameData`] trait,
/// connecting all five sub-providers (tilesets, maps, palettes, tile metadata,
/// and render data) into a single dependency-injection point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PokemonRedData;

/// Static instance of the palette provider for Pokémon Red.
static POKEMON_PALETTE_DATA: PokemonPaletteData = PokemonPaletteData { is_red: true };

/// Static instance of the render data provider (English).
static POKEMON_RENDER_DATA: PokemonRenderData = PokemonRenderData { is_zh: false };

impl GameData for PokemonRedData {
    type Tileset = TilesetId;
    type Map = MapId;
    type Palette = SgbPaletteId;
    type TileMeta = TilesetId;
    type Move = MoveId;
    type Item = ItemId;
    type Species = Species;

    fn tileset_provider(&self) -> &dyn TilesetProvider<TilesetId> {
        &PokemonTilesetData
    }

    fn map_provider(&self) -> &dyn MapProvider<MapId> {
        &PokemonMapData
    }

    fn palette_provider(&self) -> &dyn PaletteProvider<SgbPaletteId> {
        &POKEMON_PALETTE_DATA
    }

    fn tile_metadata(&self) -> &dyn TileMetadata<TilesetId> {
        &PokemonTileMetadata
    }

    fn render_data(
        &self,
    ) -> &dyn RenderData<Move = MoveId, Item = ItemId, Species = Species> {
        &POKEMON_RENDER_DATA
    }
}

#[cfg(test)]
mod game_data_tests {
    use super::*;

    #[test]
    fn test_tileset_provider() {
        let data = PokemonRedData;
        let count = data.tileset_provider().tileset_count();
        assert_eq!(count, 24);
    }

    #[test]
    fn test_map_provider() {
        let data = PokemonRedData;
        let dims = data.map_provider().dimensions(MapId::PalletTown);
        assert_eq!(dims, (10, 9));
    }

    #[test]
    fn test_palette_provider() {
        let data = PokemonRedData;
        let pal = data.palette_provider().bg_palette(SgbPaletteId::Route);
        assert_eq!(pal, [0, 1, 2, 3]);
    }

    #[test]
    fn test_tile_metadata() {
        let data = PokemonRedData;
        let passable = data.tile_metadata().is_passable(TilesetId::Overworld, 0x00);
        assert!(passable);
    }

    #[test]
    fn test_render_data() {
        let data = PokemonRedData;
        let name = data.render_data().species_name(Species::Bulbasaur);
        assert_eq!(name, "BULBASAUR");
    }

    #[test]
    fn test_type_inference() {
        let data = PokemonRedData;
        let _ = data.tileset_provider().tileset_count();
        let _ = data.map_provider().dimensions(MapId::Route1);
        let _ = data.palette_provider().bg_palette(SgbPaletteId::RedMon);
        let _ = data.tile_metadata().is_passable(TilesetId::Overworld, 0x01);
        let _ = data.render_data().move_name(MoveId::Thunder);
    }
}

// ============================================================================
// Battle trait implementations for PokemonRedData
// ============================================================================

use crate::move_data::MoveData;
use crate::pokemon_data;
use crate::type_chart;
use crate::types::PokemonType;
use jrpg_engine::battle::{
    BattleAI, BattleAction, BattleProvider, BattleRng, BattleState, BattlerRef, BattlerState,
    DamageResult, EffectHandler, EffectResult, EnumMap, MoveEffect, OrderKey, TypeChart,
};

// ── StatusCondition (moved from pokered-core for trait impl availability) ──

/// Non-volatile status condition. Only one active at a time.
/// Sleep counter: 1-7, decremented each turn the mon tries to act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum StatusCondition {
    #[default]
    None,
    Sleep(u8),
    Poison,
    Burn,
    Freeze,
    Paralysis,
}

impl StatusCondition {
    pub fn is_none(&self) -> bool {
        matches!(self, StatusCondition::None)
    }

    pub fn is_sleep(&self) -> bool {
        matches!(self, StatusCondition::Sleep(_))
    }

    pub fn is_frozen(&self) -> bool {
        matches!(self, StatusCondition::Freeze)
    }
}

// ── StatIndex (moved from pokered-core for trait impl availability) ──

/// Stat categories for stage modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StatIndex {
    Attack,
    Defense,
    Speed,
    Special,
    Accuracy,
    Evasion,
}

// ── NoAbility (Gen 1 has no abilities) ──

/// Gen 1 has no abilities — empty marker type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoAbility;

// ── TypeChart implementation ───────────────────────────────────────────

impl TypeChart for PokemonRedData {
    type Type = PokemonType;

    fn effectiveness(attacking: &Self::Type, defending: &[Self::Type]) -> f32 {
        if defending.is_empty() {
            return 1.0;
        }
        let mut multiplier = 1.0;
        for def_type in defending {
            let eff = type_chart::get_effectiveness(*attacking, *def_type);
            multiplier *= eff.multiplier();
        }
        multiplier
    }
}

// ── BattleProvider implementation ──────────────────────────────────────

/// Stage multiplier lookup: indices 0..12 map to stage values -6..+6.
const STAGE_MULTIPLIERS: [f32; 13] = [
    0.25, 0.28, 0.33, 0.40, 0.50, 0.66, 1.00, 1.50, 2.00, 2.50, 3.00, 3.50, 4.00,
];

fn apply_stat_stage(base: u16, stage: i8) -> u16 {
    let idx = (stage + 6).clamp(0, 12) as usize;
    let mult = STAGE_MULTIPLIERS[idx];
    ((base as f32) * mult) as u16
}

impl BattleProvider for PokemonRedData {
    type Monster = (); // Pokemon struct lives in pokered-core; Monster is unused in method sigs
    type Move = MoveId;
    type Ability = NoAbility;
    type Status = StatusCondition;
    type Stat = StatIndex;
    type Species = Species;
    type Type = PokemonType;
    type Item = ItemId;

    fn calculate_damage(
        &self,
        move_: &Self::Move,
        attacker: &BattlerState<Self>,
        defender: &BattlerState<Self>,
        _random: u8,
        is_critical: bool,
    ) -> DamageResult {
        let move_data = match MoveData::get(*move_) {
            Some(md) => md,
            None => {
                return DamageResult {
                    damage: 0,
                    effectiveness: 0.0,
                    is_miss: true,
                };
            }
        };

        if move_data.power == 0 {
            return DamageResult {
                damage: 0,
                effectiveness: 1.0,
                is_miss: false,
            };
        }

        // Collect defender types from the defender's species data
        let defender_types: Vec<PokemonType> = pokemon_data::get_base_stats(defender.species)
            .map(|bs| {
                let mut types = vec![bs.type1];
                if bs.type2 != bs.type1 {
                    types.push(bs.type2);
                }
                types
            })
            .unwrap_or_else(|| vec![PokemonType::Normal]);

        let effectiveness =
            <PokemonRedData as TypeChart>::effectiveness(&move_data.move_type, &defender_types);

        if effectiveness == 0.0 {
            return DamageResult {
                damage: 0,
                effectiveness: 0.0,
                is_miss: true,
            };
        }

        // Extract stats from EnumMap
        let atk_stat = attacker.stats.get(StatIndex::Attack).copied().unwrap_or(0);
        let def_stat = defender.stats.get(StatIndex::Defense).copied().unwrap_or(1).max(1);
        let atk_stage = attacker.stat_stages.get(StatIndex::Attack).copied().unwrap_or(0);
        let def_stage = defender.stat_stages.get(StatIndex::Defense).copied().unwrap_or(0);

        let mut attack = apply_stat_stage(atk_stat, atk_stage);
        let mut defense = apply_stat_stage(def_stat, def_stage);

        // Gen 1 stat scaling: if either > 255, both are divided by 4 (min 1)
        if attack > 255 || defense > 255 {
            attack = (attack / 4).max(1);
            defense = (defense / 4).max(1);
        }

        let level = attacker
            .stats
            .get(StatIndex::Speed) // Using speed stat as proxy for level (no level field)
            .copied()
            .unwrap_or(50) as u32;

        // Gen 1 damage formula
        let crit_mult = if is_critical { 2 } else { 1 };
        let base = ((2 * level * crit_mult / 5 + 2) as u32)
            .saturating_mul(move_data.power as u32)
            .saturating_mul(attack as u32)
            / (defense as u32)
            / 50
            + 2;

        let damage = ((base as f32) * effectiveness) as u16;

        DamageResult {
            damage: damage.max(1),
            effectiveness,
            is_miss: false,
        }
    }

    fn select_move(
        &self,
        battler: &BattlerState<Self>,
        _state: &BattleState<Self>,
    ) -> Self::Move {
        battler.moves.first().copied().unwrap_or(MoveId::None)
    }

    fn apply_move_effect(
        &self,
        effect: MoveEffect,
        user: &mut BattlerState<Self>,
        target: &mut BattlerState<Self>,
    ) -> EffectResult {
        match effect {
            MoveEffect::Damage => {
                // Simplified: deal 10 damage
                target.take_damage(10);
                EffectResult::DamageDealt { amount: 10 }
            }
            MoveEffect::Heal => {
                let amount = user.max_hp / 4;
                user.heal(amount);
                EffectResult::Healed { amount }
            }
            MoveEffect::StatusCondition => {
                if target.status.is_none() {
                    target.status = Some(StatusCondition::Poison);
                    EffectResult::StatusInflicted
                } else {
                    EffectResult::StatusFailed
                }
            }
            MoveEffect::StatChange => {
                target.stat_stages.set(StatIndex::Attack, -1);
                EffectResult::StatModified { stages: -1 }
            }
            MoveEffect::MultiHit => EffectResult::MultiHit { hits: 2 },
            MoveEffect::Recharge => EffectResult::MustRecharge,
            MoveEffect::DrainHp => {
                let drained = 10u16;
                target.take_damage(drained);
                user.heal(drained / 2);
                EffectResult::HpDrained { drained }
            }
            MoveEffect::Recoil => {
                let recoil = 5u16;
                user.take_damage(recoil);
                EffectResult::RecoilDamage { recoil }
            }
            MoveEffect::Flinch => EffectResult::NoEffect,
            MoveEffect::FieldEffect => EffectResult::FieldEffectSet,
            MoveEffect::SpecialDamage => EffectResult::NoEffect,
            MoveEffect::Ohko => EffectResult::NoEffect,
            MoveEffect::MultiTurn => EffectResult::NoEffect,
        }
    }

    fn create_monster(&self, species: Self::Species, level: u8) -> BattlerState<Self> {
        let level = level.max(1).min(100);
        let stats_data = pokemon_data::get_base_stats(species);

        let (hp, atk, def, spd, spc, _type1, _type2, moves) = if let Some(base) = stats_data {
            let hp = ((base.hp as u32 * 2 * level as u32 / 100) + level as u32 + 10) as u16;
            let atk = ((base.attack as u32 * 2 * level as u32 / 100) + 5) as u16;
            let def = ((base.defense as u32 * 2 * level as u32 / 100) + 5) as u16;
            let spd = ((base.speed as u32 * 2 * level as u32 / 100) + 5) as u16;
            let spc = ((base.special as u32 * 2 * level as u32 / 100) + 5) as u16;
            (hp, atk, def, spd, spc, base.type1, base.type2, base.initial_moves)
        } else {
            (10, 5, 5, 5, 5, PokemonType::Normal, PokemonType::Normal, [MoveId::None; 4])
        };

        let mut stats = EnumMap::new();
        stats.set(StatIndex::Attack, atk);
        stats.set(StatIndex::Defense, def);
        stats.set(StatIndex::Speed, spd);
        stats.set(StatIndex::Special, spc);
        // HP is not in EnumMap — it's directly on BattlerState fields

        let known_moves: Vec<MoveId> = moves.iter().filter(|m| **m != MoveId::None).copied().collect();

        BattlerState::new(species, hp, hp, stats, known_moves)
    }

    /// Gen-1 turn ordering, reproducing `pokered-core`'s legacy
    /// `battle::turn_order::determine_order`:
    ///
    /// 1. Move priority bracket (Quick Attack +1, Counter -1, else 0).
    /// 2. Effective speed (stat-stage scaled, quartered by paralysis, min 1).
    /// 3. Equal-speed Gen-1 coin flip: one RNG byte, `< 128` ⇒ player first.
    ///
    /// The engine stable-sorts actors **ascending** by [`OrderKey`] and keeps
    /// submission order (player before opponent) on a tie, so "acts earlier"
    /// must be the *smaller* key — hence priority and speed are negated.
    ///
    /// ## Draw-order / tie-break reconciliation
    ///
    /// Legacy draws **exactly one** byte for the whole order decision
    /// (`order_random`) and only consults it on a speed tie. The engine instead
    /// calls this hook once per actor, so to keep the draw count and outcome
    /// identical we make **only the player's** call draw one byte (the opponent
    /// draws none → one byte per turn, matching legacy), and we encode the
    /// coin-flip *result* directly into the key:
    ///
    /// * player tie-break = `0` if `byte < 128` else `255`;
    /// * opponent tie-break = `128` (constant, no draw).
    ///
    /// Ascending order then puts the player first exactly when `byte < 128`,
    /// reproducing `random_byte < 128 ⇒ PlayerFirst` without a second draw.
    fn turn_order_key(
        &self,
        state: &BattleState<Self>,
        who: BattlerRef,
        action: &BattleAction<Self>,
        rng: &mut dyn BattleRng,
    ) -> OrderKey {
        let party = if who.side == 0 {
            &state.player_battlers
        } else {
            &state.opponent_battlers
        };
        let battler = party.get(who.slot as usize);

        // Move priority bracket from the chosen action's move.
        let priority: i32 = match action {
            BattleAction::Fight { move_ } => match move_ {
                MoveId::QuickAttack => 1,
                MoveId::Counter => -1,
                _ => 0,
            },
            _ => 0,
        };

        // Effective speed: stat-stage scaled, paralysis quarters (min 1) —
        // mirroring `turn_order::effective_speed`.
        let base_speed = battler
            .and_then(|b| b.stats.get(StatIndex::Speed).copied())
            .unwrap_or(0);
        let speed_stage = battler
            .and_then(|b| b.stat_stages.get(StatIndex::Speed).copied())
            .unwrap_or(0);
        let staged = apply_stat_stage(base_speed, speed_stage);
        let is_paralyzed = battler
            .map(|b| b.status == Some(StatusCondition::Paralysis))
            .unwrap_or(false);
        let effective_speed: i32 = if is_paralyzed {
            (staged / 4).max(1) as i32
        } else {
            staged as i32
        };

        // Coin-flip tie-break: only the player draws (one byte per turn, as in
        // legacy), encoding `byte < 128 ⇒ player first` into the key.
        let tiebreak: u32 = if who.side == 0 {
            if rng.next_u8() < 128 {
                0
            } else {
                255
            }
        } else {
            128
        };

        OrderKey(-priority, -effective_speed, tiebreak)
    }
}

// ── BattleAI implementation ────────────────────────────────────────────

impl BattleAI<PokemonRedData> for PokemonRedData {
    fn select_move(
        &self,
        battler: &BattlerState<PokemonRedData>,
        _state: &BattleState<PokemonRedData>,
    ) -> MoveId {
        battler.moves.first().copied().unwrap_or(MoveId::None)
    }

    fn should_switch(&self, battler: &BattlerState<PokemonRedData>) -> bool {
        battler.hp < battler.max_hp / 4
    }

    fn should_use_item(
        &self,
        battler: &BattlerState<PokemonRedData>,
    ) -> Option<ItemId> {
        if battler.hp < battler.max_hp / 3 {
            Some(ItemId::Potion)
        } else {
            None
        }
    }
}

// ── EffectHandler implementation ───────────────────────────────────────

impl EffectHandler<PokemonRedData> for PokemonRedData {
    fn handle_effect(
        &self,
        effect: MoveEffect,
        user: &mut BattlerState<PokemonRedData>,
        target: &mut BattlerState<PokemonRedData>,
        _provider: &PokemonRedData,
    ) -> EffectResult {
        match effect {
            MoveEffect::Damage => {
                target.take_damage(10);
                EffectResult::DamageDealt { amount: 10 }
            }
            MoveEffect::Heal => {
                let amount = user.max_hp / 4;
                user.heal(amount);
                EffectResult::Healed { amount }
            }
            MoveEffect::StatusCondition => {
                if target.status.is_none() {
                    target.status = Some(StatusCondition::Paralysis);
                    EffectResult::StatusInflicted
                } else {
                    EffectResult::StatusFailed
                }
            }
            MoveEffect::StatChange => {
                target.stat_stages.set(StatIndex::Attack, -1);
                EffectResult::StatModified { stages: -1 }
            }
            MoveEffect::MultiHit => EffectResult::MultiHit { hits: 3 },
            MoveEffect::Recharge => EffectResult::MustRecharge,
            _ => EffectResult::NoEffect,
        }
    }
}

// ── Battle trait tests ─────────────────────────────────────────────────

#[cfg(test)]
mod battle_trait_tests {
    use super::*;

    #[test]
    fn type_chart_neutral_normal_vs_normal() {
        let eff =
            <PokemonRedData as TypeChart>::effectiveness(&PokemonType::Normal, &[PokemonType::Normal]);
        assert!((eff - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn type_chart_super_effective_water_vs_fire() {
        let eff =
            <PokemonRedData as TypeChart>::effectiveness(&PokemonType::Water, &[PokemonType::Fire]);
        assert!((eff - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn type_chart_not_very_effective_fire_vs_water() {
        let eff =
            <PokemonRedData as TypeChart>::effectiveness(&PokemonType::Fire, &[PokemonType::Water]);
        assert!((eff - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn type_chart_immune_normal_vs_ghost() {
        let eff =
            <PokemonRedData as TypeChart>::effectiveness(&PokemonType::Normal, &[PokemonType::Ghost]);
        assert!((eff - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn type_chart_dual_type() {
        let eff = <PokemonRedData as TypeChart>::effectiveness(
            &PokemonType::Water,
            &[PokemonType::Fire, PokemonType::Rock],
        );
        assert!((eff - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn type_chart_empty_defending_is_neutral() {
        let eff = <PokemonRedData as TypeChart>::effectiveness(&PokemonType::Water, &[]);
        assert!((eff - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn provider_can_be_constructed() {
        let provider = PokemonRedData;
        let mon = provider.create_monster(Species::Bulbasaur, 5);
        assert!(mon.hp > 0);
    }

    #[test]
    fn create_monster_bulbasaur_level_5() {
        let provider = PokemonRedData;
        let mon = provider.create_monster(Species::Bulbasaur, 5);
        assert_eq!(mon.species, Species::Bulbasaur);
        assert_eq!(mon.hp, mon.max_hp);
        assert!(mon.hp > 10);
        assert!(!mon.moves.is_empty());
    }

    #[test]
    fn create_monster_none_species_has_defaults() {
        let provider = PokemonRedData;
        let mon = provider.create_monster(Species::None, 5);
        assert_eq!(mon.species, Species::None);
        assert!(mon.hp > 0);
    }

    #[test]
    fn calculate_damage_base_case() {
        let provider = PokemonRedData;
        let attacker = provider.create_monster(Species::Pikachu, 50);
        let defender = provider.create_monster(Species::Bulbasaur, 50);
        let result =
            provider.calculate_damage(&MoveId::Thundershock, &attacker, &defender, 255, false);
        assert!(result.damage > 0);
        assert!(!result.is_miss);
        assert!(result.effectiveness > 0.0);
    }

    #[test]
    fn calculate_damage_none_move_returns_miss() {
        let provider = PokemonRedData;
        let attacker = provider.create_monster(Species::Pikachu, 50);
        let defender = provider.create_monster(Species::Bulbasaur, 50);
        let result = provider.calculate_damage(&MoveId::None, &attacker, &defender, 255, false);
        assert!(result.is_miss);
        assert_eq!(result.damage, 0);
    }

    #[test]
    fn select_move_returns_first_available() {
        let provider = PokemonRedData;
        let mut mon = provider.create_monster(Species::Pikachu, 50);
        mon.moves = vec![MoveId::Thunderbolt, MoveId::QuickAttack];
        let state = BattleState::<PokemonRedData>::new(vec![mon.clone()], vec![]);
        let selected = <PokemonRedData as BattleProvider>::select_move(&provider, &mon, &state);
        assert_eq!(selected, MoveId::Thunderbolt);
    }

    #[test]
    fn apply_move_effect_damage() {
        let provider = PokemonRedData;
        let mut user = provider.create_monster(Species::Pikachu, 50);
        let mut target = provider.create_monster(Species::Bulbasaur, 50);
        let before = target.hp;
        let result = provider.apply_move_effect(MoveEffect::Damage, &mut user, &mut target);
        assert!(matches!(result, EffectResult::DamageDealt { .. }));
        assert!(target.hp < before);
    }

    #[test]
    fn apply_move_effect_heal() {
        let provider = PokemonRedData;
        let mut user = provider.create_monster(Species::Pikachu, 50);
        user.hp = 10;
        let mut target = provider.create_monster(Species::Bulbasaur, 50);
        let result = provider.apply_move_effect(MoveEffect::Heal, &mut user, &mut target);
        assert!(matches!(result, EffectResult::Healed { .. }));
        assert!(user.hp > 10);
    }

    #[test]
    fn check_faint_detects_zero() {
        let provider = PokemonRedData;
        let mut mon = provider.create_monster(Species::Pikachu, 50);
        assert!(!provider.check_faint(&mon));
        mon.hp = 0;
        assert!(provider.check_faint(&mon));
    }

    #[test]
    fn ai_select_move_works() {
        let provider = PokemonRedData;
        let mut mon = provider.create_monster(Species::Pikachu, 50);
        mon.moves = vec![MoveId::Thunderbolt];
        let state = BattleState::<PokemonRedData>::new(vec![mon.clone()], vec![]);
        let chosen = BattleAI::<PokemonRedData>::select_move(&provider, &mon, &state);
        assert_eq!(chosen, MoveId::Thunderbolt);
    }

    #[test]
    fn ai_should_switch_when_low_hp() {
        let provider = PokemonRedData;
        let mon = provider.create_monster(Species::Pikachu, 50);
        assert!(!provider.should_switch(&mon)); // full HP

        let mut low_hp = provider.create_monster(Species::Pikachu, 50);
        low_hp.hp = 5;
        assert!(provider.should_switch(&low_hp));
    }

    #[test]
    fn ai_should_use_item_when_low_hp() {
        let provider = PokemonRedData;
        let mon = provider.create_monster(Species::Pikachu, 50);
        assert!(provider.should_use_item(&mon).is_none()); // full HP

        let mut low_hp = provider.create_monster(Species::Pikachu, 50);
        low_hp.hp = 5;
        assert_eq!(provider.should_use_item(&low_hp), Some(ItemId::Potion));
    }

    #[test]
    fn effect_handler_damage_works() {
        let provider = PokemonRedData;
        let mut user = provider.create_monster(Species::Pikachu, 50);
        let mut target = provider.create_monster(Species::Bulbasaur, 50);
        let before = target.hp;
        let result = EffectHandler::<PokemonRedData>::handle_effect(
            &provider,
            MoveEffect::Damage,
            &mut user,
            &mut target,
            &provider,
        );
        assert!(matches!(result, EffectResult::DamageDealt { .. }));
        assert!(target.hp < before);
    }

    #[test]
    fn effect_handler_status_infliction() {
        let provider = PokemonRedData;
        let mut user = provider.create_monster(Species::Pikachu, 50);
        let mut target = provider.create_monster(Species::Bulbasaur, 50);
        assert!(target.status.is_none());
        let result = EffectHandler::<PokemonRedData>::handle_effect(
            &provider,
            MoveEffect::StatusCondition,
            &mut user,
            &mut target,
            &provider,
        );
        assert_eq!(result, EffectResult::StatusInflicted);
        assert!(!target.status.is_none());
    }

    #[test]
    fn effect_handler_status_fails_when_already_afflicted() {
        let provider = PokemonRedData;
        let mut user = provider.create_monster(Species::Pikachu, 50);
        let mut target = provider.create_monster(Species::Bulbasaur, 50);
        target.status = Some(StatusCondition::Burn);
        let result = EffectHandler::<PokemonRedData>::handle_effect(
            &provider,
            MoveEffect::StatusCondition,
            &mut user,
            &mut target,
            &provider,
        );
        assert_eq!(result, EffectResult::StatusFailed);
    }

    #[test]
    fn battler_state_take_damage() {
        let provider = PokemonRedData;
        let mut mon = provider.create_monster(Species::Pikachu, 50);
        let original = mon.hp;
        mon.take_damage(10);
        assert_eq!(mon.hp, original - 10);
    }

    #[test]
    fn battler_state_heal() {
        let provider = PokemonRedData;
        let mut mon = provider.create_monster(Species::Pikachu, 50);
        mon.hp = 10;
        mon.heal(20);
        assert_eq!(mon.hp, 30);
    }

    #[test]
    fn battler_state_heal_caps_at_max() {
        let provider = PokemonRedData;
        let mut mon = provider.create_monster(Species::Pikachu, 50);
        let max = mon.max_hp;
        mon.hp = max - 5;
        mon.heal(100);
        assert_eq!(mon.hp, max);
    }

    #[test]
    fn battle_state_new_and_active() {
        let provider = PokemonRedData;
        let p1 = provider.create_monster(Species::Pikachu, 50);
        let o1 = provider.create_monster(Species::Bulbasaur, 50);
        let state = BattleState::<PokemonRedData>::new(vec![p1], vec![o1]);
        assert_eq!(state.player_battlers.len(), 1);
        assert_eq!(state.opponent_battlers.len(), 1);
        assert!(state.active_player().is_some());
        assert!(state.active_opponent().is_some());
    }
}

// ============================================================================
// SaveData trait implementation for PokemonSaveData
// ============================================================================

use crate::save::{PokemonSaveData, SAV_FILE_SIZE};
use jrpg_engine::save::{SaveData, SaveError};

impl SaveData for PokemonSaveData {
    fn serialize(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn deserialize(data: &[u8]) -> Result<Self, SaveError> {
        Ok(Self {
            bytes: data.to_vec(),
        })
    }

    fn save_size() -> usize {
        SAV_FILE_SIZE + 2
    }
}

#[cfg(test)]
mod save_data_tests {
    use super::*;
    use jrpg_engine::save::{InMemoryStorage, SaveManager, SaveSlot, SaveStorage};

    #[test]
    fn test_pokemon_save_data_new_is_zero_filled() {
        let data = PokemonSaveData::new();
        assert_eq!(data.bytes.len(), SAV_FILE_SIZE);
        assert!(data.bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_pokemon_save_data_roundtrip_bytes() {
        let original = PokemonSaveData::new();
        let serialized = original.serialize();
        let restored = PokemonSaveData::deserialize(&serialized).unwrap();
        assert_eq!(original.bytes, restored.bytes);
    }

    #[test]
    fn test_pokemon_save_data_save_size() {
        assert_eq!(PokemonSaveData::save_size(), SAV_FILE_SIZE + 2);
    }

    #[test]
    fn test_save_manager_roundtrip() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<PokemonSaveData>::new(storage);

        let mut original = PokemonSaveData::new();
        original.bytes[0] = 0x42;
        original.bytes[SAV_FILE_SIZE - 1] = 0xFF;

        manager.save(SaveSlot::Slot1, &original).unwrap();
        let loaded = manager.load(SaveSlot::Slot1).unwrap();
        assert_eq!(original.bytes, loaded.bytes);
    }

    #[test]
    fn test_save_manager_empty_slot_error() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<PokemonSaveData>::new(storage);
        let result = manager.load(SaveSlot::Slot1);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_manager_corrupted_checksum() {
        let storage = Box::new(InMemoryStorage::new());
        let mut original = PokemonSaveData::new();
        original.bytes[100] = 0x99;

        let raw = original.serialize();
        let mut bad_payload = raw.clone();
        bad_payload.extend_from_slice(&[0xFF, 0xFF]);
        storage.write(0, &bad_payload).unwrap();

        let manager = SaveManager::<PokemonSaveData>::new(storage);
        assert!(manager.load(SaveSlot::Slot1).is_err());
    }

    #[test]
    fn test_save_manager_multi_slot() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<PokemonSaveData>::new(storage);

        let mut a = PokemonSaveData::new();
        a.bytes[0] = 0xAA;
        let mut b = PokemonSaveData::new();
        b.bytes[0] = 0xBB;

        manager.save(SaveSlot::Slot1, &a).unwrap();
        manager.save(SaveSlot::Slot2, &b).unwrap();

        assert_eq!(manager.load(SaveSlot::Slot1).unwrap().bytes[0], 0xAA);
        assert_eq!(manager.load(SaveSlot::Slot2).unwrap().bytes[0], 0xBB);
    }

    #[test]
    fn test_list_slots() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<PokemonSaveData>::new(storage);

        let slots = manager.list_slots();
        assert_eq!(slots.len(), 3);
        assert!(slots.iter().all(|(_, has)| !has));

        manager.save(SaveSlot::Slot2, &PokemonSaveData::new()).unwrap();
        let slots = manager.list_slots();
        assert!(!slots[0].1);
        assert!(slots[1].1);
        assert!(!slots[2].1);
    }

    #[test]
    fn test_delete_slot() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<PokemonSaveData>::new(storage);

        manager.save(SaveSlot::Slot1, &PokemonSaveData::new()).unwrap();
        assert!(manager.list_slots()[0].1);

        manager.delete(SaveSlot::Slot1).unwrap();
        assert!(!manager.list_slots()[0].1);
    }

    #[test]
    fn test_from_bytes_into_bytes() {
        let bytes = vec![1u8, 2, 3];
        let data = PokemonSaveData::from_bytes(bytes.clone());
        assert_eq!(data.as_bytes(), &[1, 2, 3]);
        assert_eq!(data.into_bytes(), bytes);
    }
}

// ============================================================================
// ItemProvider and ShopProvider implementations
// ============================================================================

use crate::items::{CustomKind, ItemEffect, ShopId};
use jrpg_engine::items::{ItemProvider, ItemResult, ShopProvider};

impl ItemProvider for PokemonRedData {
    type Item = ItemId;
    type Effect = ItemEffect;
    type Monster = ();
    type CustomKind = CustomKind;

    fn item_name(&self, item: &Self::Item) -> &str {
        item_data::get_item_data(*item)
            .map(|d| d.name)
            .unwrap_or("???")
    }

    fn item_description(&self, item: &Self::Item) -> &str {
        self.item_name(item)
    }

    fn item_effect(&self, item: &Self::Item) -> Self::Effect {
        match item {
            ItemId::Potion => ItemEffect::Heal(20),
            ItemId::SuperPotion => ItemEffect::Heal(50),
            ItemId::HyperPotion => ItemEffect::Heal(200),
            ItemId::MaxPotion => ItemEffect::FullHeal,
            ItemId::FullRestore => ItemEffect::FullRestore,
            ItemId::FreshWater => ItemEffect::Heal(50),
            ItemId::SodaPop => ItemEffect::Heal(60),
            ItemId::Lemonade => ItemEffect::Heal(80),
            ItemId::Revive => ItemEffect::Revive(false),
            ItemId::MaxRevive => ItemEffect::Revive(true),
            ItemId::Antidote => ItemEffect::CurePoison,
            ItemId::BurnHeal => ItemEffect::CureBurn,
            ItemId::IceHeal => ItemEffect::CureFreeze,
            ItemId::Awakening => ItemEffect::CureSleep,
            ItemId::ParlyzHeal => ItemEffect::CureParalysis,
            ItemId::FullHeal => ItemEffect::CureAllStatus,
            ItemId::HpUp => ItemEffect::Vitamin(0),
            ItemId::Protein => ItemEffect::Vitamin(1),
            ItemId::Iron => ItemEffect::Vitamin(2),
            ItemId::Carbos => ItemEffect::Vitamin(3),
            ItemId::Calcium => ItemEffect::Vitamin(4),
            ItemId::RareCandy => ItemEffect::RareCandy,
            ItemId::XAttack => ItemEffect::BattleStat(0),
            ItemId::XDefend => ItemEffect::BattleStat(1),
            ItemId::XSpeed => ItemEffect::BattleStat(2),
            ItemId::XSpecial => ItemEffect::BattleStat(3),
            ItemId::XAccuracy => ItemEffect::BattleFlag(0),
            ItemId::GuardSpec => ItemEffect::BattleFlag(1),
            ItemId::DireHit => ItemEffect::BattleFlag(2),
            ItemId::PokeDoll => ItemEffect::Escape,
            ItemId::Ether => ItemEffect::PpRestoreSingle(false),
            ItemId::MaxEther => ItemEffect::PpRestoreSingle(true),
            ItemId::Elixer => ItemEffect::PpRestoreAll(false),
            ItemId::MaxElixer => ItemEffect::PpRestoreAll(true),
            ItemId::PpUp => ItemEffect::PpUp,
            ItemId::MoonStone => ItemEffect::EvolutionStone(0),
            ItemId::FireStone => ItemEffect::EvolutionStone(1),
            ItemId::ThunderStone => ItemEffect::EvolutionStone(2),
            ItemId::WaterStone => ItemEffect::EvolutionStone(3),
            ItemId::LeafStone => ItemEffect::EvolutionStone(4),
            _ => ItemEffect::None,
        }
    }

    fn item_price(&self, item: &Self::Item) -> u32 {
        item_data::get_item_data(*item)
            .map(|d| d.price as u32)
            .unwrap_or(0)
    }

    fn can_use_outside_battle(&self, item: &Self::Item) -> bool {
        match self.item_effect(item) {
            ItemEffect::None | ItemEffect::Escape | ItemEffect::BattleStat(_) | ItemEffect::BattleFlag(_) => false,
            _ => true,
        }
    }

    fn can_use_in_battle(&self, item: &Self::Item) -> bool {
        match self.item_effect(item) {
            ItemEffect::None | ItemEffect::EvolutionStone(_) | ItemEffect::RareCandy => false,
            ItemEffect::Heal(_) | ItemEffect::FullHeal | ItemEffect::FullRestore
            | ItemEffect::Revive(_) | ItemEffect::CurePoison | ItemEffect::CureBurn
            | ItemEffect::CureFreeze | ItemEffect::CureSleep | ItemEffect::CureParalysis
            | ItemEffect::CureAllStatus | ItemEffect::Vitamin(_) | ItemEffect::BattleStat(_)
            | ItemEffect::BattleFlag(_) | ItemEffect::Escape | ItemEffect::PpRestoreSingle(_)
            | ItemEffect::PpRestoreAll(_) | ItemEffect::PpUp => true,
        }
    }

    fn use_on_monster(&self, item: &Self::Item, _monster: &mut Self::Monster) -> ItemResult {
        match self.item_effect(item) {
            ItemEffect::None => ItemResult::NotUsable,
            _ => ItemResult::Used,
        }
    }

    fn consume(&self, item: &Self::Item) -> bool {
        !item.is_key_item() && !item.is_badge()
    }

    fn item_kind(&self, item: &Self::Item) -> jrpg_engine::items::ItemKind<Self::CustomKind> {
        crate::items::item_kind(*item)
    }
}

impl ShopProvider for PokemonRedData {
    type Item = ItemId;
    type ShopId = ShopId;

    fn shop_inventory(&self, shop_id: &Self::ShopId) -> Vec<(Self::Item, u32)> {
        item_data::shop_inventory_for(*shop_id)
            .iter()
            .map(|&item| {
                let price = item_data::get_item_data(item)
                    .map(|d| d.price as u32)
                    .unwrap_or(0);
                (item, price)
            })
            .collect()
    }

    fn shop_name(&self, shop_id: &Self::ShopId) -> &str {
        item_data::shop_name_for(*shop_id)
    }
}

#[cfg(test)]
mod item_provider_tests {
    use super::*;

    fn provider() -> PokemonRedData {
        PokemonRedData
    }

    #[test]
    fn potion_name_and_price() {
        let p = provider();
        assert_eq!(p.item_name(&ItemId::Potion), "POTION");
        assert_eq!(p.item_price(&ItemId::Potion), 300);
    }

    #[test]
    fn master_ball_price_is_zero() {
        let p = provider();
        assert_eq!(p.item_price(&ItemId::MasterBall), 0);
    }

    #[test]
    fn potion_effect_is_heal_20() {
        let p = provider();
        assert_eq!(p.item_effect(&ItemId::Potion), ItemEffect::Heal(20));
    }

    #[test]
    fn full_restore_effect() {
        let p = provider();
        assert_eq!(p.item_effect(&ItemId::FullRestore), ItemEffect::FullRestore);
    }

    #[test]
    fn unused_item_has_no_effect() {
        let p = provider();
        assert_eq!(p.item_effect(&ItemId::NoItem), ItemEffect::None);
    }

    #[test]
    fn key_item_is_not_consumed() {
        let p = provider();
        assert!(!p.consume(&ItemId::Bicycle));
        assert!(!p.consume(&ItemId::Pokedex));
    }

    #[test]
    fn regular_item_is_consumed() {
        let p = provider();
        assert!(p.consume(&ItemId::Potion));
        assert!(p.consume(&ItemId::Antidote));
    }

    #[test]
    fn badge_is_not_consumed() {
        let p = provider();
        assert!(!p.consume(&ItemId::BoulderBadge));
    }

    #[test]
    fn can_use_outside_battle() {
        let p = provider();
        assert!(p.can_use_outside_battle(&ItemId::Potion));
        assert!(p.can_use_outside_battle(&ItemId::Antidote));
        assert!(!p.can_use_outside_battle(&ItemId::XAttack));
        assert!(!p.can_use_outside_battle(&ItemId::PokeDoll));
        assert!(!p.can_use_outside_battle(&ItemId::Bicycle));
    }

    #[test]
    fn can_use_in_battle() {
        let p = provider();
        assert!(p.can_use_in_battle(&ItemId::Potion));
        assert!(p.can_use_in_battle(&ItemId::XAttack));
        assert!(p.can_use_in_battle(&ItemId::PokeDoll));
        assert!(!p.can_use_in_battle(&ItemId::RareCandy));
        assert!(!p.can_use_in_battle(&ItemId::FireStone));
    }

    #[test]
    fn use_on_monster_returns_used_for_valid_item() {
        let p = provider();
        assert_eq!(
            p.use_on_monster(&ItemId::Potion, &mut ()),
            ItemResult::Used
        );
    }

    #[test]
    fn use_on_monster_returns_not_usable_for_no_item() {
        let p = provider();
        assert_eq!(
            p.use_on_monster(&ItemId::NoItem, &mut ()),
            ItemResult::NotUsable
        );
    }
}

#[cfg(test)]
mod shop_provider_tests {
    use super::*;

    fn provider() -> PokemonRedData {
        PokemonRedData
    }

    #[test]
    fn viridian_mart_inventory() {
        let p = provider();
        let inv = p.shop_inventory(&ShopId::ViridianMart);
        assert_eq!(inv.len(), 5);
        assert_eq!(inv[0], (ItemId::PokeBall, 200));
        assert_eq!(inv[1], (ItemId::Potion, 300));
    }

    #[test]
    fn celadon_4f_sells_stones() {
        let p = provider();
        let inv = p.shop_inventory(&ShopId::CeladonMart4F);
        assert_eq!(inv.len(), 5);
        assert!(inv.iter().any(|(item, _)| *item == ItemId::FireStone));
        assert!(inv.iter().any(|(item, _)| *item == ItemId::WaterStone));
    }

    #[test]
    fn indigo_plateau_sells_endgame_items() {
        let p = provider();
        let inv = p.shop_inventory(&ShopId::IndigoPlateauMart);
        assert!(inv.iter().any(|(item, _)| *item == ItemId::UltraBall));
        assert!(inv.iter().any(|(item, _)| *item == ItemId::FullRestore));
    }

    #[test]
    fn shop_names_are_non_empty() {
        let p = provider();
        assert!(!p.shop_name(&ShopId::ViridianMart).is_empty());
        assert!(!p.shop_name(&ShopId::CeladonMart2F).is_empty());
    }
}
