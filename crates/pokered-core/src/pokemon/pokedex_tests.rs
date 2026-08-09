use super::pokedex::*;
use pokered_data::species::Species;

#[test]
fn new_pokedex_is_empty() {
    let dex = Pokedex::new();
    assert_eq!(dex.seen_count(), 0);
    assert_eq!(dex.owned_count(), 0);
    assert!(!dex.is_complete());
}

#[test]
fn set_seen_single() {
    let mut dex = Pokedex::new();
    dex.set_seen(Species::Bulbasaur);
    assert!(dex.is_seen(Species::Bulbasaur));
    assert!(!dex.is_owned(Species::Bulbasaur));
    assert_eq!(dex.seen_count(), 1);
    assert_eq!(dex.owned_count(), 0);
}

#[test]
fn set_owned_also_sets_seen() {
    let mut dex = Pokedex::new();
    dex.set_owned(Species::Pikachu);
    assert!(dex.is_seen(Species::Pikachu));
    assert!(dex.is_owned(Species::Pikachu));
    assert_eq!(dex.seen_count(), 1);
    assert_eq!(dex.owned_count(), 1);
}

#[test]
fn set_seen_then_owned() {
    let mut dex = Pokedex::new();
    dex.set_seen(Species::Charmander);
    dex.set_owned(Species::Charmander);
    assert!(dex.is_seen(Species::Charmander));
    assert!(dex.is_owned(Species::Charmander));
    assert_eq!(dex.seen_count(), 1);
    assert_eq!(dex.owned_count(), 1);
}

#[test]
fn multiple_species() {
    let mut dex = Pokedex::new();
    dex.set_seen(Species::Bulbasaur);
    dex.set_seen(Species::Charmander);
    dex.set_owned(Species::Squirtle);
    assert_eq!(dex.seen_count(), 3);
    assert_eq!(dex.owned_count(), 1);
}

#[test]
fn set_seen_idempotent() {
    let mut dex = Pokedex::new();
    dex.set_seen(Species::Mew);
    dex.set_seen(Species::Mew);
    assert_eq!(dex.seen_count(), 1);
}

#[test]
fn mew_at_151() {
    let mut dex = Pokedex::new();
    dex.set_owned(Species::Mew);
    assert!(dex.is_owned(Species::Mew));
    assert!(dex.is_seen(Species::Mew));
}

#[test]
fn reset_seen() {
    let mut dex = Pokedex::new();
    dex.set_seen(Species::Pidgey);
    assert!(dex.is_seen(Species::Pidgey));
    dex.reset_seen(Species::Pidgey);
    assert!(!dex.is_seen(Species::Pidgey));
    assert_eq!(dex.seen_count(), 0);
}

#[test]
fn reset_owned() {
    let mut dex = Pokedex::new();
    dex.set_owned(Species::Mewtwo);
    dex.reset_owned(Species::Mewtwo);
    assert!(!dex.is_owned(Species::Mewtwo));
    assert!(dex.is_seen(Species::Mewtwo));
    assert_eq!(dex.owned_count(), 0);
}

#[test]
fn flag_bytes_19() {
    let dex = Pokedex::new();
    assert_eq!(dex.seen_flags().len(), 19);
    assert_eq!(dex.owned_flags().len(), 19);
}

#[test]
fn is_complete_all_151() {
    let mut dex = Pokedex::new();
    let all_species = [
        Species::Bulbasaur,
        Species::Ivysaur,
        Species::Venusaur,
        Species::Charmander,
        Species::Charmeleon,
        Species::Charizard,
        Species::Squirtle,
        Species::Wartortle,
        Species::Blastoise,
        Species::Caterpie,
        Species::Metapod,
        Species::Butterfree,
        Species::Weedle,
        Species::Kakuna,
        Species::Beedrill,
        Species::Pidgey,
        Species::Pidgeotto,
        Species::Pidgeot,
        Species::Rattata,
        Species::Raticate,
        Species::Spearow,
        Species::Fearow,
        Species::Ekans,
        Species::Arbok,
        Species::Pikachu,
        Species::Raichu,
        Species::Sandshrew,
        Species::Sandslash,
        Species::NidoranF,
        Species::Nidorina,
        Species::Nidoqueen,
        Species::NidoranM,
        Species::Nidorino,
        Species::Nidoking,
        Species::Clefairy,
        Species::Clefable,
        Species::Vulpix,
        Species::Ninetales,
        Species::Jigglypuff,
        Species::Wigglytuff,
        Species::Zubat,
        Species::Golbat,
        Species::Oddish,
        Species::Gloom,
        Species::Vileplume,
        Species::Paras,
        Species::Parasect,
        Species::Venonat,
        Species::Venomoth,
        Species::Diglett,
        Species::Dugtrio,
        Species::Meowth,
        Species::Persian,
        Species::Psyduck,
        Species::Golduck,
        Species::Mankey,
        Species::Primeape,
        Species::Growlithe,
        Species::Arcanine,
        Species::Poliwag,
        Species::Poliwhirl,
        Species::Poliwrath,
        Species::Abra,
        Species::Kadabra,
        Species::Alakazam,
        Species::Machop,
        Species::Machoke,
        Species::Machamp,
        Species::Bellsprout,
        Species::Weepinbell,
        Species::Victreebel,
        Species::Tentacool,
        Species::Tentacruel,
        Species::Geodude,
        Species::Graveler,
        Species::Golem,
        Species::Ponyta,
        Species::Rapidash,
        Species::Slowpoke,
        Species::Slowbro,
        Species::Magnemite,
        Species::Magneton,
        Species::Farfetchd,
        Species::Doduo,
        Species::Dodrio,
        Species::Seel,
        Species::Dewgong,
        Species::Grimer,
        Species::Muk,
        Species::Shellder,
        Species::Cloyster,
        Species::Gastly,
        Species::Haunter,
        Species::Gengar,
        Species::Onix,
        Species::Drowzee,
        Species::Hypno,
        Species::Krabby,
        Species::Kingler,
        Species::Voltorb,
        Species::Electrode,
        Species::Exeggcute,
        Species::Exeggutor,
        Species::Cubone,
        Species::Marowak,
        Species::Hitmonlee,
        Species::Hitmonchan,
        Species::Lickitung,
        Species::Koffing,
        Species::Weezing,
        Species::Rhyhorn,
        Species::Rhydon,
        Species::Chansey,
        Species::Tangela,
        Species::Kangaskhan,
        Species::Horsea,
        Species::Seadra,
        Species::Goldeen,
        Species::Seaking,
        Species::Staryu,
        Species::Starmie,
        Species::MrMime,
        Species::Scyther,
        Species::Jynx,
        Species::Electabuzz,
        Species::Magmar,
        Species::Pinsir,
        Species::Tauros,
        Species::Magikarp,
        Species::Gyarados,
        Species::Lapras,
        Species::Ditto,
        Species::Eevee,
        Species::Vaporeon,
        Species::Jolteon,
        Species::Flareon,
        Species::Porygon,
        Species::Omanyte,
        Species::Omastar,
        Species::Kabuto,
        Species::Kabutops,
        Species::Aerodactyl,
        Species::Snorlax,
        Species::Articuno,
        Species::Zapdos,
        Species::Moltres,
        Species::Dratini,
        Species::Dragonair,
        Species::Dragonite,
        Species::Mewtwo,
        Species::Mew,
    ];
    assert_eq!(all_species.len(), NUM_POKEMON);
    for &sp in &all_species {
        dex.set_owned(sp);
    }
    assert!(dex.is_complete());
    assert_eq!(dex.owned_count(), 151);
    assert_eq!(dex.seen_count(), 151);
}

#[test]
fn not_complete_at_150() {
    let mut dex = Pokedex::new();
    let species_without_mew = [
        Species::Bulbasaur,
        Species::Ivysaur,
        Species::Venusaur,
        Species::Charmander,
        Species::Charmeleon,
        Species::Charizard,
        Species::Squirtle,
        Species::Wartortle,
        Species::Blastoise,
        Species::Caterpie,
        Species::Metapod,
        Species::Butterfree,
        Species::Weedle,
        Species::Kakuna,
        Species::Beedrill,
        Species::Pidgey,
        Species::Pidgeotto,
        Species::Pidgeot,
        Species::Rattata,
        Species::Raticate,
        Species::Spearow,
        Species::Fearow,
        Species::Ekans,
        Species::Arbok,
        Species::Pikachu,
        Species::Raichu,
        Species::Sandshrew,
        Species::Sandslash,
        Species::NidoranF,
        Species::Nidorina,
        Species::Nidoqueen,
        Species::NidoranM,
        Species::Nidorino,
        Species::Nidoking,
        Species::Clefairy,
        Species::Clefable,
        Species::Vulpix,
        Species::Ninetales,
        Species::Jigglypuff,
        Species::Wigglytuff,
        Species::Zubat,
        Species::Golbat,
        Species::Oddish,
        Species::Gloom,
        Species::Vileplume,
        Species::Paras,
        Species::Parasect,
        Species::Venonat,
        Species::Venomoth,
        Species::Diglett,
        Species::Dugtrio,
        Species::Meowth,
        Species::Persian,
        Species::Psyduck,
        Species::Golduck,
        Species::Mankey,
        Species::Primeape,
        Species::Growlithe,
        Species::Arcanine,
        Species::Poliwag,
        Species::Poliwhirl,
        Species::Poliwrath,
        Species::Abra,
        Species::Kadabra,
        Species::Alakazam,
        Species::Machop,
        Species::Machoke,
        Species::Machamp,
        Species::Bellsprout,
        Species::Weepinbell,
        Species::Victreebel,
        Species::Tentacool,
        Species::Tentacruel,
        Species::Geodude,
        Species::Graveler,
        Species::Golem,
        Species::Ponyta,
        Species::Rapidash,
        Species::Slowpoke,
        Species::Slowbro,
        Species::Magnemite,
        Species::Magneton,
        Species::Farfetchd,
        Species::Doduo,
        Species::Dodrio,
        Species::Seel,
        Species::Dewgong,
        Species::Grimer,
        Species::Muk,
        Species::Shellder,
        Species::Cloyster,
        Species::Gastly,
        Species::Haunter,
        Species::Gengar,
        Species::Onix,
        Species::Drowzee,
        Species::Hypno,
        Species::Krabby,
        Species::Kingler,
        Species::Voltorb,
        Species::Electrode,
        Species::Exeggcute,
        Species::Exeggutor,
        Species::Cubone,
        Species::Marowak,
        Species::Hitmonlee,
        Species::Hitmonchan,
        Species::Lickitung,
        Species::Koffing,
        Species::Weezing,
        Species::Rhyhorn,
        Species::Rhydon,
        Species::Chansey,
        Species::Tangela,
        Species::Kangaskhan,
        Species::Horsea,
        Species::Seadra,
        Species::Goldeen,
        Species::Seaking,
        Species::Staryu,
        Species::Starmie,
        Species::MrMime,
        Species::Scyther,
        Species::Jynx,
        Species::Electabuzz,
        Species::Magmar,
        Species::Pinsir,
        Species::Tauros,
        Species::Magikarp,
        Species::Gyarados,
        Species::Lapras,
        Species::Ditto,
        Species::Eevee,
        Species::Vaporeon,
        Species::Jolteon,
        Species::Flareon,
        Species::Porygon,
        Species::Omanyte,
        Species::Omastar,
        Species::Kabuto,
        Species::Kabutops,
        Species::Aerodactyl,
        Species::Snorlax,
        Species::Articuno,
        Species::Zapdos,
        Species::Moltres,
        Species::Dratini,
        Species::Dragonair,
        Species::Dragonite,
        Species::Mewtwo,
    ];
    assert_eq!(species_without_mew.len(), 150);
    for &sp in &species_without_mew {
        dex.set_owned(sp);
    }
    assert!(!dex.is_complete());
    assert_eq!(dex.owned_count(), 150);
}

#[test]
fn bit_layout_matches_gen1() {
    let mut dex = Pokedex::new();
    dex.set_seen(Species::Bulbasaur);
    assert_eq!(dex.seen_flags()[0] & 0x01, 0x01);

    dex.set_seen(Species::Pikachu);
    assert_eq!(dex.seen_flags()[3] & (1 << 0), 1 << 0);
}

#[test]
fn default_is_new() {
    let dex = Pokedex::default();
    assert_eq!(dex.seen_count(), 0);
    assert_eq!(dex.owned_count(), 0);
}
