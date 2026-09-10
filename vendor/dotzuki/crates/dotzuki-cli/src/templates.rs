//! Embedded project templates for `dotzuki new --template`.
//!
//! `your-first-game` is a byte-for-byte copy of
//! `examples/your-first-game/`; the test
//! `vendored_template_matches_examples` fails when the two drift, and the
//! test `your_first_game_template_scaffolds_and_checks` proves a scaffolded
//! copy passes `dotzuki check`.

/// `(relative path, file bytes)` entries of the `your-first-game` template,
/// in a deterministic (path-sorted) order.
pub const YOUR_FIRST_GAME: &[(&str, &[u8])] = &[
    (
        ".dotzuki-editor.json",
        include_bytes!("../templates/your-first-game/.dotzuki-editor.json"),
    ),
    (
        "README.md",
        include_bytes!("../templates/your-first-game/README.md"),
    ),
    (
        "data/rules.ron",
        include_bytes!("../templates/your-first-game/data/rules.ron"),
    ),
    (
        "data/maps/Hometown/map.tmx.json",
        include_bytes!("../templates/your-first-game/data/maps/Hometown/map.tmx.json"),
    ),
    (
        "data/maps/Hometown/objects.json",
        include_bytes!("../templates/your-first-game/data/maps/Hometown/objects.json"),
    ),
    (
        "data/maps/Hometown/script.scene",
        include_bytes!("../templates/your-first-game/data/maps/Hometown/script.scene"),
    ),
    (
        "data/maps/Hometown/tileset.png",
        include_bytes!("../templates/your-first-game/data/maps/Hometown/tileset.png"),
    ),
    (
        "data/maps/Clearing/map.tmx.json",
        include_bytes!("../templates/your-first-game/data/maps/Clearing/map.tmx.json"),
    ),
    (
        "data/maps/Clearing/objects.json",
        include_bytes!("../templates/your-first-game/data/maps/Clearing/objects.json"),
    ),
    (
        "data/maps/Clearing/script.scene",
        include_bytes!("../templates/your-first-game/data/maps/Clearing/script.scene"),
    ),
    (
        "data/maps/Clearing/tileset.png",
        include_bytes!("../templates/your-first-game/data/maps/Clearing/tileset.png"),
    ),
    (
        "data/heroes/aria.json",
        include_bytes!("../templates/your-first-game/data/heroes/aria.json"),
    ),
    (
        "data/spells/bubble.json",
        include_bytes!("../templates/your-first-game/data/spells/bubble.json"),
    ),
    (
        "data/spells/fire-bolt.json",
        include_bytes!("../templates/your-first-game/data/spells/fire-bolt.json"),
    ),
    (
        "data/spells/heal.json",
        include_bytes!("../templates/your-first-game/data/spells/heal.json"),
    ),
    (
        "data/spells/slash.json",
        include_bytes!("../templates/your-first-game/data/spells/slash.json"),
    ),
    (
        "data/encounters/rival.json",
        include_bytes!("../templates/your-first-game/data/encounters/rival.json"),
    ),
    (
        "data/items/potion.json",
        include_bytes!("../templates/your-first-game/data/items/potion.json"),
    ),
    (
        "data/monsters/goblin.json",
        include_bytes!("../templates/your-first-game/data/monsters/goblin.json"),
    ),
    (
        "data/monsters/slime.json",
        include_bytes!("../templates/your-first-game/data/monsters/slime.json"),
    ),
    (
        "assets/scenes/main.scene",
        include_bytes!("../templates/your-first-game/assets/scenes/main.scene"),
    ),
    (
        "gfx/README.md",
        include_bytes!("../templates/your-first-game/gfx/README.md"),
    ),
];

/// Template names accepted by `dotzuki new --template`.
pub const TEMPLATE_NAMES: &[&str] = &["empty", "your-first-game"];
