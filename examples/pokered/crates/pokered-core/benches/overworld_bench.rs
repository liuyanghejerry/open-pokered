use criterion::{black_box, criterion_group, criterion_main, Criterion};

use pokered_core::battle::wild::{EncounterContext, WildEncounterRandoms};
use pokered_core::overworld::hm_effects::*;
use pokered_core::overworld::special_terrain::*;
use pokered_core::overworld::wild_encounters::*;
use pokered_core::overworld::Direction;

use pokered_data::maps::MapId;
use pokered_data::tilesets::TilesetId;

fn bench_wild_encounter_check(c: &mut Criterion) {
    let randoms = WildEncounterRandoms {
        encounter_roll: 10,
        slot_roll: 75,
    };
    let context = EncounterContext {
        repel_active: false,
        party_lead_level: 50,
    };

    c.bench_function("wild_encounter_route1_grass", |b| {
        b.iter(|| {
            check_wild_encounter(
                black_box(MapId::Route1),
                black_box(TilesetId::Overworld),
                black_box(0x52),
                black_box(pokered_data::wild_data::GameVersion::Red),
                black_box(&randoms),
                black_box(&context),
                black_box(false),
                black_box(false),
                black_box(0),
            )
        })
    });
}

fn bench_wild_encounter_no_data(c: &mut Criterion) {
    let randoms = WildEncounterRandoms {
        encounter_roll: 10,
        slot_roll: 75,
    };
    let context = EncounterContext {
        repel_active: false,
        party_lead_level: 50,
    };

    c.bench_function("wild_encounter_pallet_town_none", |b| {
        b.iter(|| {
            check_wild_encounter(
                black_box(MapId::PalletTown),
                black_box(TilesetId::Overworld),
                black_box(0x00),
                black_box(pokered_data::wild_data::GameVersion::Red),
                black_box(&randoms),
                black_box(&context),
                black_box(false),
                black_box(false),
                black_box(0),
            )
        })
    });
}

fn bench_encounter_type_determination(c: &mut Criterion) {
    c.bench_function("determine_encounter_type_grass", |b| {
        b.iter(|| {
            determine_encounter_type(
                black_box(0x52),
                black_box(TilesetId::Overworld),
                black_box(MapId::Route1),
            )
        })
    });
}

fn bench_should_check_encounter(c: &mut Criterion) {
    c.bench_function("should_check_encounter", |b| {
        b.iter(|| should_check_encounter(black_box(false), black_box(false), black_box(0)))
    });
}

fn bench_use_cut(c: &mut Criterion) {
    let badges = 1 << BIT_CASCADEBADGE;
    c.bench_function("hm_cut_tree", |b| {
        b.iter(|| {
            use_cut(
                black_box(badges),
                black_box(TilesetId::Overworld),
                black_box(0x3D),
                black_box(0x32),
            )
        })
    });
}

fn bench_use_surf(c: &mut Criterion) {
    let badges = 1 << BIT_SOULBADGE;
    c.bench_function("hm_surf_start", |b| {
        b.iter(|| {
            use_surf(
                black_box(badges),
                black_box(TilesetId::Overworld),
                black_box(true),
                black_box(false),
                black_box(false),
                black_box(MapId::Route21),
                black_box(false),
                black_box(0),
                black_box(0),
            )
        })
    });
}

fn bench_ledge_jump(c: &mut Criterion) {
    c.bench_function("ledge_jump_check", |b| {
        b.iter(|| {
            check_ledge_jump(
                black_box(TilesetId::Overworld),
                black_box(Direction::Down),
                black_box(0x2C),
                black_box(0x37),
                black_box(Some(Direction::Down)),
            )
        })
    });
}

fn bench_dark_cave_state(c: &mut Criterion) {
    c.bench_function("dark_cave_enter_check_flash", |b| {
        b.iter(|| {
            let mut state = DarkCaveState::new();
            state.enter_map(black_box(MapId::RockTunnel1F));
            let dark = state.is_dark();
            state.use_flash();
            black_box(dark);
        })
    });
}

fn bench_event_flags(c: &mut Criterion) {
    use pokered_core::overworld::event_flags::EventFlags;
    use pokered_data::event_flags::EventFlag;

    c.bench_function("event_flags_set_check_cycle", |b| {
        b.iter(|| {
            let mut flags = EventFlags::new();
            flags.set(black_box(EventFlag::EVENT_FOLLOWED_OAK_INTO_LAB));
            flags.set(black_box(EventFlag::EVENT_GOT_TOWN_MAP));
            flags.set(black_box(EventFlag::EVENT_PASSED_CASCADEBADGE_CHECK));
            let result = flags.check(black_box(EventFlag::EVENT_GOT_TOWN_MAP));
            black_box(result);
        })
    });
}

criterion_group!(
    benches,
    bench_wild_encounter_check,
    bench_wild_encounter_no_data,
    bench_encounter_type_determination,
    bench_should_check_encounter,
    bench_use_cut,
    bench_use_surf,
    bench_ledge_jump,
    bench_dark_cave_state,
    bench_event_flags,
);
criterion_main!(benches);
