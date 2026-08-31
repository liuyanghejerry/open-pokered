//! Guard: every `openShop([...])` item name used by any map scene must
//! resolve to a real `ItemId` via the script-facing resolver
//! (`ItemId::from_const_name`). The frontend builds the shop stock from
//! these names; an unresolvable name makes the mart silently never open.

use pokered_data::items::ItemId;

/// Extract the string entries of every `openShop([...])` call in a scene.
fn open_shop_item_names(scene: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = scene;
    while let Some(pos) = rest.find("openShop(") {
        rest = &rest[pos + "openShop(".len()..];
        let Some(start) = rest.find('[') else { continue };
        let Some(end) = rest[start..].find(']').map(|e| start + e) else {
            continue;
        };
        for part in rest[start + 1..end].split(',') {
            let name = part.trim().trim_matches('"').trim_matches('\'').trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
        rest = &rest[end..];
    }
    names
}

#[test]
fn all_scene_open_shop_items_resolve() {
    let maps_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("maps");
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&maps_dir).expect("read maps/") {
        let path = entry.expect("dir entry").path().join("script.scene");
        let Ok(scene) = std::fs::read_to_string(&path) else {
            continue;
        };
        for name in open_shop_item_names(&scene) {
            checked += 1;
            if ItemId::from_const_name(&name).is_none() {
                failures.push(format!("{}: '{}'", path.display(), name));
            }
        }
    }
    assert!(checked > 0, "expected to find openShop calls in maps/*/");
    assert!(
        failures.is_empty(),
        "unresolvable openShop item names:\n{}",
        failures.join("\n")
    );
}
