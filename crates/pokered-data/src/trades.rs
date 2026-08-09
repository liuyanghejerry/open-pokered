//! In-game NPC trade table — port of `data/events/trades.asm` (`TradeMons`)
//! from the original Red/Blue disassembly.
//!
//! Each `npctrade` entry in the original stores only four fields:
//!
//! ```text
//! MACRO npctrade
//! ; give mon, get mon, dialog id, nickname
//! 	db \1, \2, \3
//! 	dname \4, NAME_LENGTH
//! ENDM
//! ```
//!
//! Everything else about the received Pokémon is *not* per-trade data
//! (verified against `engine/events/in_game_trades.asm` and
//! `engine/pokemon/add_mon.asm`):
//!
//! - **OT name**: always the literal string `<TRAINER>`
//!   (`InGameTrade_TrainerString: dname "<TRAINER>", NAME_LENGTH`, copied by
//!   `InGameTrade_CopyDataToReceivedMon`).
//! - **OT ID**: two random bytes rolled at trade time
//!   (`InGameTrade_PrepareTradeData`: `call Random` → `wTradedEnemyMonOTID`).
//! - **DVs**: random, like any non-wild gift mon — the trade calls
//!   `AddPartyMon` with `wMonDataLocation = $80` (player party), which takes
//!   the `call Random ; generate random IVs` path, NOT the fixed
//!   `ATKDEFDV_TRAINER/SPDSPCDV_TRAINER` ($98/$88) enemy-trainer path.
//! - **Level**: equal to the level of the mon the player gave
//!   (`InGameTrade_DoTrade` copies the selected party mon's level into
//!   `wCurEnemyLevel`, which `AddPartyMon` uses).
//! - **Evolution**: English Red/Blue in-game trades NEVER trigger evolution.
//!   `InGameTrade_CheckForTradeEvo` (`engine/events/evolve_trade.asm`) only
//!   fires when the received mon's name starts with 'G' (GRAVELER) or "SP"
//!   ("SPECTRE", Haunter's early name) — both are Japanese Blue leftovers and
//!   match nothing in the RB table below. The "went and evolved" post-trade
//!   text (`_AfterTrade2Text`) is similarly vestigial.

use crate::species::Species;

/// OT name of every NPC-traded Pokémon (`InGameTrade_TrainerString`).
pub const NPC_TRADE_OT_NAME: &str = "<TRAINER>";

/// Dialogue-set selector (`TRADE_DIALOGSET_*` in `constants/script_constants.asm`).
/// Picks which of the three `TradeTextPointersN` tables the NPC's dialogue uses.
/// Kept for parity; scene scripts own their dialogue text in this rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeDialogSet {
    /// TRADE_DIALOGSET_CASUAL (0) — TradeTextPointers1
    Casual,
    /// TRADE_DIALOGSET_EVOLUTION (1) — TradeTextPointers2; the "went and
    /// evolved" after-trade text is a Japanese Blue leftover (see module docs).
    Evolution,
    /// TRADE_DIALOGSET_HAPPY (2) — TradeTextPointers3
    Happy,
}

/// One `npctrade` entry of the original `TradeMons` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcTrade {
    /// Species the player must give.
    pub give: Species,
    /// Species the player receives.
    pub receive: Species,
    /// Dialogue set the trading NPC uses.
    pub dialog_set: TradeDialogSet,
    /// Fixed nickname of the received mon (`dname`, max NAME_LENGTH-1 = 10 chars).
    pub nickname: &'static str,
    /// The Butterfree↔Beedrill trade exists in the table but no NPC uses it
    /// (`TRADE_FOR_CHIKUCHIKU ; unused` in `constants/script_constants.asm`).
    pub unused: bool,
}

/// The complete `TradeMons` table, in `TRADE_FOR_*` order
/// (`assert_table_length NUM_NPC_TRADES` = 10).
pub static NPC_TRADES: [NpcTrade; 10] = [
    // TRADE_FOR_TERRY
    NpcTrade {
        give: Species::Nidorino,
        receive: Species::Nidorina,
        dialog_set: TradeDialogSet::Casual,
        nickname: "TERRY",
        unused: false,
    },
    // TRADE_FOR_MARCEL
    NpcTrade {
        give: Species::Abra,
        receive: Species::MrMime,
        dialog_set: TradeDialogSet::Casual,
        nickname: "MARCEL",
        unused: false,
    },
    // TRADE_FOR_CHIKUCHIKU (unused in RB)
    NpcTrade {
        give: Species::Butterfree,
        receive: Species::Beedrill,
        dialog_set: TradeDialogSet::Happy,
        nickname: "CHIKUCHIKU",
        unused: true,
    },
    // TRADE_FOR_SAILOR
    NpcTrade {
        give: Species::Ponyta,
        receive: Species::Seel,
        dialog_set: TradeDialogSet::Casual,
        nickname: "SAILOR",
        unused: false,
    },
    // TRADE_FOR_DUX
    NpcTrade {
        give: Species::Spearow,
        receive: Species::Farfetchd,
        dialog_set: TradeDialogSet::Happy,
        nickname: "DUX",
        unused: false,
    },
    // TRADE_FOR_MARC
    NpcTrade {
        give: Species::Slowbro,
        receive: Species::Lickitung,
        dialog_set: TradeDialogSet::Casual,
        nickname: "MARC",
        unused: false,
    },
    // TRADE_FOR_LOLA
    NpcTrade {
        give: Species::Poliwhirl,
        receive: Species::Jynx,
        dialog_set: TradeDialogSet::Evolution,
        nickname: "LOLA",
        unused: false,
    },
    // TRADE_FOR_DORIS
    NpcTrade {
        give: Species::Raichu,
        receive: Species::Electrode,
        dialog_set: TradeDialogSet::Evolution,
        nickname: "DORIS",
        unused: false,
    },
    // TRADE_FOR_CRINKLES
    NpcTrade {
        give: Species::Venonat,
        receive: Species::Tangela,
        dialog_set: TradeDialogSet::Happy,
        nickname: "CRINKLES",
        unused: false,
    },
    // TRADE_FOR_SPOT
    NpcTrade {
        give: Species::NidoranM,
        receive: Species::NidoranF,
        dialog_set: TradeDialogSet::Happy,
        nickname: "SPOT",
        unused: false,
    },
];

/// Look up a trade by its (give, receive) species pair, exactly matching the
/// original keying (`wInGameTradeGiveMonSpecies`/`wInGameTradeReceiveMonSpecies`).
/// All nine used pairs are unique, so this is unambiguous.
pub fn find_npc_trade(give: Species, receive: Species) -> Option<&'static NpcTrade> {
    NPC_TRADES
        .iter()
        .find(|t| t.give == give && t.receive == receive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evos_moves::{get_evos_moves, EvolutionMethod};
    use crate::lang_data::species_name;

    #[test]
    fn table_matches_asm_trade_mons() {
        // data/events/trades.asm — ten npctrade rows, in order.
        let expected: [(Species, Species, &str); 10] = [
            (Species::Nidorino, Species::Nidorina, "TERRY"),
            (Species::Abra, Species::MrMime, "MARCEL"),
            (Species::Butterfree, Species::Beedrill, "CHIKUCHIKU"),
            (Species::Ponyta, Species::Seel, "SAILOR"),
            (Species::Spearow, Species::Farfetchd, "DUX"),
            (Species::Slowbro, Species::Lickitung, "MARC"),
            (Species::Poliwhirl, Species::Jynx, "LOLA"),
            (Species::Raichu, Species::Electrode, "DORIS"),
            (Species::Venonat, Species::Tangela, "CRINKLES"),
            (Species::NidoranM, Species::NidoranF, "SPOT"),
        ];
        assert_eq!(NPC_TRADES.len(), expected.len());
        for (trade, (give, receive, nick)) in NPC_TRADES.iter().zip(expected.iter()) {
            assert_eq!(&trade.give, give);
            assert_eq!(&trade.receive, receive);
            assert_eq!(&trade.nickname, nick);
            assert!(trade.nickname.len() <= 10, "NAME_LENGTH-1");
        }
        // Only the Butterfree↔Beedrill trade is unused.
        assert_eq!(NPC_TRADES.iter().filter(|t| t.unused).count(), 1);
        assert!(NPC_TRADES[2].unused);
    }

    #[test]
    fn find_npc_trade_resolves_every_used_trade() {
        for trade in NPC_TRADES.iter() {
            let found = find_npc_trade(trade.give, trade.receive).expect("pair must resolve");
            assert!(std::ptr::eq(found, trade));
        }
        assert!(find_npc_trade(Species::Pikachu, Species::Raichu).is_none());
    }

    #[test]
    fn rb_trades_never_trigger_trade_evolution() {
        // engine/events/evolve_trade.asm: the post-trade evolution check only
        // fires when the received mon's name starts with 'G' (GRAVELER) or
        // "SP" ("SPECTRE") — Japanese Blue leftovers that match nothing here.
        for trade in NPC_TRADES.iter() {
            let name = species_name(trade.receive, false);
            assert!(!name.starts_with('G'), "{} would false-match", name);
            assert!(!name.starts_with("SP"), "{} would false-match", name);
            // Belt and braces: none of the received species evolves by trade
            // (Graveler/Haunter/Machoke/Kadabra are never received in RB).
            let entry = get_evos_moves(trade.receive).expect("gen-1 species");
            assert!(
                !entry
                    .evolutions
                    .iter()
                    .any(|e| matches!(e, EvolutionMethod::Trade { .. })),
                "{} evolves by trade — RB NPC trades must not",
                name
            );
        }
    }
}
