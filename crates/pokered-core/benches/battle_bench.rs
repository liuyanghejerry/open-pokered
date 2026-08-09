use criterion::{black_box, criterion_group, criterion_main, Criterion};

use pokered_core::battle::damage::{calculate_damage, crit_chance, DamageParams};
use pokered_core::battle::stat_stages::{apply_stage, StatIndex, StatStages};
use pokered_core::battle::types::get_type_effectiveness;

use pokered_data::moves::MoveId;
use pokered_data::types::PokemonType;

fn bench_damage_calculation(c: &mut Criterion) {
    let params = DamageParams {
        attacker_level: 50,
        move_power: 95,
        move_type: PokemonType::Fire,
        move_id: MoveId::Flamethrower,
        attack_stat: 130,
        defense_stat: 100,
        attack_stage: 0,
        defense_stage: 0,
        attacker_type1: PokemonType::Fire,
        attacker_type2: PokemonType::Flying,
        defender_type1: PokemonType::Grass,
        defender_type2: PokemonType::Poison,
        is_critical: false,
        random_value: 217,
        has_reflect_or_light_screen: false,
        is_explode_effect: false,
        attacker_burned: false,
    };

    c.bench_function("damage_calc_stab_super_effective", |b| {
        b.iter(|| calculate_damage(black_box(&params)))
    });
}

fn bench_damage_critical(c: &mut Criterion) {
    let params = DamageParams {
        attacker_level: 100,
        move_power: 120,
        move_type: PokemonType::Normal,
        move_id: MoveId::HyperBeam,
        attack_stat: 300,
        defense_stat: 250,
        attack_stage: 2,
        defense_stage: -1,
        attacker_type1: PokemonType::Normal,
        attacker_type2: PokemonType::Normal,
        defender_type1: PokemonType::Rock,
        defender_type2: PokemonType::Ground,
        is_critical: true,
        random_value: 255,
        has_reflect_or_light_screen: true,
        is_explode_effect: false,
        attacker_burned: false,
    };

    c.bench_function("damage_calc_critical_high_stats", |b| {
        b.iter(|| calculate_damage(black_box(&params)))
    });
}

fn bench_type_effectiveness(c: &mut Criterion) {
    c.bench_function("type_effectiveness_single", |b| {
        b.iter(|| {
            get_type_effectiveness(
                black_box(PokemonType::Water),
                black_box(PokemonType::Fire),
                black_box(PokemonType::Rock),
            )
        })
    });
}

fn bench_type_effectiveness_neutral(c: &mut Criterion) {
    c.bench_function("type_effectiveness_neutral", |b| {
        b.iter(|| {
            get_type_effectiveness(
                black_box(PokemonType::Normal),
                black_box(PokemonType::Normal),
                black_box(PokemonType::Normal),
            )
        })
    });
}

fn bench_stat_stage_apply(c: &mut Criterion) {
    c.bench_function("stat_stage_apply", |b| {
        b.iter(|| apply_stage(black_box(200), black_box(2)))
    });
}

fn bench_stat_stages_modify(c: &mut Criterion) {
    c.bench_function("stat_stages_modify_cycle", |b| {
        b.iter(|| {
            let mut stages = StatStages::default();
            stages.modify(StatIndex::Attack, 2);
            stages.modify(StatIndex::Defense, -1);
            stages.modify(StatIndex::Speed, 1);
            stages.modify(StatIndex::Special, -2);
            black_box(&stages);
        })
    });
}

fn bench_crit_chance(c: &mut Criterion) {
    c.bench_function("crit_chance_normal", |b| {
        b.iter(|| crit_chance(black_box(100), black_box(false), black_box(false)))
    });

    c.bench_function("crit_chance_high_crit_focus_energy_bug", |b| {
        b.iter(|| crit_chance(black_box(130), black_box(true), black_box(true)))
    });
}

criterion_group!(
    benches,
    bench_damage_calculation,
    bench_damage_critical,
    bench_type_effectiveness,
    bench_type_effectiveness_neutral,
    bench_stat_stage_apply,
    bench_stat_stages_modify,
    bench_crit_chance,
);
criterion_main!(benches);
