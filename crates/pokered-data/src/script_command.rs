//! Typed Pokémon-specific extension to the generic dotzuki script protocol.

use crate::alloc_prelude::*;
use dotzuki_engine_script::ScriptCommand;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum PokemonScriptCommand {
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
}

impl PokemonScriptCommand {
    pub const fn name(&self) -> &'static str {
        match self {
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
        }
    }

    pub fn into_script_command(self) -> ScriptCommand {
        let name = self.name().to_string();
        let args = match self {
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
            | Self::EnterHallOfFame => vec![],
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
    fn malformed_and_unknown_commands_are_rejected() {
        assert!(PokemonScriptCommand::from_custom("giveCoins", &[json!(70000)]).is_err());
        assert!(PokemonScriptCommand::from_custom("notRegistered", &[]).is_err());
    }
}
