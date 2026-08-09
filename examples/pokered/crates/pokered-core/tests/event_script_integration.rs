//! Integration tests — Event flags cross-module scenarios.
//!
//! These tests verify that the event flag system works correctly
//! across multi-step game scenarios (e.g., setting flags to gate progression).

use pokered_core::overworld::event_flags::EventFlags;
use pokered_data::event_flags::EventFlag;

#[test]
fn event_flags_gym_badge_progression() {
    let mut flags = EventFlags::new();
    let gym_flags = [
        EventFlag::EVENT_BEAT_BROCK,
        EventFlag::EVENT_BEAT_MISTY,
        EventFlag::EVENT_BEAT_LT_SURGE,
        EventFlag::EVENT_BEAT_ERIKA,
        EventFlag::EVENT_BEAT_KOGA,
        EventFlag::EVENT_BEAT_SABRINA,
        EventFlag::EVENT_BEAT_BLAINE,
    ];

    for flag in &gym_flags {
        assert!(!flags.check(*flag), "flag should not be set initially");
    }

    for (i, flag) in gym_flags.iter().enumerate() {
        flags.set(*flag);
        for (j, check_flag) in gym_flags.iter().enumerate() {
            if j <= i {
                assert!(
                    flags.check(*check_flag),
                    "flag {j} should be set after setting up to {i}"
                );
            } else {
                assert!(!flags.check(*check_flag), "flag {j} should not be set yet");
            }
        }
    }

    assert!(flags.count_set() >= 7, "at least 7 flags should be set");
}

#[test]
fn event_flags_set_reset_idempotent() {
    let mut flags = EventFlags::new();
    let flag = EventFlag::EVENT_GOT_STARTER;

    flags.set(flag);
    flags.set(flag);
    assert!(flags.check(flag));

    flags.reset(flag);
    assert!(!flags.check(flag));
    flags.reset(flag);
    assert!(!flags.check(flag));
}

#[test]
fn event_flags_clear_all_resets_everything() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_BEAT_BROCK);
    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BEAT_MISTY);
    assert!(flags.count_set() >= 3);

    flags.clear_all();
    assert_eq!(flags.count_set(), 0);
    assert!(!flags.check(EventFlag::EVENT_BEAT_BROCK));
    assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
}

#[test]
fn event_flags_serialization_roundtrip() {
    let mut flags = EventFlags::new();
    flags.set(EventFlag::EVENT_BEAT_BROCK);
    flags.set(EventFlag::EVENT_GOT_POKEDEX);
    flags.set(EventFlag::EVENT_BEAT_CHAMPION_RIVAL);

    let bytes = flags.as_bytes();
    let restored = EventFlags::from_bytes(bytes);

    assert!(restored.check(EventFlag::EVENT_BEAT_BROCK));
    assert!(restored.check(EventFlag::EVENT_GOT_POKEDEX));
    assert!(restored.check(EventFlag::EVENT_BEAT_CHAMPION_RIVAL));
    assert!(!restored.check(EventFlag::EVENT_BEAT_MISTY));
    assert_eq!(flags.count_set(), restored.count_set());
}

#[test]
fn event_flags_raw_bit_access_matches_named() {
    let mut flags = EventFlags::new();
    let flag = EventFlag::EVENT_GOT_STARTER;
    let raw_bit = flag.bit_index();

    flags.set(flag);
    assert!(
        flags.check_raw(raw_bit),
        "raw check should match named check"
    );

    flags.reset(flag);
    assert!(!flags.check_raw(raw_bit));

    flags.set_raw(raw_bit);
    assert!(flags.check(flag), "named check should match raw set");
}

#[test]
fn oak_lab_starter_scenario_flag_gating() {
    let mut flags = EventFlags::new();
    assert!(!flags.check(EventFlag::EVENT_GOT_STARTER));
    flags.set(EventFlag::EVENT_GOT_STARTER);
    assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
}

#[test]
fn rival_battle_then_pokedex_scenario() {
    let mut flags = EventFlags::new();

    flags.set(EventFlag::EVENT_GOT_STARTER);
    flags.set(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB);
    flags.set(EventFlag::EVENT_GOT_POKEDEX);

    assert!(flags.check(EventFlag::EVENT_GOT_STARTER));
    assert!(flags.check(EventFlag::EVENT_BATTLED_RIVAL_IN_OAKS_LAB));
    assert!(flags.check(EventFlag::EVENT_GOT_POKEDEX));
}
