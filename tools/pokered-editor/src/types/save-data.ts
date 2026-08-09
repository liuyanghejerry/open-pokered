export interface PokemonEntry {
  species: string
  level: number
  currentHp: number
  maxHp: number
  moves: string[]
  nickname: string
}

export interface ItemEntry {
  name: string
  quantity: number
}

export interface PlayerInfo {
  playerName: string
  rivalName: string
  mapName: string
  positionX: number
  positionY: number
  facing: string
  playTimeHours: number
  playTimeMinutes: number
  money: number
}

export interface SaveDataSnapshot {
  player: PlayerInfo
  badges: boolean[]
  party: PokemonEntry[]
  items: ItemEntry[]
  flags: Record<string, boolean>
}

export const COMMON_SPECIES = [
  'Bulbasaur', 'Ivysaur', 'Venusaur',
  'Charmander', 'Charmeleon', 'Charizard',
  'Squirtle', 'Wartortle', 'Blastoise',
  'Pikachu', 'Raichu',
  'Jigglypuff', 'Wigglytuff',
  'Meowth', 'Persian',
  'Abra', 'Kadabra', 'Alakazam',
  'Machop', 'Machoke', 'Machamp',
  'Geodude', 'Graveler', 'Golem',
  'Gastly', 'Haunter', 'Gengar',
  'Onix',
  'Drowzee', 'Hypno',
  'Voltorb', 'Electrode',
  'Cubone', 'Marowak',
  'Rhyhorn', 'Rhydon',
  'Magikarp', 'Gyarados',
  'Lapras',
  'Eevee', 'Vaporeon', 'Jolteon', 'Flareon',
  'Porygon',
  'Snorlax',
  'Articuno', 'Zapdos', 'Moltres',
  'Dratini', 'Dragonair', 'Dragonite',
  'Mewtwo', 'Mew',
]

export const COMMON_ITEMS = [
  'POTION', 'SUPER_POTION', 'HYPER_POTION', 'MAX_POTION',
  'FULL_RESTORE', 'REVIVE',
  'ANTIDOTE', 'BURN_HEAL', 'ICE_HEAL', 'AWAKENING', 'PARLYZ_HEAL', 'FULL_HEAL',
  'POKE_BALL', 'GREAT_BALL', 'ULTRA_BALL', 'MASTER_BALL',
  'RARE_CANDY',
  'ESCAPE_ROPE', 'REPEL', 'SUPER_REPEL', 'MAX_REPEL',
  'BICYCLE', 'TOWN_MAP',
  'S_S__TICKET', 'POKE_DOLL',
  'FRESH_WATER', 'SODA_POP', 'LEMONADE',
  'X_ACCURACY', 'X_ATTACK', 'X_DEFEND', 'X_SPEED', 'X_SPECIAL',
  'DIRE_HIT', 'GUARD_SPEC_',
  'TM01', 'TM02', 'TM03', 'TM05', 'TM06',
  'HM01', 'HM02', 'HM03', 'HM04', 'HM05',
  'NUGGET',
  'HELIX_FOSSIL', 'DOME_FOSSIL', 'OLD_AMBER',
  'SILPH_SCOPE', 'POKE_FLUTE',
  'CARD_KEY', 'GOLD_TEETH',
  'OAKS_PARCEL',
  'MOON_STONE', 'FIRE_STONE', 'THUNDER_STONE', 'WATER_STONE', 'LEAF_STONE',
  'ELIXER', 'MAX_ELIXER', 'ETHER', 'MAX_ETHER',
  'HP_UP', 'PROTEIN', 'IRON', 'CARBOS', 'CALCIUM',
  'PP_UP',
  'EXP__ALL',
]

export const COMMON_FLAGS = [
  'EVENT_GOT_STARTER',
  'EVENT_GOT_POKEDEX',
  'EVENT_GOT_TOWN_MAP',
  'EVENT_BEAT_BROCK',
  'EVENT_BEAT_MISTY',
  'EVENT_BEAT_LT_SURGE',
  'EVENT_BEAT_ERIKA',
  'EVENT_BEAT_KOGA',
  'EVENT_BEAT_SABRINA',
  'EVENT_BEAT_BLAINE',
  'EVENT_BEAT_GIOVANNI',
  'EVENT_BEAT_ELITE_4',
  'EVENT_BEAT_CHAMPION',
  'EVENT_OBTAINED_CUT',
  'EVENT_OBTAINED_FLY',
  'EVENT_OBTAINED_SURF',
  'EVENT_OBTAINED_STRENGTH',
  'EVENT_OBTAINED_FLASH',
  'EVENT_GOT_BIKE_VOUCHER',
  'EVENT_GOT_BICYCLE',
  'EVENT_SS_ANNE_LEFT',
  'EVENT_GAME_CORNER_COINS',
  'EVENT_GOT_LIFT_KEY',
  'EVENT_RELEASED_ZAPDOS',
  'EVENT_SILPH_CO_SOLVED',
  'EVENT_GOT_OLD_AMBER',
  'EVENT_FOUND_FOSSIL',
  'EVENT_GOT_CARD_KEY',
  'EVENT_GOT_GOLD_TEETH',
  'EVENT_SAFARI_GAME_OVER',
  'EVENT_BOUGHT_MAGIKARP',
  'EVENT_GOT_TEA',
  'EVENT_GOT_RARE_CANDY',
  'EVENT_SEEL_SPOKEN',
  'EVENT_MET_GAMBLER',
  'EVENT_MET_BILL',
  'EVENT_MET_OAK',
  'EVENT_MET_RIVAL',
  'EVENT_MET_WARDEN',
  'EVENT_GOT_FUJI_NOTE',
  'EVENT_GOT_POKE_FLUTE',
  'EVENT_GOT_SILPH_SCOPE',
  'EVENT_GOT_SECRET_KEY',
  'EVENT_GOT_ITEMFINDER',
  'EVENT_GOT_COIN_CASE',
  'EVENT_GOT_POKEDOLL',
  'EVENT_GOT_EXP_ALL',
  'EVENT_FOUND_SNORLAX_12',
  'EVENT_FOUND_SNORLAX_16',
  'EVENT_BEAT_GYM_TRAINERS',
]

export const FACING_DIRECTIONS = ['down', 'up', 'left', 'right'] as const
export type Facing = typeof FACING_DIRECTIONS[number]

export const BADGE_NAMES = [
  'Boulder Badge',
  'Cascade Badge',
  'Thunder Badge',
  'Rainbow Badge',
  'Soul Badge',
  'Marsh Badge',
  'Volcano Badge',
  'Earth Badge',
]

export const MAP_NAMES = [
  'PalletTown', 'ViridianCity', 'PewterCity', 'CeruleanCity',
  'LavenderTown', 'VermilionCity', 'CeladonCity', 'FuchsiaCity',
  'CinnabarIsland', 'IndigoPlateau', 'SaffronCity',
  'Route1', 'Route2', 'Route3', 'Route4', 'Route5',
  'Route6', 'Route7', 'Route8', 'Route9', 'Route10',
  'Route11', 'Route12', 'Route13', 'Route14', 'Route15',
  'Route16', 'Route17', 'Route18', 'Route19', 'Route20',
  'Route21', 'Route22', 'Route23', 'Route24', 'Route25',
  'OaksLab', 'ViridianForest',
  'MtMoon1F', 'MtMoonB1F', 'MtMoonB2F',
  'SSAnne1F', 'SSAnne2F', 'SSAnneKitchen', 'SSAnneCaptainsRoom',
  'PokemonTower1F', 'PokemonTower2F', 'PokemonTower3F', 'PokemonTower4F', 'PokemonTower5F', 'PokemonTower6F', 'PokemonTower7F',
  'RocketHideoutB1F', 'RocketHideoutB2F', 'RocketHideoutB3F', 'RocketHideoutB4F',
  'SilphCo1F', 'SilphCo2F', 'SilphCo3F', 'SilphCo4F', 'SilphCo5F', 'SilphCo6F', 'SilphCo7F', 'SilphCo8F', 'SilphCo9F', 'SilphCo10F', 'SilphCo11F',
  'PokemonMansion1F', 'PokemonMansion2F', 'PokemonMansion3F', 'PokemonMansionB1F',
  'SafariZoneCenter', 'SafariZoneEast', 'SafariZoneNorth', 'SafariZoneWest',
  'CeruleanCave1F', 'CeruleanCave2F', 'CeruleanCaveB1F',
  'PowerPlant',
  'VictoryRoad1F', 'VictoryRoad2F', 'VictoryRoad3F',
  'SeafoamIslands1F', 'SeafoamIslandsB1F', 'SeafoamIslandsB2F', 'SeafoamIslandsB3F', 'SeafoamIslandsB4F',
  'DiglettsCave',
  'RockTunnel1F', 'RockTunnelB1F',
]

export function createDefaultSaveData(): SaveDataSnapshot {
  return {
    player: {
      playerName: 'RED',
      rivalName: 'BLUE',
      mapName: 'PalletTown',
      positionX: 5,
      positionY: 5,
      facing: 'down',
      playTimeHours: 0,
      playTimeMinutes: 0,
      money: 3000,
    },
    badges: [false, false, false, false, false, false, false, false],
    party: [
      {
        species: 'Pikachu',
        level: 25,
        currentHp: 70,
        maxHp: 70,
        moves: ['THUNDERSHOCK', 'QUICK_ATTACK', 'THUNDER_WAVE', 'DOUBLE_TEAM'],
        nickname: '',
      },
      {
        species: 'Pidgeotto',
        level: 18,
        currentHp: 50,
        maxHp: 50,
        moves: ['GUST', 'QUICK_ATTACK', 'SAND_ATTACK', 'WHIRLWIND'],
        nickname: '',
      },
      {
        species: 'Butterfree',
        level: 15,
        currentHp: 45,
        maxHp: 45,
        moves: ['CONFUSION', 'SLEEP_POWDER', 'STUN_SPORE', 'POISONPOWDER'],
        nickname: '',
      },
    ],
    items: [
      { name: 'POKE_BALL', quantity: 12 },
      { name: 'POTION', quantity: 5 },
      { name: 'RARE_CANDY', quantity: 2 },
      { name: 'ESCAPE_ROPE', quantity: 1 },
      { name: 'REPEL', quantity: 3 },
    ],
    flags: {
      EVENT_GOT_STARTER: true,
      EVENT_GOT_POKEDEX: true,
      EVENT_MET_OAK: true,
      EVENT_BEAT_BROCK: false,
      EVENT_BEAT_MISTY: false,
    },
  }
}
