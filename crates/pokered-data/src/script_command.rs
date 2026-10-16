//! Typed Pokémon-specific extension to the generic dotzuki script protocol.

use crate::alloc_prelude::*;
use dotzuki_engine_script::ScriptCommand;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum PokemonScriptCommand {
    /// Inner PrintText with DONE: return after printing, without an outer A/B wait.
    PrintFieldText { text: String, retain_window: bool },
    ShowItemDialogue { text: String, sound_id: Option<String> },
    OldManTutorial,
    TradePokemon {
        offered: String,
        received: String,
        nickname: String,
    },
    AnimateHealingMachine,
    ShowPokedexEntry {
        species: String,
    },
    OpenNamingScreen {
        species: String,
    },
    ChoosePartyPokemon,
    SetPartyNickname {
        index: u8,
        nickname: String,
    },
    StartBattleSet {
        trainer_id: String,
        rival_triplet_base: u8,
    },
    OpenSlots {
        lucky: Option<bool>,
    },
    ElevatorMenu {
        floors: Vec<String>,
    },
    FilterBag {
        item_ids: Vec<String>,
    },
    ShowDiploma,
    OpenPc,
    OpenItemPc,
    OpenBillsPc,
    LinkStart,
    GiveCoins {
        amount: u16,
    },
    TakeCoins {
        amount: u16,
    },
    DepositDaycare {
        index: u8,
    },
    WithdrawDaycare,
    ReplaceTileBlock {
        x: u8,
        y: u8,
        block_id: u8,
    },
    PlayShipDeparture,
    EnterHallOfFame,
    WaitMusic,
    WaitSound,
    ReadingMenu { options: Vec<String>, texts: Vec<String> },
    PokemonMenu { options: Vec<String>, species: Vec<String> },
    ShowMoneyBox { amount: i64 },
    ShowCoinBox { amount: i64 },
    VendingDelivery,
    /// Escort one tile behind an explicit NPC route, including backtracking.
    FollowNpcPath { npc_id: String, path: Vec<(u8, u8)> },
    /// Independent synchronized routes; a zero player delta is one field-loop pause.
    WalkTogether { npc_id: String, path: Vec<(u8, u8)>, player_steps: Vec<(i8, i8)> },
}

impl PokemonScriptCommand {
    pub const fn name(&self) -> &'static str {
        match self {
            Self::PrintFieldText { .. } => "printFieldText",
            Self::ShowItemDialogue { .. } => "showItemDialogue",
            Self::OldManTutorial => "oldManTutorial",
            Self::TradePokemon { .. } => "tradePokemon",
            Self::AnimateHealingMachine => "animateHealingMachine",
            Self::ShowPokedexEntry { .. } => "showPokedexEntry",
            Self::OpenNamingScreen { .. } => "openNamingScreen",
            Self::ChoosePartyPokemon => "choosePartyPokemon",
            Self::SetPartyNickname { .. } => "setPartyNickname",
            Self::StartBattleSet { .. } => "startBattleSet",
            Self::OpenSlots { .. } => "openSlots",
            Self::ElevatorMenu { .. } => "elevatorMenu",
            Self::FilterBag { .. } => "filterBag",
            Self::ShowDiploma => "showDiploma",
            Self::OpenPc => "openPC",
            Self::OpenItemPc => "openItemPC",
            Self::OpenBillsPc => "openBillsPC",
            Self::LinkStart => "linkStart",
            Self::GiveCoins { .. } => "giveCoins",
            Self::TakeCoins { .. } => "takeCoins",
            Self::DepositDaycare { .. } => "depositDaycare",
            Self::WithdrawDaycare => "withdrawDaycare",
            Self::ReplaceTileBlock { .. } => "replaceTileBlock",
            Self::PlayShipDeparture => "playShipDeparture",
            Self::EnterHallOfFame => "enterHallOfFame",
            Self::WaitMusic => "waitMusic",
            Self::WaitSound => "waitSound",
            Self::ReadingMenu { .. } => "readingMenu",
            Self::PokemonMenu { .. } => "pokemonMenu",
            Self::ShowMoneyBox { .. } => "showMoneyBox",
            Self::ShowCoinBox { .. } => "showCoinBox",
            Self::VendingDelivery => "vendingDelivery",
            Self::FollowNpcPath { .. } => "followNpcPath",
            Self::WalkTogether { .. } => "walkTogether",
        }
    }

    pub fn into_script_command(self) -> ScriptCommand {
        let name = self.name().to_string();
        let args = match self {
            Self::PrintFieldText { text, retain_window } => {
                let mut args = vec![json!(text)];
                if retain_window { args.push(json!(true)); }
                args
            },
            Self::FollowNpcPath { npc_id, path } => vec![json!(npc_id), json!(path)],
            Self::WalkTogether { npc_id, path, player_steps } => vec![json!(npc_id), json!(path), json!(player_steps)],
            Self::ShowItemDialogue { text, sound_id } => {
                let mut args = vec![json!(text)];
                if let Some(sound) = sound_id { args.push(json!(sound)); }
                args
            },
            Self::ReadingMenu { options, texts } => vec![json!(options), json!(texts)],
            Self::PokemonMenu { options, species } => vec![json!(options), json!(species)],
            Self::ShowMoneyBox { amount } => vec![json!(amount)],
            Self::ShowCoinBox { amount } => vec![json!(amount)],
            Self::TradePokemon {
                offered,
                received,
                nickname,
            } => {
                vec![json!(offered), json!(received), json!(nickname)]
            }
            Self::ShowPokedexEntry { species } | Self::OpenNamingScreen { species } => {
                vec![json!(species)]
            }
            Self::SetPartyNickname { index, nickname } => vec![json!(index), json!(nickname)],
            Self::StartBattleSet {
                trainer_id,
                rival_triplet_base,
            } => {
                vec![json!(trainer_id), json!(rival_triplet_base)]
            }
            Self::OpenSlots { lucky } => vec![json!(lucky)],
            Self::ElevatorMenu { floors } => vec![json!(floors)],
            Self::FilterBag { item_ids } => vec![json!(item_ids)],
            Self::GiveCoins { amount } | Self::TakeCoins { amount } => vec![json!(amount)],
            Self::DepositDaycare { index } => vec![json!(index)],
            Self::ReplaceTileBlock { x, y, block_id } => vec![json!(x), json!(y), json!(block_id)],
            Self::OldManTutorial
            | Self::AnimateHealingMachine
            | Self::ChoosePartyPokemon
            | Self::ShowDiploma
            | Self::OpenPc
            | Self::OpenItemPc
            | Self::OpenBillsPc
            | Self::LinkStart
            | Self::WithdrawDaycare
            | Self::PlayShipDeparture
            | Self::EnterHallOfFame
            | Self::WaitMusic
            | Self::WaitSound
            | Self::VendingDelivery => vec![],
        };
        ScriptCommand::Custom { name, args }
    }

    pub fn from_custom(name: &str, args: &[Value]) -> Result<Self, String> {
        let string = |index: usize| {
            args.get(index)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("{name}: argument {index} must be a string"))
        };
        let u8_arg = |index: usize| {
            args.get(index)
                .and_then(Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| format!("{name}: argument {index} must be a u8"))
        };
        let u16_arg = |index: usize| {
            args.get(index)
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .ok_or_else(|| format!("{name}: argument {index} must be a u16"))
        };
        let strings = |index: usize| {
            args.get(index)
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{name}: argument {index} must be a string array"))?
                .iter()
                .enumerate()
                .map(|(item, value)| {
                    value
                        .as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("{name}: argument {index}[{item}] must be a string"))
                })
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match name {
            "printFieldText" => Self::PrintFieldText {
                text: string(0)?,
                retain_window: match args.get(1) {
                    None => false,
                    Some(Value::Bool(retain)) => *retain,
                    Some(_) => return Err("printFieldText: retainWindow must be a boolean".into()),
                },
            },
            "waitMusic" => Self::WaitMusic,
            "waitSound" => Self::WaitSound,
            "vendingDelivery" => Self::VendingDelivery,
            "walkTogether" => {
                let path: Vec<(u8, u8)> = serde_json::from_value(args.get(1).cloned().ok_or("walkTogether: missing NPC path")?)
                    .map_err(|error| format!("walkTogether: invalid NPC path: {error}"))?;
                let player_steps: Vec<(i8, i8)> = serde_json::from_value(args.get(2).cloned().ok_or("walkTogether: missing player steps")?)
                    .map_err(|error| format!("walkTogether: invalid player steps: {error}"))?;
                if path.is_empty() || player_steps.is_empty() { return Err("walkTogether: empty route".into()); }
                if player_steps.iter().any(|&(x,y)| i16::from(x).abs()+i16::from(y).abs()>1) {
                    return Err("walkTogether: player steps must be cardinal unit deltas or pauses".into());
                }
                Self::WalkTogether { npc_id: string(0)?, path, player_steps }
            },
            "followNpcPath" => {
                let path: Vec<(u8, u8)> = serde_json::from_value(args.get(1).cloned().ok_or("followNpcPath: missing path")?)
                    .map_err(|error| format!("followNpcPath: invalid path: {error}"))?;
                if path.is_empty() { return Err("followNpcPath: empty path".into()); }
                Self::FollowNpcPath { npc_id: string(0)?, path }
            },
            "showMoneyBox" => Self::ShowMoneyBox { amount: args.first().and_then(Value::as_i64).ok_or_else(|| format!("{name}: amount must be an integer"))? },
            "showCoinBox" => Self::ShowCoinBox { amount: args.first().and_then(Value::as_i64).ok_or_else(|| format!("{name}: amount must be an integer"))? },
            "badgeMenu" => {
                let options = strings(0)?;
                let texts = strings(1)?;
                let owned = args.get(2).and_then(Value::as_array).ok_or("badgeMenu: ownership array required")?;
                if options.len() != 9 || texts.len() != 8 || owned.len() != 8 {
                    return Err("badgeMenu: eight badge names, descriptions and ownership bits plus CANCEL required".into());
                }
                let mut filtered_options = Vec::new();
                let mut filtered_texts = Vec::new();
                for (i, bit) in owned.iter().enumerate() {
                    if bit.as_bool().ok_or("badgeMenu: ownership bits must be booleans")? {
                        filtered_options.push(options[i].clone());
                        filtered_texts.push(texts[i].clone());
                    }
                }
                filtered_options.push(options[8].clone());
                Self::ReadingMenu { options: filtered_options, texts: filtered_texts }
            },
            "pokemonMenu" => {
                let options = strings(0)?;
                let species = strings(1)?;
                if options.len() != species.len() + 1 || species.is_empty() {
                    return Err("pokemonMenu: one species per heading plus CANCEL required".into());
                }
                for name in &species {
                    if name.parse::<crate::species::Species>().is_err() {
                        return Err(format!("pokemonMenu: unknown species {name}"));
                    }
                }
                Self::PokemonMenu { options, species }
            },
            "readingMenu" => {
                let options = strings(0)?;
                let texts = strings(1)?;
                if options.len() != texts.len() + 1 {
                    return Err("readingMenu: one text per heading plus a final exit option required".to_string());
                }
                Self::ReadingMenu { options, texts }
            },
            "showItemDialogue" => Self::ShowItemDialogue { text: string(0)?, sound_id: if args.len() > 1 { Some(string(1)?) } else { None } },
            "oldManTutorial" => Self::OldManTutorial,
            "tradePokemon" => Self::TradePokemon {
                offered: string(0)?,
                received: string(1)?,
                nickname: string(2)?,
            },
            "animateHealingMachine" => Self::AnimateHealingMachine,
            "showPokedexEntry" => Self::ShowPokedexEntry {
                species: string(0)?,
            },
            "openNamingScreen" => Self::OpenNamingScreen {
                species: string(0)?,
            },
            "choosePartyPokemon" => Self::ChoosePartyPokemon,
            "setPartyNickname" => Self::SetPartyNickname {
                index: u8_arg(0)?,
                nickname: string(1)?,
            },
            "startBattleSet" => Self::StartBattleSet {
                trainer_id: string(0)?,
                rival_triplet_base: u8_arg(1)?,
            },
            "openSlots" => Self::OpenSlots {
                lucky: args.first().and_then(Value::as_bool),
            },
            "elevatorMenu" => Self::ElevatorMenu {
                floors: strings(0)?,
            },
            "filterBag" => Self::FilterBag {
                item_ids: strings(0)?,
            },
            "showDiploma" => Self::ShowDiploma,
            "openPC" => Self::OpenPc,
            "openItemPC" => Self::OpenItemPc,
            "openBillsPC" => Self::OpenBillsPc,
            "linkStart" => Self::LinkStart,
            "giveCoins" => Self::GiveCoins {
                amount: u16_arg(0)?,
            },
            "takeCoins" => Self::TakeCoins {
                amount: u16_arg(0)?,
            },
            "depositDaycare" => Self::DepositDaycare { index: u8_arg(0)? },
            "withdrawDaycare" => Self::WithdrawDaycare,
            "replaceTileBlock" => Self::ReplaceTileBlock {
                x: u8_arg(0)?,
                y: u8_arg(1)?,
                block_id: u8_arg(2)?,
            },
            "playShipDeparture" => Self::PlayShipDeparture,
            "enterHallOfFame" => Self::EnterHallOfFame,
            _ => return Err(format!("unsupported Pokémon script command: {name}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_command_round_trips_through_protocol() {
        let expected = PokemonScriptCommand::TradePokemon {
            offered: "NIDORAN_M".to_string(),
            received: "NIDORAN_F".to_string(),
            nickname: "TERRY".to_string(),
        };
        let ScriptCommand::Custom { name, args } = expected.clone().into_script_command() else {
            panic!("typed extension must use ScriptCommand::Custom");
        };
        assert_eq!(
            PokemonScriptCommand::from_custom(&name, &args),
            Ok(expected)
        );
    }

    #[test]
    fn synchronized_routes_preserve_pauses_and_reject_non_unit_player_steps() {
        let args = vec![json!("GUIDE"), json!([[35,17], [35,18]]),
            json!([[0,0], [0,0], [-1,0], [0,1]])];
        let expected = PokemonScriptCommand::WalkTogether {
            npc_id: "GUIDE".into(), path: vec![(35,17),(35,18)],
            player_steps: vec![(0,0),(0,0),(-1,0),(0,1)],
        };
        assert_eq!(PokemonScriptCommand::from_custom("walkTogether", &args), Ok(expected.clone()));
        let ScriptCommand::Custom { name, args } = expected.into_script_command() else { panic!("custom command required"); };
        assert_eq!(PokemonScriptCommand::from_custom(&name, &args).unwrap().name(), "walkTogether");
        for steps in [json!([]), json!([[1,1]]), json!([[2,0]]), json!([[0,0.5]]), json!([[0,128]])] {
            assert!(PokemonScriptCommand::from_custom("walkTogether",
                &[json!("GUIDE"), json!([[35,17]]), steps]).is_err());
        }
    }

    #[test]
    fn explicit_escort_rejects_missing_or_invalid_routes() {
        for args in [vec![json!("OAK")], vec![json!("OAK"), json!([])],
            vec![json!("OAK"), json!([[12]])], vec![json!("OAK"), json!([[256,12]])]] {
            assert!(PokemonScriptCommand::from_custom("followNpcPath", &args).is_err());
        }
    }

    #[test]
    fn malformed_and_unknown_commands_are_rejected() {
        assert!(PokemonScriptCommand::from_custom("giveCoins", &[json!(70000)]).is_err());
        assert!(PokemonScriptCommand::from_custom("notRegistered", &[]).is_err());
    }

    #[test]
    fn field_text_window_retention_is_explicit_and_old_commands_default_false() {
        assert_eq!(PokemonScriptCommand::from_custom("printFieldText", &[json!("old")]).unwrap(),
            PokemonScriptCommand::PrintFieldText { text: "old".into(), retain_window: false });
        let retained = PokemonScriptCommand::PrintFieldText { text: "mother".into(), retain_window: true };
        let ScriptCommand::Custom { name, args } = retained.clone().into_script_command() else { panic!() };
        assert_eq!(PokemonScriptCommand::from_custom(&name, &args).unwrap(), retained);
        assert!(PokemonScriptCommand::from_custom("printFieldText", &[json!("x"), json!(1)]).is_err());
        assert_eq!(PokemonScriptCommand::from_custom("waitSound", &[]).unwrap(), PokemonScriptCommand::WaitSound);
    }}
