//! Compile-time catalog of Pokémon-specific `game.*` capabilities.
//!
//! Generic movement, rendering, audio, flag, position and localization verbs
//! are owned by dotzuki's `core_host` catalog.

pub const POKERED_SCRIPT_FUNCTIONS: &[&str] = &[
    "animateHealingMachine",
    "choosePartyPokemon",
    "depositDaycare",
    "elevatorMenu",
    "enterHallOfFame",
    "filterBag",
    "getBadgeCount",
    "getCoins",
    "getDaycareCost",
    "getDaycareLevelsGrown",
    "getDaycareMonName",
    "getGameVersion",
    "getMoney",
    "getPartyCount",
    "getPartyMonName",
    "getPlayerFacing",
    "getPokedexOwnedCount",
    "getPokedexSeenCount",
    "getRivalStarter",
    "giveBadge",
    "giveCoins",
    "giveItem",
    "giveMoney",
    "givePokemon",
    "hasBadge",
    "hasCoins",
    "hasItem",
    "hasMoney",
    "isDaycareInUse",
    "linkStart",
    "oldManTutorial",
    "openBillsPC",
    "openItemPC",
    "openNamingScreen",
    "openPC",
    "openSlots",
    "partyMonKnowsHm",
    "playCry",
    "playShipDeparture",
    "replaceTileBlock",
    "setPartyNickname",
    "showDiploma",
    "showPokedexEntry",
    "startBattle",
    "startBattleSet",
    "startWildBattle",
    "takeCoins",
    "takeItem",
    "takeMoney",
    "tradePokemon",
    "withdrawDaycare",
];

pub fn is_pokered_script_function(name: &str) -> bool {
    POKERED_SCRIPT_FUNCTIONS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_sorted_and_unique() {
        assert!(
            POKERED_SCRIPT_FUNCTIONS
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
            "the capability catalog must remain sorted and duplicate-free"
        );
    }
}
