//! Example demonstrating compile-time rules.ron to Rust code generation.

use dotzuki_rules::rules_ron;
use dotzuki_rules::Ruleset;

fn main() {
    let ruleset: Ruleset = rules_ron!("rules.ron");

    println!("Stats: {:?}", ruleset.stats);
    println!("Types: {:?}", ruleset.types);
    println!("Effects: {}", ruleset.effects.len());
    println!("Type chart entries: {}", ruleset.type_chart.len());
}
