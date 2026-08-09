use pokered_data::type_chart::TYPE_CHART;
use pokered_data::types::{Effectiveness, PokemonType};
use std::collections::HashSet;

#[test]
fn type_chart_has_82_entries() {
    assert_eq!(TYPE_CHART.len(), 82);
}

#[test]
fn no_duplicate_matchups() {
    let mut seen = HashSet::new();
    for matchup in TYPE_CHART.iter() {
        let key = (matchup.attacker, matchup.defender);
        assert!(
            seen.insert(key),
            "Duplicate matchup: {:?} vs {:?}",
            matchup.attacker,
            matchup.defender
        );
    }
}

#[test]
fn no_normal_effectiveness_in_chart() {
    for matchup in TYPE_CHART.iter() {
        assert_ne!(
            matchup.effectiveness,
            Effectiveness::Normal,
            "TYPE_CHART should not contain Normal effectiveness entries: {:?} vs {:?}",
            matchup.attacker,
            matchup.defender
        );
    }
}

#[test]
fn water_beats_fire() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Water
            && m.defender == PokemonType::Fire
            && m.effectiveness == Effectiveness::SuperEffective
    });
    assert!(found, "Water -> Fire should be super effective");
}

#[test]
fn fire_beats_grass() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Fire
            && m.defender == PokemonType::Grass
            && m.effectiveness == Effectiveness::SuperEffective
    });
    assert!(found, "Fire -> Grass should be super effective");
}

#[test]
fn grass_beats_water() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Grass
            && m.defender == PokemonType::Water
            && m.effectiveness == Effectiveness::SuperEffective
    });
    assert!(found, "Grass -> Water should be super effective");
}

#[test]
fn ground_no_effect_on_flying() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Ground
            && m.defender == PokemonType::Flying
            && m.effectiveness == Effectiveness::NoEffect
    });
    assert!(found, "Ground -> Flying should be no effect");
}

#[test]
fn normal_no_effect_on_ghost() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Normal
            && m.defender == PokemonType::Ghost
            && m.effectiveness == Effectiveness::NoEffect
    });
    assert!(found, "Normal -> Ghost should be no effect");
}

#[test]
fn ghost_no_effect_on_normal() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Ghost
            && m.defender == PokemonType::Normal
            && m.effectiveness == Effectiveness::NoEffect
    });
    assert!(found, "Ghost -> Normal should be no effect (Gen 1 bug)");
}

#[test]
fn electric_no_effect_on_ground() {
    let found = TYPE_CHART.iter().any(|m| {
        m.attacker == PokemonType::Electric
            && m.defender == PokemonType::Ground
            && m.effectiveness == Effectiveness::NoEffect
    });
    assert!(found, "Electric -> Ground should be no effect");
}
