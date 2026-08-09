/// Verify the generated layout statics are addressable, parseable, and
/// correctly populated with the values from the JSON files.
use std::borrow::Cow;

use pokered_data::ui_layout::schema::{self, DIALOG_DEFAULT_LAYOUT};

// ── Dialog layout smoke tests ─────────────────────────────────────────

#[test]
fn cow_smoke_dialog_default_layout_addressable() {
    let layout = &DIALOG_DEFAULT_LAYOUT;

    assert_eq!(layout.box_0.id, "box_0");
    assert_eq!(layout.box_0.rect.tx, 0);
    assert_eq!(layout.box_0.rect.ty, 12);
    assert_eq!(layout.box_0.rect.tw, 20);
    assert_eq!(layout.box_0.rect.th, 6);

    assert_eq!(layout.cursor.tx, 17);
    assert_eq!(layout.cursor.base_ty, 3);
    assert_eq!(layout.cursor.row_step, 0);
    assert_eq!(layout.cursor.glyph, '\u{25BC}');

    let json = schema::get_layout_json("dialog");
    assert!(json.is_some());
    let json_str = json.unwrap();
    assert!(json_str.contains("\"screen\": \"dialog\""));

    assert!(schema::get_layout_json("nonexistent").is_none());
}

// ── Cow smoke test ────────────────────────────────────────────────────
//
// Verifies that all compile-time generated layout statics use
// `Cow::Borrowed` (zero-allocation) rather than `Cow::Owned`.

#[test]
fn cow_smoke_all_statics_are_borrowed() {
    let layout = &DIALOG_DEFAULT_LAYOUT;

    // BoxDef fields
    assert!(
        matches!(layout.box_0.id, Cow::Borrowed(_)),
        "box_0.id must be Cow::Borrowed"
    );
    assert!(
        matches!(layout.box_0.labels, Cow::Borrowed(_)),
        "box_0.labels must be Cow::Borrowed"
    );

    // Label text inside labels
    for label in layout.box_0.labels.iter() {
        assert!(
            matches!(label.text, Cow::Borrowed(_)),
            "label.text must be Cow::Borrowed"
        );
    }
}

// ── Serde roundtrip tests ─────────────────────────────────────────────
//
// Deserialize every compiled layout into ScreenLayout to verify the serde
// schema covers all fields the `.gui` compiler emits. (The hand-filled
// `ui_layouts/*.json` files this used to read were replaced by `.gui`
// sources compiled at build time; the JSON now comes from the registry.)

#[cfg(feature = "serde")]
mod serde_tests {
    use std::collections::BTreeMap;

    use pokered_data::ui_layout::schema;
    use pokered_data::ui_layout::ScreenLayout;

    const SCREENS: [&str; 20] = [
        "bag", "battle_bag", "battle_main", "battle_move", "battle_party",
        "battle_text", "dialog", "main", "mart", "naming", "oak_speech",
        "options", "party", "pokedex", "pokedex_list", "save", "start",
        "stats", "title", "yes_no",
    ];

    #[test]
    fn roundtrip_all_layouts_parse() {
        let mut count = 0;
        let mut errors: BTreeMap<&str, String> = BTreeMap::new();

        for screen in SCREENS {
            let json = schema::get_layout_json(screen)
                .unwrap_or_else(|| panic!("{screen}: missing from layout registry"));

            let val: serde_json::Value =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{screen}: bad JSON: {e}"));
            let sv = val.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(0);
            assert!(sv == 1 || sv == 2, "{screen}: unexpected schema_version {sv}");
            assert_eq!(
                val.get("screen").and_then(|v| v.as_str()).unwrap_or(""),
                screen,
                "{screen}: screen field mismatch"
            );

            if sv == 1 {
                // v1 documents must fully deserialize into the typed schema.
                match serde_json::from_str::<ScreenLayout>(&json) {
                    Ok(_) => count += 1,
                    Err(e) => {
                        errors.insert(screen, e.to_string());
                    }
                }
            } else {
                // v2 documents (gui-only screens) use the layout-engine
                // element schema, which lives in jrpg-renderer — just check
                // the structural shape here.
                assert!(
                    val.get("elements").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty()),
                    "{screen}: schema_version 2 must have non-empty elements"
                );
                count += 1;
            }
        }

        if !errors.is_empty() {
            for (file, err) in &errors {
                eprintln!("FAIL {}: {}", file, err);
            }
            panic!("{} layout(s) failed to parse", errors.len());
        }

        assert_eq!(count, SCREENS.len(), "all registry layouts must parse");
    }
}

// ── Registry coverage ─────────────────────────────────────────────────

#[test]
fn get_layout_json_covers_all_files() {
    let screens = [
        "bag", "battle_bag", "battle_main", "battle_move", "battle_party",
        "battle_text", "dialog", "main", "mart", "naming", "oak_speech",
        "options", "party", "pokedex", "pokedex_list", "save", "start",
        "stats", "title", "yes_no",
    ];
    for screen in &screens {
        let json = schema::get_layout_json(screen);
        assert!(
            json.is_some(),
            "get_layout_json(\"{}\") returned None", screen
        );
        let json_str = json.unwrap();
        assert!(
            json_str.contains(&format!("\"screen\": \"{}\"", screen)),
            "JSON for {} doesn't contain screen field", screen
        );
    }
}
