//! Compile-time catalog of Pokémon-specific `game.*` capabilities.
//!
//! Generic movement, rendering, audio, flag, position and localization verbs
//! are owned by dotzuki's `core_host` catalog.

pub const POKERED_SCRIPT_FUNCTIONS: &[&str] = &[
    "acknowledgeFieldText",
    "animateHealingMachine",
    "askFieldYesNo",
    "badgeMenu",
    "bikeShopOffer",
    "billPokemonList",
    "choosePartyPokemon",
    "cyclingGateReturn",
    "depositDaycare",
    "elevatorMenu",
    "enterHallOfFame",
    "fieldCry",
    "fieldCryContinuation",
    "filterBag",
    "finishFieldText",
    "followNpcPath",
    "getBadgeCount",
    "getCoins",
    "getDaycareCost",
    "getDaycareLevelsGrown",
    "getDaycareMonName",
    "getDaycareMonSpecies",
    "getGameVersion",
    "getMoney",
    "getPartyCount",
    "getPartyMonName",
    "getPartyMonSpecies",
    "getPlayerFacing",
    "getPokecenterUsed",
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
    "jigglypuffSong",
    "linkStart",
    "oldManTutorial",
    "openBillsPC",
    "openItemPC",
    "openNamingScreen",
    "openPC",
    "openSlots",
    "partyMonCanRename",
    "partyMonKnowsHm",
    "playCry",
    "playShipDeparture",
    "pokecenterFarewell",
    "pokecenterNurseOffer",
    "pokedexRating",
    "pokemonMenu",
    "printFieldText",
    "printFieldTextParts",
    "printItemFieldText",
    "printTextCry",
    "readingMenu",
    "replaceTileBlock",
    "route22GuardReturn",
    "setPartyNickname",
    "showCoinBox",
    "showDiploma",
    "showItemDialogue",
    "showMoneyBox",
    "showPokedexEntry",
    "silphRocketDeparture",
    "startBattle",
    "startBattleSet",
    "startBattleWithText",
    "startWildBattle",
    "staticEncounter",
    "takeCoins",
    "takeItem",
    "takeMoney",
    "tradePokemon",
    "vendingDelivery",
    "waitMusic",
    "waitSound",
    "walkTogether",
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
