//! Generic bookshelf / wall-object text — a port of `PrintBookshelfText`
//! (engine/events/hidden_events/bookshelves.asm) + the
//! `BookshelfTileIDs` table (data/tilesets/bookshelf_tile_ids.asm).
//!
//! When the player presses A facing UP at a building tile, the faced tile is
//! matched against a per-tileset table; a hit prints the associated text —
//! bookshelves ("Crammed full of #MON books!"), the wall TOWN MAP (which then
//! opens the map), elevators, #MON mart/center merchandise shelves, and the
//! INDIGO PLATEAU statues (text varies with the player's X parity).
//! Runs BEFORE sign/NPC interaction, like the original's hidden-event pass.

use pokered_data::blockset_data;
use pokered_data::tilesets::TilesetId;

/// What the matched shelf/wall object shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookshelfText {
    /// `IndigoPlateauStatues`: "INDIGO PLATEAU" + a text that varies with the
    /// player's X-coordinate parity (even → "highest authority" wording).
    IndigoPlateauStatues,
    /// `TownMapText`: "A TOWN MAP." then the TOWN MAP screen opens.
    TownMap,
    /// `BookOrSculptureText` (Diglett-sculpture tile $38 at Celadon Mansion
    /// (8,6) handled by the caller via the coord check).
    PokemonBooks,
    /// `DiglettSculptureText`: "It's a sculpture of DIGLETT."
    DiglettSculpture,
    /// `ElevatorText`: "This is an elevator."
    Elevator,
    /// `PokemonStuffText`: "Wow! Tons of #MON stuff!"
    PokemonStuff,
}

type Entry = (&'static str, &'static [(u8, BookshelfText)]);

/// `BookshelfTileIDs` keyed on the tileset NAME as used in map.json headers.
const TABLE: &[Entry] = &[
    (
        "Plateau",
        &[(0x30, BookshelfText::IndigoPlateauStatues)],
    ),
    ("House", &[(0x3D, BookshelfText::TownMap), (0x1E, BookshelfText::PokemonBooks)]),
    ("Mansion", &[(0x32, BookshelfText::PokemonBooks)]),
    ("RedsHouse1", &[(0x32, BookshelfText::PokemonBooks)]),
    ("Lab", &[(0x28, BookshelfText::PokemonBooks)]),
    (
        "Lobby",
        &[
            (0x16, BookshelfText::Elevator),
            (0x50, BookshelfText::PokemonStuff),
            (0x52, BookshelfText::PokemonStuff),
        ],
    ),
    ("Gym", &[(0x1D, BookshelfText::PokemonBooks)]),
    ("Dojo", &[(0x1D, BookshelfText::PokemonBooks)]),
    ("Gate", &[(0x22, BookshelfText::PokemonBooks)]),
    ("Mart", &[(0x54, BookshelfText::PokemonStuff), (0x55, BookshelfText::PokemonStuff)]),
    (
        "Pokecenter",
        &[(0x54, BookshelfText::PokemonStuff), (0x55, BookshelfText::PokemonStuff)],
    ),
    ("Ship", &[(0x36, BookshelfText::PokemonBooks)]),
];

/// Match a faced tile against the bookshelf table. `tileset_name` is the
/// map header's tileset name (e.g. "Gym", "House").
pub fn lookup(tileset_name: &str, tile: u8) -> Option<BookshelfText> {
    let (_, entries) = TABLE.iter().find(|(name, _)| *name == tileset_name)?;
    entries
        .iter()
        .find(|(t, _)| *t == tile)
        .map(|(_, text)| *text)
}

/// Resolve the faced tile's concrete id for the position the player FACES
/// (same block→tile derivation as the step-processing tile lookup).
pub fn faced_tile_id(
    blocks: &[u8],
    width: u8,
    concrete: TilesetId,
    x: u8,
    y: u8,
) -> u8 {
    let block_x = (x / 2) as usize;
    let block_y = (y / 2) as usize;
    let sub_x = (x % 2) as usize;
    let sub_y = (y % 2) as usize;
    if block_x < width as usize {
        let block_idx = block_y * (width as usize) + block_x;
        if block_idx < blocks.len() {
            let block_id = blocks[block_idx];
            return blockset_data::block_tiles(concrete, block_id)
                .map(|t| t[(sub_y * 2 + 1) * 4 + sub_x * 2])
                .unwrap_or(0);
        }
    }
    0
}

/// The full text body for a matched shelf (English; `localize_message`
/// translates via the dialog table when the script language is zh).
pub fn text_for(kind: BookshelfText, player_x: u8) -> String {
    match kind {
        BookshelfText::IndigoPlateauStatues => {
            // Original prints "INDIGO PLATEAU", then — by X parity —
            // odd → "The ultimate goal of trainers!", even → "The highest
            // #MON authority" (both "…#MON LEAGUE HQ").
            let flavor = if player_x % 2 == 1 {
                "The ultimate goal\nof trainers!\n#MON LEAGUE HQ"
            } else {
                "The highest\n#MON authority\n#MON LEAGUE HQ"
            };
            format!("INDIGO PLATEAU\n\n{flavor}")
        }
        BookshelfText::TownMap => "A TOWN MAP.".to_string(),
        BookshelfText::PokemonBooks => "Crammed full of\n#MON books!".to_string(),
        BookshelfText::DiglettSculpture => "It's a sculpture\nof DIGLETT.".to_string(),
        BookshelfText::Elevator => "This is an\nelevator.".to_string(),
        BookshelfText::PokemonStuff => "Wow! Tons of\n#MON stuff!".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_bookshelf_tile_ids() {
        // data/tilesets/bookshelf_tile_ids.asm spot checks.
        assert_eq!(lookup("Plateau", 0x30), Some(BookshelfText::IndigoPlateauStatues));
        assert_eq!(lookup("House", 0x3D), Some(BookshelfText::TownMap));
        assert_eq!(lookup("House", 0x1E), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("Mansion", 0x32), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("RedsHouse1", 0x32), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("Lab", 0x28), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("Lobby", 0x16), Some(BookshelfText::Elevator));
        assert_eq!(lookup("Gym", 0x1D), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("Dojo", 0x1D), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("Gate", 0x22), Some(BookshelfText::PokemonBooks));
        assert_eq!(lookup("Mart", 0x54), Some(BookshelfText::PokemonStuff));
        assert_eq!(lookup("Pokecenter", 0x55), Some(BookshelfText::PokemonStuff));
        assert_eq!(lookup("Ship", 0x36), Some(BookshelfText::PokemonBooks));
        // Wrong tile / tileset → nothing.
        assert_eq!(lookup("House", 0x30), None);
        assert_eq!(lookup("Overworld", 0x30), None);
    }

    #[test]
    fn statue_text_varies_with_x_parity() {
        let odd = text_for(BookshelfText::IndigoPlateauStatues, 9);
        let even = text_for(BookshelfText::IndigoPlateauStatues, 10);
        assert!(odd.starts_with("INDIGO PLATEAU"));
        assert!(odd.contains("ultimate goal"), "odd x → trainers' goal");
        assert!(even.contains("highest"), "even x → highest authority");
        assert_ne!(odd, even);
    }
}
