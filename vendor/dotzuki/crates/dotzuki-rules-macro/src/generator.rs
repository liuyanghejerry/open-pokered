//! Code generator for rules.ron to Rust code conversion.
//!
//! This module takes the parsed rules.ron input and generates Rust code
//! that directly constructs the Ruleset struct.

use crate::parser::*;
use proc_macro2::TokenStream;
use quote::quote;

/// Generate Rust code for a Ruleset from a rules.ron file.
pub fn generate_ruleset_code(path: &str) -> Result<TokenStream, String> {
    // Get the directory of the file being compiled
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR not set".to_string())?;

    // Resolve the path relative to the manifest directory
    let full_path = std::path::Path::new(&manifest_dir).join(path);

    let ruleset = parse_rules_ron(&full_path.to_string_lossy())?;
    Ok(generate_ruleset_with_imports(&ruleset))
}

/// Generate Rust code for a Ruleset struct with imports.
fn generate_ruleset_with_imports(ruleset: &RulesetInput) -> TokenStream {
    let stats = generate_string_vec(&ruleset.stats);
    let types = generate_string_vec(&ruleset.types);
    let resources = generate_string_vec(&ruleset.resources);
    let type_chart = generate_type_chart(&ruleset.type_chart);
    let effects = generate_effects(&ruleset.effects);

    quote! {
        {
            use dotzuki_rules::{Ruleset, TypeChartEntry, Rational, EffectRecord, EffectKind, HookRecord, ResourceCost, Op, Predicate, Selector, FractionOf, DamageValue, HitCount, FinalHitRider};

            Ruleset {
                stats: #stats,
                types: #types,
                resources: #resources,
                type_chart: #type_chart,
                effects: #effects,
            }
        }
    }
}

/// Generate code for a vector of strings.
fn generate_string_vec(items: &[String]) -> TokenStream {
    let items = items.iter().map(|s| quote! { #s.to_string() });
    quote! { vec![#(#items),*] }
}

/// Generate code for the type chart.
fn generate_type_chart(entries: &[TypeChartEntryInput]) -> TokenStream {
    let entries = entries.iter().map(|entry| {
        let atk = &entry.atk;
        let def = &entry.def;
        let num = entry.mult.num;
        let den = entry.mult.den;
        quote! {
            TypeChartEntry {
                atk: #atk.to_string(),
                def: #def.to_string(),
                mult: Rational { num: #num, den: #den },
            }
        }
    });
    quote! { vec![#(#entries),*] }
}

/// Generate code for effects.
fn generate_effects(effects: &[EffectRecordInput]) -> TokenStream {
    let effects = effects.iter().map(generate_effect);
    quote! { vec![#(#effects),*] }
}

/// Generate code for a single effect.
fn generate_effect(effect: &EffectRecordInput) -> TokenStream {
    let id = &effect.id;
    let kind = generate_effect_kind(&effect.kind);
    let category = generate_option(&effect.category);
    let power = generate_option_u32(&effect.power);
    let mtype = generate_option(&effect.mtype);
    let accuracy = generate_option_u32(&effect.accuracy);
    let cost = generate_resource_costs(&effect.cost);
    let hooks = generate_hooks(&effect.hooks);

    quote! {
        EffectRecord {
            id: #id.to_string(),
            kind: #kind,
            category: #category,
            power: #power,
            mtype: #mtype,
            accuracy: #accuracy,
            cost: #cost,
            hooks: #hooks,
        }
    }
}

/// Generate code for an EffectKind.
fn generate_effect_kind(kind: &EffectKindInput) -> TokenStream {
    match kind {
        EffectKindInput::Move => quote! { EffectKind::Move },
        EffectKindInput::Status => quote! { EffectKind::Status },
        EffectKindInput::Ability => quote! { EffectKind::Ability },
        EffectKindInput::Item => quote! { EffectKind::Item },
        EffectKindInput::Weather => quote! { EffectKind::Weather },
    }
}

/// Generate code for an Option<String>.
fn generate_option(opt: &Option<String>) -> TokenStream {
    match opt {
        Some(s) => quote! { Some(#s.to_string()) },
        None => quote! { None },
    }
}

/// Generate code for an Option<u32>.
fn generate_option_u32(opt: &Option<u32>) -> TokenStream {
    match opt {
        Some(v) => quote! { Some(#v) },
        None => quote! { None },
    }
}

/// Generate code for resource costs.
fn generate_resource_costs(costs: &[ResourceCostInput]) -> TokenStream {
    let costs = costs.iter().map(|cost| {
        let resource = &cost.resource;
        let amount = cost.amount;
        quote! {
            ResourceCost {
                resource: #resource.to_string(),
                amount: #amount,
            }
        }
    });
    quote! { vec![#(#costs),*] }
}

/// Generate code for hooks.
fn generate_hooks(hooks: &[HookRecordInput]) -> TokenStream {
    let hooks = hooks.iter().map(generate_hook);
    quote! { vec![#(#hooks),*] }
}

/// Generate code for a single hook.
fn generate_hook(hook: &HookRecordInput) -> TokenStream {
    let on = &hook.on;
    let order = hook.order;
    let priority = hook.priority;
    let chance = generate_chance(&hook.chance);
    let ops = generate_ops(&hook.ops);

    quote! {
        HookRecord {
            on: #on.to_string(),
            order: #order,
            priority: #priority,
            chance: #chance,
            ops: #ops,
        }
    }
}

/// Generate code for a chance rational.
fn generate_chance(chance: &Option<RationalInput>) -> TokenStream {
    match chance {
        Some(r) => {
            let num = r.num;
            let den = r.den;
            quote! { Some(Rational { num: #num, den: #den }) }
        }
        None => quote! { None },
    }
}

/// Generate code for ops.
fn generate_ops(ops: &[OpInput]) -> TokenStream {
    let ops = ops.iter().map(generate_op);
    quote! { vec![#(#ops),*] }
}

/// Generate code for a single op.
fn generate_op(op: &OpInput) -> TokenStream {
    match op {
        OpInput::DealMoveDamage => quote! { Op::DealMoveDamage },
        OpInput::DamageFraction {
            num,
            den,
            of,
            target,
            unless,
        } => {
            let of = generate_fraction_of(of);
            let target = generate_selector(target);
            let unless = generate_predicate_option(unless);
            quote! {
                Op::DamageFraction {
                    num: #num,
                    den: #den,
                    of: #of,
                    target: #target,
                    unless: #unless,
                }
            }
        }
        OpInput::HealFraction {
            num,
            den,
            of,
            target,
            unless,
        } => {
            let of = generate_fraction_of(of);
            let target = generate_selector(target);
            let unless = generate_predicate_option(unless);
            quote! {
                Op::HealFraction {
                    num: #num,
                    den: #den,
                    of: #of,
                    target: #target,
                    unless: #unless,
                }
            }
        }
        OpInput::InflictStatus { status, target } => {
            let target = generate_selector(target);
            quote! {
                Op::InflictStatus {
                    status: #status.to_string(),
                    target: #target,
                }
            }
        }
        OpInput::Boost {
            stat,
            stages,
            target,
        } => {
            let target = generate_selector(target);
            quote! {
                Op::Boost {
                    stat: #stat.to_string(),
                    stages: #stages,
                    target: #target,
                }
            }
        }
        OpInput::ScaleRelay { num, den, when } => {
            let when = generate_predicates(when);
            quote! {
                Op::ScaleRelay {
                    num: #num,
                    den: #den,
                    when: #when,
                }
            }
        }
        OpInput::SetRelay(v) => quote! { Op::SetRelay(#v) },
        OpInput::AddRelay(k) => quote! { Op::AddRelay(#k) },
        OpInput::ClampRelay { lo, hi } => quote! {
            Op::ClampRelay {
                lo: #lo,
                hi: #hi,
            }
        },
        OpInput::VetoIf { cond, silent } => {
            let cond = generate_predicate(cond);
            quote! {
                Op::VetoIf {
                    cond: #cond,
                    silent: #silent,
                }
            }
        }
        OpInput::ApplyTypeChart => quote! { Op::ApplyTypeChart },
        OpInput::PayResource {
            resource,
            amount,
            target,
        } => {
            let target = generate_selector(target);
            quote! {
                Op::PayResource {
                    resource: #resource.to_string(),
                    amount: #amount,
                    target: #target,
                }
            }
        }
        OpInput::SetHp {
            target,
            value,
            when,
        } => {
            let target = generate_selector(target);
            let when = generate_predicates(when);
            quote! {
                Op::SetHp {
                    target: #target,
                    value: #value,
                    when: #when,
                }
            }
        }
        OpInput::SetDamage { value, of } => {
            let value = generate_damage_value(value);
            let of = generate_selector(of);
            quote! {
                Op::SetDamage {
                    value: #value,
                    of: #of,
                }
            }
        }
        OpInput::DamageCurrentHpFraction { num, den, target } => {
            let target = generate_selector(target);
            quote! {
                Op::DamageCurrentHpFraction {
                    num: #num,
                    den: #den,
                    target: #target,
                }
            }
        }
        OpInput::RepeatHits {
            count,
            target,
            final_hit,
        } => {
            let count = generate_hit_count(count);
            let target = generate_selector(target);
            let final_hit = generate_final_hit_rider(final_hit);
            quote! {
                Op::RepeatHits {
                    count: #count,
                    target: #target,
                    final_hit: #final_hit,
                }
            }
        }
        OpInput::RemoveStatus { target } => {
            let target = generate_selector(target);
            quote! {
                Op::RemoveStatus {
                    target: #target,
                }
            }
        }
    }
}

/// Generate code for a FractionOf.
fn generate_fraction_of(of: &FractionOfInput) -> TokenStream {
    match of {
        FractionOfInput::MaxHp => quote! { FractionOf::MaxHp },
        FractionOfInput::CurHp => quote! { FractionOf::CurHp },
        FractionOfInput::LastDamage => quote! { FractionOf::LastDamage },
    }
}

/// Generate code for a Selector.
fn generate_selector(sel: &SelectorInput) -> TokenStream {
    match sel {
        SelectorInput::Target => quote! { Selector::Target },
        SelectorInput::Foe => quote! { Selector::Foe },
        SelectorInput::Host => quote! { Selector::Host },
        SelectorInput::Source => quote! { Selector::Source },
    }
}

/// Generate code for a Predicate.
fn generate_predicate(pred: &PredicateInput) -> TokenStream {
    match pred {
        PredicateInput::HasType(t) => quote! { Predicate::HasType(#t.to_string()) },
        PredicateInput::StatIs(s) => quote! { Predicate::StatIs(#s.to_string()) },
        PredicateInput::RelayIntLt(n) => quote! { Predicate::RelayIntLt(#n) },
        PredicateInput::HasVolatile(name) => quote! { Predicate::HasVolatile(#name.to_string()) },
        PredicateInput::MoveTypeIsDefenderType => quote! { Predicate::MoveTypeIsDefenderType },
        PredicateInput::TargetHasStatus(s) => quote! { Predicate::TargetHasStatus(#s.to_string()) },
        PredicateInput::LevelGE => quote! { Predicate::LevelGE },
        PredicateInput::SelfHpBelow { num, den } => quote! {
            Predicate::SelfHpBelow {
                num: #num,
                den: #den,
            }
        },
        PredicateInput::SourceHasStatus(s) => quote! { Predicate::SourceHasStatus(#s.to_string()) },
    }
}

/// Generate code for an optional Predicate.
fn generate_predicate_option(pred: &Option<PredicateInput>) -> TokenStream {
    match pred {
        Some(p) => {
            let p = generate_predicate(p);
            quote! { Some(#p) }
        }
        None => quote! { None },
    }
}

/// Generate code for a vector of Predicates.
fn generate_predicates(preds: &[PredicateInput]) -> TokenStream {
    let preds = preds.iter().map(generate_predicate);
    quote! { vec![#(#preds),*] }
}

/// Generate code for a DamageValue.
fn generate_damage_value(value: &DamageValueInput) -> TokenStream {
    match value {
        DamageValueInput::Const(v) => quote! { DamageValue::Const(#v) },
        DamageValueInput::UserLevel => quote! { DamageValue::UserLevel },
        DamageValueInput::RngScaledLevel { num, den } => quote! {
            DamageValue::RngScaledLevel {
                num: #num,
                den: #den,
            }
        },
    }
}

/// Generate code for a HitCount.
fn generate_hit_count(count: &HitCountInput) -> TokenStream {
    match count {
        HitCountInput::Fixed(k) => quote! { HitCount::Fixed(#k) },
        HitCountInput::TwoToFive => quote! { HitCount::TwoToFive },
    }
}

/// Generate code for a FinalHitRider.
fn generate_final_hit_rider(rider: &FinalHitRiderInput) -> TokenStream {
    match rider {
        FinalHitRiderInput::None => quote! { FinalHitRider::None },
        FinalHitRiderInput::OnFinal { chance, ops } => {
            let num = chance.num;
            let den = chance.den;
            let ops = generate_ops(ops);
            quote! {
                FinalHitRider::OnFinal {
                    chance: Rational { num: #num, den: #den },
                    ops: #ops,
                }
            }
        }
    }
}
