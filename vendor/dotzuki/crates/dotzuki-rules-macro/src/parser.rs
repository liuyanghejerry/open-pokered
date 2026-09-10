//! Parser for rules.ron files.
//!
//! This module reads and parses rules.ron files into the intermediate
//! representation that can be used by the code generator.

use serde::Deserialize;

/// A simplified version of the Ruleset struct for parsing.
/// We use this to avoid depending on dotzuki-engine in the proc-macro crate.
#[derive(Debug, Deserialize)]
#[serde(rename = "Ruleset")]
pub struct RulesetInput {
    #[serde(default)]
    pub stats: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub type_chart: Vec<TypeChartEntryInput>,
    #[serde(default)]
    pub effects: Vec<EffectRecordInput>,
}

#[derive(Debug, Deserialize)]
pub struct TypeChartEntryInput {
    pub atk: String,
    pub def: String,
    pub mult: RationalInput,
}

#[derive(Debug)]
pub struct RationalInput {
    pub num: u32,
    pub den: u32,
}

impl<'de> Deserialize<'de> for RationalInput {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v: Vec<u32> = Vec::deserialize(d)?;
        if v.len() != 2 {
            return Err(serde::de::Error::invalid_length(
                v.len(),
                &"a 2-element rational [num, den]",
            ));
        }
        Ok(RationalInput {
            num: v[0],
            den: v[1],
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename = "Effect")]
pub struct EffectRecordInput {
    pub id: String,
    pub kind: EffectKindInput,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub power: Option<u32>,
    #[serde(rename = "type", default)]
    pub mtype: Option<String>,
    #[serde(default)]
    pub accuracy: Option<u32>,
    #[serde(default)]
    pub cost: Vec<ResourceCostInput>,
    #[serde(default)]
    pub hooks: Vec<HookRecordInput>,
}

#[derive(Debug, Deserialize)]
pub enum EffectKindInput {
    Move,
    Status,
    Ability,
    Item,
    Weather,
}

#[derive(Debug, Deserialize)]
pub struct ResourceCostInput {
    pub resource: String,
    pub amount: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "Hook")]
pub struct HookRecordInput {
    pub on: String,
    #[serde(default = "default_order")]
    pub order: u32,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub chance: Option<RationalInput>,
    #[serde(rename = "do", default)]
    pub ops: Vec<OpInput>,
}

fn default_order() -> u32 {
    u32::MAX
}

#[derive(Debug, Deserialize)]
pub enum SelectorInput {
    Target,
    Foe,
    Host,
    Source,
}

impl Default for SelectorInput {
    fn default() -> Self {
        SelectorInput::Target
    }
}

#[derive(Debug, Deserialize)]
pub enum FractionOfInput {
    MaxHp,
    CurHp,
    LastDamage,
}

impl Default for FractionOfInput {
    fn default() -> Self {
        FractionOfInput::MaxHp
    }
}

#[derive(Debug, Deserialize)]
pub enum OpInput {
    DealMoveDamage,
    DamageFraction {
        num: u32,
        den: u32,
        #[serde(default)]
        of: FractionOfInput,
        target: SelectorInput,
        #[serde(default)]
        unless: Option<PredicateInput>,
    },
    HealFraction {
        num: u32,
        den: u32,
        #[serde(default)]
        of: FractionOfInput,
        target: SelectorInput,
        #[serde(default)]
        unless: Option<PredicateInput>,
    },
    InflictStatus {
        status: String,
        target: SelectorInput,
    },
    Boost {
        stat: String,
        stages: i8,
        target: SelectorInput,
    },
    ScaleRelay {
        num: u32,
        den: u32,
        #[serde(default)]
        when: Vec<PredicateInput>,
    },
    SetRelay(i64),
    AddRelay(i64),
    ClampRelay {
        lo: i64,
        hi: i64,
    },
    VetoIf {
        cond: PredicateInput,
        #[serde(default)]
        silent: bool,
    },
    ApplyTypeChart,
    PayResource {
        resource: String,
        amount: u16,
        target: SelectorInput,
    },
    SetHp {
        target: SelectorInput,
        value: u16,
        #[serde(default)]
        when: Vec<PredicateInput>,
    },
    SetDamage {
        value: DamageValueInput,
        #[serde(default)]
        of: SelectorInput,
    },
    DamageCurrentHpFraction {
        num: u32,
        den: u32,
        target: SelectorInput,
    },
    RepeatHits {
        count: HitCountInput,
        target: SelectorInput,
        #[serde(default)]
        final_hit: FinalHitRiderInput,
    },
    RemoveStatus {
        target: SelectorInput,
    },
}

#[derive(Debug, Deserialize)]
pub enum PredicateInput {
    HasType(String),
    StatIs(String),
    RelayIntLt(i64),
    HasVolatile(String),
    MoveTypeIsDefenderType,
    TargetHasStatus(String),
    LevelGE,
    SelfHpBelow { num: u32, den: u32 },
    SourceHasStatus(String),
}

#[derive(Debug, Deserialize)]
pub enum DamageValueInput {
    Const(u16),
    UserLevel,
    RngScaledLevel { num: u32, den: u32 },
}

#[derive(Debug, Deserialize)]
pub enum HitCountInput {
    Fixed(u8),
    TwoToFive,
}

#[derive(Debug, Deserialize)]
pub enum FinalHitRiderInput {
    None,
    OnFinal {
        chance: RationalInput,
        ops: Vec<OpInput>,
    },
}

impl Default for FinalHitRiderInput {
    fn default() -> Self {
        FinalHitRiderInput::None
    }
}

/// Read and parse a rules.ron file.
pub fn parse_rules_ron(path: &str) -> Result<RulesetInput, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let opts =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);

    opts.from_str(&content)
        .map_err(|e| format!("Failed to parse RON: {}", e))
}
