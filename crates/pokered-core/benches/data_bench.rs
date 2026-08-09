use criterion::{black_box, criterion_group, criterion_main, Criterion};

use pokered_data::maps::MapId;
use pokered_data::move_data::MoveData;
use pokered_data::moves::MoveId;
use pokered_data::pokemon_data::get_base_stats;
use pokered_data::species::Species;
use pokered_data::trainer_data::{get_base_money, trainer_data, TrainerClass};
use pokered_data::type_chart::TYPE_CHART;
use pokered_data::wild_data::{wild_data_for_map, GameVersion};

fn bench_base_stats_lookup(c: &mut Criterion) {
    c.bench_function("base_stats_pikachu", |b| {
        b.iter(|| get_base_stats(black_box(Species::Pikachu)))
    });

    c.bench_function("base_stats_mewtwo", |b| {
        b.iter(|| get_base_stats(black_box(Species::Mewtwo)))
    });
}

fn bench_base_stats_all(c: &mut Criterion) {
    c.bench_function("base_stats_all_151", |b| {
        b.iter(|| {
            for i in 1..=151u8 {
                let species: Species = unsafe { std::mem::transmute(i) };
                black_box(get_base_stats(species));
            }
        })
    });
}

fn bench_move_data_lookup(c: &mut Criterion) {
    c.bench_function("move_data_thunderbolt", |b| {
        b.iter(|| MoveData::get(black_box(MoveId::Thunderbolt)))
    });

    c.bench_function("move_data_struggle", |b| {
        b.iter(|| MoveData::get(black_box(MoveId::Struggle)))
    });
}

fn bench_type_chart_scan(c: &mut Criterion) {
    c.bench_function("type_chart_full_scan", |b| {
        b.iter(|| {
            let mut count = 0u32;
            for entry in TYPE_CHART.iter() {
                black_box(entry);
                count += 1;
            }
            black_box(count);
        })
    });
}

fn bench_wild_data_lookup(c: &mut Criterion) {
    c.bench_function("wild_data_route1_red", |b| {
        b.iter(|| wild_data_for_map(black_box(MapId::Route1), black_box(GameVersion::Red)))
    });

    c.bench_function("wild_data_mt_moon_blue", |b| {
        b.iter(|| wild_data_for_map(black_box(MapId::MtMoon1F), black_box(GameVersion::Blue)))
    });
}

fn bench_trainer_data_load(c: &mut Criterion) {
    c.bench_function("trainer_data_full_load", |b| {
        b.iter(|| {
            let data = trainer_data();
            black_box(data.len());
        })
    });
}

fn bench_trainer_base_money(c: &mut Criterion) {
    c.bench_function("trainer_base_money_lookup", |b| {
        b.iter(|| get_base_money(black_box(TrainerClass::Giovanni)))
    });
}

criterion_group!(
    benches,
    bench_base_stats_lookup,
    bench_base_stats_all,
    bench_move_data_lookup,
    bench_type_chart_scan,
    bench_wild_data_lookup,
    bench_trainer_data_load,
    bench_trainer_base_money,
);
criterion_main!(benches);
