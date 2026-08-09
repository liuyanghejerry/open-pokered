//! Tests for fishing — the pure rod rules (`overworld::fishing`) and the
//! live `OverworldScreen::use_field_item` wiring for OLD/GOOD/SUPER ROD.
//!
//! Asm references: engine/items/item_effects.asm `ItemUseOldRod` /
//! `ItemUseGoodRod` / `ItemUseSuperRod` / `FishingInit` /
//! `IsNextTileShoreOrWater` / `ReadSuperRodData`; data/wild/good_rod.asm;
//! data/wild/super_rod.asm.

use super::fishing::{is_fishing_tile, response_text, roll_rod_response, RodKind, RodResponse};
use super::screen::{OverworldAudioRequest, OverworldScreen};
use super::{Direction, OverworldInput, TransportMode};
use pokered_data::blockset_data;
use pokered_data::impl_traits::PokemonRedData;
use pokered_data::items::ItemId;
use pokered_data::maps::MapId;
use pokered_data::species::Species;
use pokered_data::tilesets::TilesetId;

fn screen_on(map: MapId) -> OverworldScreen<PokemonRedData> {
    OverworldScreen::new(map, None, PokemonRedData)
}

/// A scripted byte stream for `roll_rod_response`; panics when the roll
/// consumes more bytes than supplied (proving exact RNG consumption).
fn scripted(bytes: &[u8]) -> impl FnMut() -> u8 + '_ {
    let mut iter = bytes.iter().copied();
    move || iter.next().expect("roll consumed more random bytes than scripted")
}

// ── IsNextTileShoreOrWater (item_effects.asm:2829-2851) ─────────────

#[test]
fn fishing_tile_requires_water_tileset_and_water_or_shore_ahead() {
    // Water tile $14 on a water tileset.
    assert!(is_fishing_tile(TilesetId::Overworld, 0x14));
    // Eastern shore tiles $32 (usual) / $48 (Safari Zone) also count...
    assert!(is_fishing_tile(TilesetId::Overworld, 0x32));
    assert!(is_fishing_tile(TilesetId::Overworld, 0x48));
    // ...except on the SHIP_PORT (Vermilion Dock) tileset, which skips them.
    assert!(!is_fishing_tile(TilesetId::ShipPort, 0x32));
    assert!(!is_fishing_tile(TilesetId::ShipPort, 0x48));
    assert!(is_fishing_tile(TilesetId::ShipPort, 0x14));
    // Other tiles never count.
    assert!(!is_fishing_tile(TilesetId::Overworld, 0x00));
    assert!(!is_fishing_tile(TilesetId::Overworld, 0x15));
    // Non-water tilesets never fish (WaterTilesets, data/tilesets/
    // water_tilesets.asm: OVERWORLD/FOREST/DOJO/GYM/SHIP/SHIP_PORT/CAVERN/
    // FACILITY/PLATEAU only).
    assert!(!is_fishing_tile(TilesetId::House, 0x14));
    assert!(is_fishing_tile(TilesetId::Cavern, 0x14));
    assert!(is_fishing_tile(TilesetId::Gym, 0x14));
}

// ── ItemUseOldRod (item_effects.asm:1826-1831) ──────────────────────

#[test]
fn old_rod_always_hooks_magikarp_level5_without_rng() {
    // `lb bc, 5, MAGIKARP` — fixed mon, no Random call at all.
    let mut no_bytes = scripted(&[]);
    let response = roll_rod_response(RodKind::Old, MapId::Route1, &mut no_bytes);
    assert_eq!(
        response,
        RodResponse::Bite {
            species: Species::Magikarp,
            level: 5
        }
    );
}

// ── ItemUseGoodRod (item_effects.asm:1833-1857) ─────────────────────

#[test]
fn good_rod_no_bite_on_odd_random_byte() {
    // `call Random; srl a; jr c, .SetBite` — bit 0 set → no bite (50%).
    let mut rng = scripted(&[0b0000_0001]);
    assert_eq!(
        roll_rod_response(RodKind::Good, MapId::PalletTown, &mut rng),
        RodResponse::NoBite
    );
}

#[test]
fn good_rod_picks_from_global_table_with_shifted_low_bits() {
    // `and %11` masks the SHIFTED byte: (r >> 1) & 3 indexes GoodRodMons.
    // r = 0 → index 0 → 10 GOLDEEN (data/wild/good_rod.asm).
    let mut rng = scripted(&[0]);
    assert_eq!(
        roll_rod_response(RodKind::Good, MapId::PalletTown, &mut rng),
        RodResponse::Bite {
            species: Species::Goldeen,
            level: 10
        }
    );
    // r = 2 → index 1 → 10 POLIWAG.
    let mut rng = scripted(&[2]);
    assert_eq!(
        roll_rod_response(RodKind::Good, MapId::PalletTown, &mut rng),
        RodResponse::Bite {
            species: Species::Poliwag,
            level: 10
        }
    );
    // r = 4 or 6 → index 2/3 ≥ table size → `.RandomLoop` redraws.
    let mut rng = scripted(&[4, 2]);
    assert_eq!(
        roll_rod_response(RodKind::Good, MapId::PalletTown, &mut rng),
        RodResponse::Bite {
            species: Species::Poliwag,
            level: 10
        }
    );
}

#[test]
fn good_rod_bite_probability_is_exactly_half() {
    // Feed every possible first byte (with 0 as the redraw byte): 128 no-bite,
    // and of the even bytes 32+32 resolve immediately (Goldeen/Poliwag) while
    // 64 redraw into index 0 (Goldeen) — i.e. exactly 50% no-bite, then a
    // uniform pick between the two table entries.
    let (mut no_bite, mut goldeen, mut poliwag) = (0, 0, 0);
    for r in 0..=255u8 {
        let bytes = [r, 0];
        let mut rng = scripted(&bytes);
        match roll_rod_response(RodKind::Good, MapId::PalletTown, &mut rng) {
            RodResponse::NoBite => no_bite += 1,
            RodResponse::Bite { species, level } => {
                assert_eq!(level, 10);
                match species {
                    Species::Goldeen => goldeen += 1,
                    Species::Poliwag => poliwag += 1,
                    other => panic!("good rod hooked {other:?}, not in GoodRodMons"),
                }
            }
            RodResponse::NothingHere => panic!("good rod has no map table"),
        }
    }
    assert_eq!((no_bite, goldeen, poliwag), (128, 96, 32));
}

// ── ItemUseSuperRod + ReadSuperRodData (item_effects.asm:1861-1865, 2855-2898)

#[test]
fn super_rod_on_map_without_fishing_group_finds_nothing() {
    // Route 1 is not in SuperRodData → e=$2, "nothing here", no RNG drawn.
    let mut no_bytes = scripted(&[]);
    assert_eq!(
        roll_rod_response(RodKind::Super, MapId::Route1, &mut no_bytes),
        RodResponse::NothingHere
    );
}

#[test]
fn super_rod_no_bite_on_odd_random_byte() {
    // `srl a; ret c ; 50% chance of no battle` — bit 0 set → no bite.
    let mut rng = scripted(&[0b1111_1111]);
    assert_eq!(
        roll_rod_response(RodKind::Super, MapId::PalletTown, &mut rng),
        RodResponse::NoBite
    );
}

#[test]
fn super_rod_picks_from_map_group_with_listed_levels() {
    // Pallet Town → .Group1: 15 TENTACOOL / 15 POLIWAG.
    let mut rng = scripted(&[0]);
    assert_eq!(
        roll_rod_response(RodKind::Super, MapId::PalletTown, &mut rng),
        RodResponse::Bite {
            species: Species::Tentacool,
            level: 15
        }
    );
    let mut rng = scripted(&[2]);
    assert_eq!(
        roll_rod_response(RodKind::Super, MapId::PalletTown, &mut rng),
        RodResponse::Bite {
            species: Species::Poliwag,
            level: 15
        }
    );
    // Index 2/3 ≥ group size (2) → redraw.
    let mut rng = scripted(&[6, 2]);
    assert_eq!(
        roll_rod_response(RodKind::Super, MapId::PalletTown, &mut rng),
        RodResponse::Bite {
            species: Species::Poliwag,
            level: 15
        }
    );
}

#[test]
fn super_rod_uses_each_entry_listed_level() {
    // Cerulean Cave 1F → .Group9, all four entries at level 23.
    for (byte, expected) in [
        (0u8, Species::Slowbro),
        (2, Species::Seaking),
        (4, Species::Kingler),
        (6, Species::Seadra),
    ] {
        let bytes = [byte];
        let mut rng = scripted(&bytes);
        assert_eq!(
            roll_rod_response(RodKind::Super, MapId::CeruleanCave1F, &mut rng),
            RodResponse::Bite {
                species: expected,
                level: 23
            }
        );
    }
    // Route 12 → .Group7: index 0 is a level-5 TENTACOOL, the rest L15.
    let mut rng = scripted(&[0]);
    assert_eq!(
        roll_rod_response(RodKind::Super, MapId::Route12, &mut rng),
        RodResponse::Bite {
            species: Species::Tentacool,
            level: 5
        }
    );
}

// ── FishingAnim texts (data/text/text_1.asm:21-33) ──────────────────

#[test]
fn response_texts_match_asm() {
    assert_eq!(response_text(RodResponse::NoBite), "Not even a nibble!");
    assert_eq!(
        response_text(RodResponse::NothingHere),
        "Looks like there's\nnothing here."
    );
    assert_eq!(
        response_text(RodResponse::Bite {
            species: Species::Magikarp,
            level: 5
        }),
        "Oh!\nIt's a bite!"
    );
}

// ── Screen wiring (use_field_item) ──────────────────────────────────

/// Find a block in the Overworld blockset whose player-readable tile equals
/// `tile` (same helper shape as tests_field_moves).
fn find_block_with_tile(tile: u8) -> Option<(u8, u16, u16)> {
    for block in 0u8..=255 {
        let Some(tiles) = blockset_data::block_tiles(TilesetId::Overworld, block) else {
            break;
        };
        for (sub_x, sub_y) in [(0u16, 0u16), (1, 0), (0, 1), (1, 1)] {
            let idx = ((sub_y * 2 + 1) * 4 + sub_x * 2) as usize;
            if tiles[idx] == tile {
                return Some((block, sub_x, sub_y));
            }
        }
    }
    None
}

/// Position the player so the tile in front of them reads sub-tile
/// (sub_x, sub_y) of `block` placed at map block (5,5).
fn face_block(
    screen: &mut OverworldScreen<PokemonRedData>,
    block: u8,
    sub_x: u16,
    sub_y: u16,
) {
    screen
        .map_data
        .as_mut()
        .expect("map_data present")
        .set_block(5, 5, block);
    let front_x = 5u16 * 2 + sub_x;
    let front_y = 5u16 * 2 + sub_y;
    screen.state.player.x = front_x;
    screen.state.player.y = front_y + 1;
    screen.state.player.facing = Direction::Up;
}

fn face_water(screen: &mut OverworldScreen<PokemonRedData>) {
    let (block, sub_x, sub_y) = find_block_with_tile(0x14).expect("blockset has a water block");
    face_block(screen, block, sub_x, sub_y);
}

fn current_dialogue_page(screen: &OverworldScreen<PokemonRedData>) -> Option<String> {
    let page = screen.pending_dialogue.as_ref()?.current()?;
    Some(format!("{}\n{}", page.line1, page.line2))
}

/// Accumulate every dialogue page shown while mashing A, until
/// `pending_wild_encounter` surfaces (or the frame budget runs out).
fn mash_a_until_encounter(screen: &mut OverworldScreen<PokemonRedData>) -> String {
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    let press_a = OverworldInput::new(false, false, false, false, true, false, false, false);
    let mut seen: Vec<String> = Vec::new();
    for i in 0..600 {
        if let Some(page) = current_dialogue_page(screen) {
            if seen.last() != Some(&page) {
                seen.push(page);
            }
        }
        let input = if i % 2 == 0 { press_a } else { neutral };
        screen.update_frame(input);
        if screen.pending_wild_encounter.is_some() {
            break;
        }
    }
    seen.join("\n")
}

#[test]
fn old_rod_facing_water_hooks_magikarp_and_queues_battle_after_text() {
    let mut screen = screen_on(MapId::PalletTown);
    face_water(&mut screen);

    let consumed = screen.use_field_item(ItemId::OldRod, MapId::PalletTown);
    assert!(!consumed, "rods are key items, never consumed");

    // FishingInit success: ItemUseText00 is shown; SFX_HEAL_AILMENT and the
    // 80-frame pause come AFTER the text is dismissed (the SFX must not
    // overlap the text box — item_effects.asm:1906-1911).
    assert!(!screen.audio_requests.iter().any(|r| matches!(
        r,
        OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_HEAL_AILMENT"
    )));
    assert_eq!(
        current_dialogue_page(&screen).as_deref(),
        Some("You used the\nOLD ROD!")
    );

    // The battle is deferred until the text is dismissed.
    assert!(screen.pending_wild_encounter.is_none());

    // Mash A through the two pages; the hooked battle then surfaces.
    let shown_text = mash_a_until_encounter(&mut screen);
    assert!(shown_text.contains("Oh!\nIt's a bite!"), "{shown_text}");
    let encounter = screen
        .pending_wild_encounter
        .take()
        .expect("the hooked battle starts once the text is dismissed");
    assert_eq!(encounter.species, Species::Magikarp);
    assert_eq!(encounter.level, 5, "ItemUseOldRod: lb bc, 5, MAGIKARP");
    assert!(encounter.hooked, "rod battles set the hooked (wMoveMissed) flag");
    assert!(!encounter.old_man);
}

#[test]
fn rod_refused_while_surfing() {
    // FishingInit: `cp 2 ; Surfing?` → carry → ItemUseNotTime.
    let mut screen = screen_on(MapId::PalletTown);
    face_water(&mut screen);
    screen.state.player.transport = TransportMode::Surfing;

    let consumed = screen.use_field_item(ItemId::OldRod, MapId::PalletTown);
    assert!(!consumed);
    assert_eq!(
        current_dialogue_page(&screen).as_deref(),
        Some("This isn't the\ntime to use that!")
    );
    assert!(screen.post_dialogue_battle.is_none());
}

#[test]
fn rod_refused_when_not_facing_water() {
    let mut screen = screen_on(MapId::PalletTown);
    // An inland spot with no water ahead (same position the SURF refusal
    // test uses).
    screen.state.player.x = 5;
    screen.state.player.y = 5;
    screen.state.player.facing = Direction::Up;

    let consumed = screen.use_field_item(ItemId::SuperRod, MapId::PalletTown);
    assert!(!consumed);
    assert_eq!(
        current_dialogue_page(&screen).as_deref(),
        Some("This isn't the\ntime to use that!")
    );
    assert!(screen.post_dialogue_battle.is_none());
}

#[test]
fn good_and_super_rods_facing_water_produce_a_result_message() {
    // Both rods must reach the roll path (exact mon is RNG-driven; the pure
    // tests above pin the rolls byte-for-byte).
    for (item, name) in [(ItemId::GoodRod, "GOOD ROD"), (ItemId::SuperRod, "SUPER ROD")] {
        let mut screen = screen_on(MapId::PalletTown);
        face_water(&mut screen);
        assert!(!screen.use_field_item(item, MapId::PalletTown));
        assert_eq!(
            current_dialogue_page(&screen).as_deref(),
            Some(format!("You used the\n{}!", name)).as_deref()
        );
        let text = mash_a_until_encounter(&mut screen);
        assert!(
            text.contains("Not even a nibble!")
                || text.contains("Oh!\nIt's a bite!")
                || text.contains("Looks like there's\nnothing here."),
            "{name}: {text}"
        );
    }
}

// ── FishingAnim screen wiring (player_animations.asm:378-469) ─────

use super::presentation::FishingAnimPhase;
use super::screen::BedroomDialogue;
use crate::overworld::fishing::PendingFishing;

/// Tick with A pressed every other frame until no dialogue is up. The frame
/// that clears a dialogue returns from `update_frame` immediately, so one
/// extra neutral tick runs any dialogue-close follow-up (the rod animation
/// starts on the frame after the item-use text closes, exactly like the
/// existing `post_dialogue_battle` path).
fn dismiss_dialogue(screen: &mut OverworldScreen<PokemonRedData>) {
    let press_a = OverworldInput::new(false, false, false, false, true, false, false, false);
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    for i in 0..600 {
        if screen.pending_dialogue.is_none() {
            screen.update_frame(neutral);
            return;
        }
        screen.update_frame(if i % 2 == 0 { press_a } else { neutral });
    }
    panic!("dialogue never dismissed");
}

fn tick_frames(screen: &mut OverworldScreen<PokemonRedData>, n: u32) {
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    for _ in 0..n {
        screen.update_frame(neutral);
    }
}

#[test]
fn fishing_anim_plays_between_used_text_and_result_text_and_locks_input() {
    let mut screen = screen_on(MapId::PalletTown);
    face_water(&mut screen);
    screen.use_field_item(ItemId::OldRod, MapId::PalletTown);

    // ItemUseText00 first ("<PLAYER> used <ITEM>!"), response rolled but the
    // animation not yet started (it is deferred until the text closes).
    assert_eq!(
        current_dialogue_page(&screen).as_deref(),
        Some("You used the\nOLD ROD!")
    );
    assert!(screen.pending_fishing.is_some());
    assert!(screen.fishing_anim.is_none());

    // Dismissing the item-use text starts FishingInit's 80-frame pause
    // (SFX_HEAL_AILMENT + DelayFrames(80), item_effects.asm:1908-1911), then
    // the rod animation — not the result.
    dismiss_dialogue(&mut screen);
    assert!(screen.pending_dialogue.is_none());
    assert!(
        screen.fishing_anim.is_none(),
        "the 80-frame pause plays before the cast"
    );
    tick_frames(&mut screen, 80);
    let mut anim = screen
        .fishing_anim
        .expect("the rod animation starts once the pause elapses");
    assert_eq!(anim.phase(), FishingAnimPhase::CastDelay);

    // The animation freezes gameplay: movement input is ignored the whole
    // time (the original's blocking DelayFrames loops read no joypad).
    let pos_before = (screen.state.player.x, screen.state.player.y);
    let up = OverworldInput::new(true, false, false, false, false, false, false, false);
    for _ in 0..20 {
        screen.update_frame(up);
    }
    assert_eq!(
        (screen.state.player.x, screen.state.player.y),
        pos_before,
        "player cannot move while the rod animation plays"
    );
    anim = screen.fishing_anim.expect("animation still running");
    assert!(matches!(
        anim.phase(),
        FishingAnimPhase::RodOut | FishingAnimPhase::Shake
    ));
    assert!(anim.pose_active(), "fishing pose shown while the rod is out");

    // Run the bite choreography to the end (Old Rod always hooks MAGIKARP):
    // the shake and "!" bubble are observable phases, then the result text
    // appears (FishingAnim.done → PrintText ItsABiteText). The anim is at
    // frame ~21 here (1 start tick + 20 locked-movement ticks); the shake
    // spans frames 110..139 and the bubble 140..199.
    tick_frames(&mut screen, 91);
    anim = screen.fishing_anim.expect("shake phase");
    assert_eq!(anim.phase(), FishingAnimPhase::Shake);
    assert!(anim.player_shake_offset() == 0 || anim.player_shake_offset() == 1);
    tick_frames(&mut screen, 30);
    anim = screen.fishing_anim.expect("bubble phase");
    assert_eq!(anim.phase(), FishingAnimPhase::Bubble);
    assert!(anim.bubble_active());
    tick_frames(&mut screen, 60);
    assert!(screen.fishing_anim.is_none(), "animation finished");
    assert_eq!(
        current_dialogue_page(&screen).as_deref(),
        Some("Oh!\nIt's a bite!")
    );
    assert!(screen.pending_fishing.is_none(), "result consumed");

    // The hooked battle still fires only after the result text closes.
    dismiss_dialogue(&mut screen);
    let encounter = screen
        .pending_wild_encounter
        .take()
        .expect("hooked battle starts once the bite text is dismissed");
    assert_eq!(encounter.species, Species::Magikarp);
    assert_eq!(encounter.level, 5);
    assert!(encounter.hooked);
}

#[test]
fn fishing_anim_no_bite_skips_shake_and_bubble() {
    // Deterministic no-bite (bypassing the RNG roll): the animation is
    // 10 + 100 frames and the result text follows directly.
    let mut screen = screen_on(MapId::PalletTown);
    face_water(&mut screen);
    screen.pending_dialogue = Some(BedroomDialogue::from_message("You used the\nGOOD ROD!"));
    screen.pending_fishing = Some(PendingFishing {
        response: RodResponse::NoBite,
    });

    dismiss_dialogue(&mut screen);
    // FishingInit's 80-frame pause runs before FishingAnim (item_effects.asm:
    // 1906-1911) — the cast animation starts after it elapses.
    tick_frames(&mut screen, 80);
    let anim = screen
        .fishing_anim
        .expect("animation starts after the item-use text and cast pause");
    assert_eq!(anim.phase(), FishingAnimPhase::CastDelay);

    // Mid-wait: pose visible, no bubble, no shake.
    tick_frames(&mut screen, 60);
    let anim = screen.fishing_anim.expect("still waiting");
    assert_eq!(anim.phase(), FishingAnimPhase::RodOut);
    assert!(anim.pose_active());
    assert!(!anim.bubble_active());
    assert_eq!(anim.player_shake_offset(), 0);

    // 50 more frames (110 total) → done → "Not even a nibble!".
    tick_frames(&mut screen, 50);
    assert!(screen.fishing_anim.is_none());
    assert!(
        current_dialogue_page(&screen)
            .as_deref()
            .map_or(false, |page| page.starts_with("Not even a nibble!")),
        "no-bite result text after the animation"
    );
    assert!(screen.pending_wild_encounter.is_none(), "no battle without a bite");
}

#[test]
fn fishing_anim_refused_paths_never_start_the_animation() {
    fn assert_rod_refused(screen: &mut OverworldScreen<PokemonRedData>, item: ItemId) {
        assert!(!screen.use_field_item(item, MapId::PalletTown));
        assert_eq!(
            current_dialogue_page(screen).as_deref(),
            Some("This isn't the\ntime to use that!")
        );
        assert!(
            screen.pending_fishing.is_none(),
            "failed FishingInit never defers an animation"
        );
        assert!(screen.fishing_anim.is_none());
    }

    // Surfing refusal (FishingInit: `cp 2 ; Surfing?` → ItemUseNotTime).
    let mut surfing = screen_on(MapId::PalletTown);
    face_water(&mut surfing);
    surfing.state.player.transport = TransportMode::Surfing;
    assert_rod_refused(&mut surfing, ItemId::OldRod);

    // Not facing a shore/water tile (IsNextTileShoreOrWater → carry).
    let mut inland = screen_on(MapId::PalletTown);
    face_water(&mut inland);
    inland.state.player.x = 5;
    inland.state.player.y = 5;
    inland.state.player.facing = Direction::Up;
    assert_rod_refused(&mut inland, ItemId::SuperRod);
}

// ── FishingInit's post-text cast pause (item_effects.asm:1906-1911) ────

#[test]
fn rod_waits_80_frames_before_the_cast_animation() {
    let mut screen = screen_on(MapId::PalletTown);
    face_water(&mut screen);
    screen.use_field_item(ItemId::OldRod, MapId::PalletTown);
    assert!(screen.pending_fishing.is_some());
    assert_eq!(screen.fishing_cast_delay, 0, "pause starts after the text");

    // Dismiss "You used the OLD ROD!" (the shared helper alternates A and
    // neutral, matching the dialogue's hold-open release semantics).
    dismiss_dialogue(&mut screen);
    assert!(screen.pending_dialogue.is_none(), "text dismissed");

    // The SFX + 80-frame pause start once the text closes (dismiss_dialogue's
    // closing neutral tick runs the dialogue-close follow-up, which starts
    // the pause).
    let neutral = OverworldInput::new(false, false, false, false, false, false, false, false);
    assert!(
        screen.audio_requests.iter().any(|r| matches!(
            r,
            OverworldAudioRequest::PlaySound { sound_id } if sound_id == "SFX_HEAL_AILMENT"
        )),
        "SFX_HEAL_AILMENT plays after the text, before the cast"
    );
    assert_eq!(screen.fishing_cast_delay, 80);
    assert!(
        screen.fishing_anim.is_none(),
        "animation waits for the pause to elapse"
    );

    // 80 frames of frozen gameplay (DelayFrames), then FishingAnim starts.
    for frame in (1..=80).rev() {
        screen.update_frame(neutral);
        assert_eq!(
            screen.fishing_cast_delay,
            frame - 1,
            "pause counts down {frame} frames after the text"
        );
        if frame > 1 {
            assert!(
                screen.fishing_anim.is_none(),
                "no anim until the pause elapses"
            );
        }
    }
    assert_eq!(screen.fishing_cast_delay, 0);
    assert!(
        screen.fishing_anim.is_some(),
        "FishingAnim starts when the 80 frames elapse"
    );
}
