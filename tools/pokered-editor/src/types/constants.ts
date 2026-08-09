export const TILE_SIZE = 8
export const BLOCK_TILES = 4

// All 45 music IDs from pokered-data/src/music.rs
export const MUSIC_LIST: string[] = [
  'PalletTown',
  'Pokecenter',
  'Gym',
  'Cities1',
  'Cities2',
  'Celadon',
  'Cinnabar',
  'Vermilion',
  'Lavender',
  'SSAnne',
  'MeetProfOak',
  'MeetRival',
  'MuseumGuy',
  'SafariZone',
  'PkmnHealed',
  'Routes1',
  'Routes2',
  'Routes3',
  'Routes4',
  'IndigoPlateau',
  'GymLeaderBattle',
  'TrainerBattle',
  'WildBattle',
  'FinalBattle',
  'DefeatedTrainer',
  'DefeatedWildMon',
  'DefeatedGymLeader',
  'TitleScreen',
  'Credits',
  'HallOfFame',
  'OaksLab',
  'JigglypuffSong',
  'BikeRiding',
  'Surfing',
  'GameCorner',
  'IntroBattle',
  'Dungeon1',
  'Dungeon2',
  'Dungeon3',
  'CinnabarMansion',
  'PokemonTower',
  'SilphCo',
  'MeetEvilTrainer',
  'MeetFemaleTrainer',
  'MeetMaleTrainer',
]

// All Pokemon species names from pokered-data/src/species.rs (Species enum)
// Used for the wild encounter editor's species dropdown.
export const SPECIES_LIST: string[] = [
  'None', 'Bulbasaur', 'Ivysaur', 'Venusaur', 'Charmander', 'Charmeleon', 'Charizard', 'Squirtle',
  'Wartortle', 'Blastoise', 'Caterpie', 'Metapod', 'Butterfree', 'Weedle', 'Kakuna', 'Beedrill',
  'Pidgey', 'Pidgeotto', 'Pidgeot', 'Rattata', 'Raticate', 'Spearow', 'Fearow', 'Ekans',
  'Arbok', 'Pikachu', 'Raichu', 'Sandshrew', 'Sandslash', 'NidoranF', 'Nidorina', 'Nidoqueen',
  'NidoranM', 'Nidorino', 'Nidoking', 'Clefairy', 'Clefable', 'Vulpix', 'Ninetales', 'Jigglypuff',
  'Wigglytuff', 'Zubat', 'Golbat', 'Oddish', 'Gloom', 'Vileplume', 'Paras', 'Parasect',
  'Venonat', 'Venomoth', 'Diglett', 'Dugtrio', 'Meowth', 'Persian', 'Psyduck', 'Golduck',
  'Mankey', 'Primeape', 'Growlithe', 'Arcanine', 'Poliwag', 'Poliwhirl', 'Poliwrath', 'Abra',
  'Kadabra', 'Alakazam', 'Machop', 'Machoke', 'Machamp', 'Bellsprout', 'Weepinbell', 'Victreebel',
  'Tentacool', 'Tentacruel', 'Geodude', 'Graveler', 'Golem', 'Ponyta', 'Rapidash', 'Slowpoke',
  'Slowbro', 'Magnemite', 'Magneton', 'Farfetchd', 'Doduo', 'Dodrio', 'Seel', 'Dewgong',
  'Grimer', 'Muk', 'Shellder', 'Cloyster', 'Gastly', 'Haunter', 'Gengar', 'Onix',
  'Drowzee', 'Hypno', 'Krabby', 'Kingler', 'Voltorb', 'Electrode', 'Exeggcute', 'Exeggutor',
  'Cubone', 'Marowak', 'Hitmonlee', 'Hitmonchan', 'Lickitung', 'Koffing', 'Weezing', 'Rhyhorn',
  'Rhydon', 'Chansey', 'Tangela', 'Kangaskhan', 'Horsea', 'Seadra', 'Goldeen', 'Seaking',
  'Staryu', 'Starmie', 'MrMime', 'Scyther', 'Jynx', 'Electabuzz', 'Magmar', 'Pinsir',
  'Tauros', 'Magikarp', 'Gyarados', 'Lapras', 'Ditto', 'Eevee', 'Vaporeon', 'Jolteon',
  'Flareon', 'Porygon', 'Omanyte', 'Omastar', 'Kabuto', 'Kabutops', 'Aerodactyl', 'Snorlax',
  'Articuno', 'Zapdos', 'Moltres', 'Dratini', 'Dragonair', 'Dragonite', 'Mewtwo', 'Mew',
]

// Direction names for map connections
export const CONNECTION_DIRECTIONS = ['north', 'south', 'west', 'east'] as const
export type ConnectionDirection = typeof CONNECTION_DIRECTIONS[number]

// Town map coordinates for minimap display (from town_map_data.rs)
// Maps 0x00-0x24 (37 outdoor maps) with x,y coordinates on 16x16 grid
export interface TownMapCoord {
  mapId: number
  mapName: string
  x: number
  y: number
  displayName: string
}

// Outdoor map coordinates from OUTDOOR_TOWN_MAP_ENTRIES
export const TOWN_MAP_COORDS: TownMapCoord[] = [
  { mapId: 0x00, mapName: 'PalletTown', x: 2, y: 11, displayName: 'PALLET TOWN' },
  { mapId: 0x01, mapName: 'ViridianCity', x: 2, y: 8, displayName: 'VIRIDIAN CITY' },
  { mapId: 0x02, mapName: 'PewterCity', x: 2, y: 3, displayName: 'PEWTER CITY' },
  { mapId: 0x03, mapName: 'CeruleanCity', x: 10, y: 2, displayName: 'CERULEAN CITY' },
  { mapId: 0x04, mapName: 'LavenderTown', x: 14, y: 5, displayName: 'LAVENDER TOWN' },
  { mapId: 0x05, mapName: 'VermilionCity', x: 10, y: 9, displayName: 'VERMILION CITY' },
  { mapId: 0x06, mapName: 'CeladonCity', x: 7, y: 5, displayName: 'CELADON CITY' },
  { mapId: 0x07, mapName: 'FuchsiaCity', x: 8, y: 13, displayName: 'FUCHSIA CITY' },
  { mapId: 0x08, mapName: 'CinnabarIsland', x: 2, y: 15, displayName: 'CINNABAR ISLAND' },
  { mapId: 0x09, mapName: 'IndigoPlateau', x: 0, y: 2, displayName: 'INDIGO PLATEAU' },
  { mapId: 0x0A, mapName: 'SaffronCity', x: 10, y: 5, displayName: 'SAFFRON CITY' },
  { mapId: 0x0B, mapName: 'UnusedMap0B', x: 0, y: 0, displayName: 'UNUSED' },
  { mapId: 0x0C, mapName: 'Route1', x: 2, y: 10, displayName: 'ROUTE 1' },
  { mapId: 0x0D, mapName: 'Route2', x: 2, y: 6, displayName: 'ROUTE 2' },
  { mapId: 0x0E, mapName: 'Route3', x: 4, y: 3, displayName: 'ROUTE 3' },
  { mapId: 0x0F, mapName: 'Route4', x: 8, y: 2, displayName: 'ROUTE 4' },
  { mapId: 0x10, mapName: 'Route5', x: 10, y: 3, displayName: 'ROUTE 5' },
  { mapId: 0x11, mapName: 'Route6', x: 10, y: 8, displayName: 'ROUTE 6' },
  { mapId: 0x12, mapName: 'Route7', x: 8, y: 5, displayName: 'ROUTE 7' },
  { mapId: 0x13, mapName: 'Route8', x: 13, y: 5, displayName: 'ROUTE 8' },
  { mapId: 0x14, mapName: 'Route9', x: 13, y: 2, displayName: 'ROUTE 9' },
  { mapId: 0x15, mapName: 'Route10', x: 14, y: 4, displayName: 'ROUTE 10' },
  { mapId: 0x16, mapName: 'Route11', x: 12, y: 9, displayName: 'ROUTE 11' },
  { mapId: 0x17, mapName: 'Route12', x: 14, y: 9, displayName: 'ROUTE 12' },
  { mapId: 0x18, mapName: 'Route13', x: 13, y: 11, displayName: 'ROUTE 13' },
  { mapId: 0x19, mapName: 'Route14', x: 11, y: 12, displayName: 'ROUTE 14' },
  { mapId: 0x1A, mapName: 'Route15', x: 10, y: 13, displayName: 'ROUTE 15' },
  { mapId: 0x1B, mapName: 'Route16', x: 5, y: 5, displayName: 'ROUTE 16' },
  { mapId: 0x1C, mapName: 'Route17', x: 4, y: 8, displayName: 'ROUTE 17' },
  { mapId: 0x1D, mapName: 'Route18', x: 6, y: 13, displayName: 'ROUTE 18' },
  { mapId: 0x1E, mapName: 'Route19', x: 6, y: 15, displayName: 'SEA ROUTE 19' },
  { mapId: 0x1F, mapName: 'Route20', x: 4, y: 15, displayName: 'SEA ROUTE 20' },
  { mapId: 0x20, mapName: 'Route21', x: 2, y: 13, displayName: 'SEA ROUTE 21' },
  { mapId: 0x21, mapName: 'Route22', x: 0, y: 8, displayName: 'ROUTE 22' },
  { mapId: 0x22, mapName: 'Route23', x: 0, y: 6, displayName: 'ROUTE 23' },
  { mapId: 0x23, mapName: 'Route24', x: 10, y: 1, displayName: 'ROUTE 24' },
  { mapId: 0x24, mapName: 'Route25', x: 11, y: 0, displayName: 'ROUTE 25' },
]

// Tileset categories — derived from the original game's classification:
// - Outdoor (室外):      home/overworld.asm CheckIfInOutsideMap (Overworld, Plateau)
// - Cave/Dungeon (洞穴): data/tilesets/dungeon_tilesets.asm
// - Indoor (室内):       everything else
// In R/B there is no separate map-header field for indoor/outdoor/cave —
// the engine derives it from the tileset, which is why the same lookup is
// surfaced here rather than introducing a new map-header attribute.
export type TilesetCategory = 'outdoor' | 'indoor' | 'cave'

export const TILESET_OUTDOOR: ReadonlySet<string> = new Set([
  'Overworld',
  'Plateau',
])

export const TILESET_CAVE: ReadonlySet<string> = new Set([
  'Forest',
  'Museum',
  'Ship',
  'Cavern',
  'Lobby',
  'Mansion',
  'Gate',
  'Lab',
  'Facility',
  'Cemetery',
  'Gym',
])

export function tilesetCategory(name: string): TilesetCategory {
  if (TILESET_OUTDOOR.has(name)) return 'outdoor'
  if (TILESET_CAVE.has(name)) return 'cave'
  return 'indoor'
}

export const TILESET_CATEGORY_LABEL: Record<TilesetCategory, string> = {
  outdoor: '室外 Outdoor',
  indoor: '室内 Indoor',
  cave: '洞穴/迷宫 Cave/Dungeon',
}

export const TILESET_FILES: Record<string, string> = {
  Overworld: 'overworld.png',
  RedsHouse1: 'reds_house.png',
  Mart: 'pokecenter.png',
  Forest: 'forest.png',
  RedsHouse2: 'reds_house.png',
  Dojo: 'gym.png',
  Pokecenter: 'pokecenter.png',
  Gym: 'gym.png',
  House: 'house.png',
  ForestGate: 'gate.png',
  Museum: 'gate.png',
  Underground: 'underground.png',
  Gate: 'gate.png',
  Ship: 'ship.png',
  ShipPort: 'ship_port.png',
  Cemetery: 'cemetery.png',
  Interior: 'interior.png',
  Cavern: 'cavern.png',
  Lobby: 'lobby.png',
  Mansion: 'mansion.png',
  Lab: 'lab.png',
  Club: 'club.png',
  Facility: 'facility.png',
  Plateau: 'plateau.png',
}

export const POKEMON_TYPES: string[] = [
  'Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock',
  'Bird', 'Bug', 'Ghost',
  'Fire', 'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon',
]

export const GROWTH_RATES: string[] = [
  'MediumFast', 'SlightlyFast', 'SlightlySlow', 'MediumSlow', 'Fast', 'Slow',
]

export const MOVE_EFFECTS: string[] = [
  'NoAdditionalEffect', 'Effect01', 'PoisonSideEffect1', 'DrainHpEffect',
  'BurnSideEffect1', 'FreezeSideEffect1', 'ParalyzeSideEffect1', 'ExplodeEffect',
  'DreamEaterEffect', 'MirrorMoveEffect', 'AttackUp1Effect', 'DefenseUp1Effect',
  'SpeedUp1Effect', 'SpecialUp1Effect', 'AccuracyUp1Effect', 'EvasionUp1Effect',
  'PayDayEffect', 'SwiftEffect', 'AttackDown1Effect', 'DefenseDown1Effect',
  'SpeedDown1Effect', 'SpecialDown1Effect', 'AccuracyDown1Effect', 'EvasionDown1Effect',
  'ConversionEffect', 'HazeEffect', 'BideEffect', 'ThrashPetalDanceEffect',
  'SwitchAndTeleportEffect', 'TwoToFiveAttacksEffect', 'Effect1E', 'FlinchSideEffect1',
  'SleepEffect', 'PoisonSideEffect2', 'BurnSideEffect2', 'FreezeSideEffect2',
  'ParalyzeSideEffect2', 'FlinchSideEffect2', 'OhkoEffect', 'ChargeEffect',
  'SuperFangEffect', 'SpecialDamageEffect', 'TrappingEffect', 'FlyEffect',
  'AttackTwiceEffect', 'JumpKickEffect', 'MistEffect', 'FocusEnergyEffect',
  'RecoilEffect', 'ConfusionEffect', 'AttackUp2Effect', 'DefenseUp2Effect',
  'SpeedUp2Effect', 'SpecialUp2Effect', 'AccuracyUp2Effect', 'EvasionUp2Effect',
  'HealEffect', 'TransformEffect', 'AttackDown2Effect', 'DefenseDown2Effect',
  'SpeedDown2Effect', 'SpecialDown2Effect', 'AccuracyDown2Effect', 'EvasionDown2Effect',
  'LightScreenEffect', 'ReflectEffect', 'PoisonEffect', 'ParalyzeEffect',
  'AttackDownSideEffect', 'DefenseDownSideEffect', 'SpeedDownSideEffect',
  'SpecialDownSideEffect', 'ConfusionSideEffect', 'TwineedleEffect', 'SubstituteEffect',
  'HyperBeamEffect', 'RageEffect', 'MimicEffect', 'MetronomeEffect', 'LeechSeedEffect',
  'SplashEffect', 'DisableEffect',
]

// All 165 moves in MoveId dex order (Pound=0x01..Struggle=0xA5). 'None' is excluded
// for editor lists; include manually if your form needs an "empty move" slot.
export const MOVE_LIST: string[] = [
  'Pound', 'KarateChop', 'Doubleslap', 'CometPunch', 'MegaPunch', 'PayDay',
  'FirePunch', 'IcePunch', 'Thunderpunch', 'Scratch', 'Vicegrip', 'Guillotine',
  'RazorWind', 'SwordsDance', 'Cut', 'Gust', 'WingAttack', 'Whirlwind',
  'Fly', 'Bind', 'Slam', 'VineWhip', 'Stomp', 'DoubleKick',
  'MegaKick', 'JumpKick', 'RollingKick', 'SandAttack', 'Headbutt', 'HornAttack',
  'FuryAttack', 'HornDrill', 'Tackle', 'BodySlam', 'Wrap', 'TakeDown',
  'Thrash', 'DoubleEdge', 'TailWhip', 'PoisonSting', 'Twineedle', 'PinMissile',
  'Leer', 'Bite', 'Growl', 'Roar', 'Sing', 'Supersonic',
  'Sonicboom', 'Disable', 'Acid', 'Ember', 'Flamethrower', 'Mist',
  'WaterGun', 'HydroPump', 'Surf', 'IceBeam', 'Blizzard', 'Psybeam',
  'Bubblebeam', 'AuroraBeam', 'HyperBeam', 'Peck', 'DrillPeck', 'Submission',
  'LowKick', 'Counter', 'SeismicToss', 'Strength', 'Absorb', 'MegaDrain',
  'LeechSeed', 'Growth', 'RazorLeaf', 'Solarbeam', 'Poisonpowder', 'StunSpore',
  'SleepPowder', 'PetalDance', 'StringShot', 'DragonRage', 'FireSpin', 'Thundershock',
  'Thunderbolt', 'ThunderWave', 'Thunder', 'RockThrow', 'Earthquake', 'Fissure',
  'Dig', 'Toxic', 'Confusion', 'PsychicM', 'Hypnosis', 'Meditate',
  'Agility', 'QuickAttack', 'Rage', 'Teleport', 'NightShade', 'Mimic',
  'Screech', 'DoubleTeam', 'Recover', 'Harden', 'Minimize', 'Smokescreen',
  'ConfuseRay', 'Withdraw', 'DefenseCurl', 'Barrier', 'LightScreen', 'Haze',
  'Reflect', 'FocusEnergy', 'Bide', 'Metronome', 'MirrorMove', 'Selfdestruct',
  'EggBomb', 'Lick', 'Smog', 'Sludge', 'BoneClub', 'FireBlast',
  'Waterfall', 'Clamp', 'Swift', 'SkullBash', 'SpikeCannon', 'Constrict',
  'Amnesia', 'Kinesis', 'Softboiled', 'HiJumpKick', 'Glare', 'DreamEater',
  'PoisonGas', 'Barrage', 'LeechLife', 'LovelyKiss', 'SkyAttack', 'Transform',
  'Bubble', 'DizzyPunch', 'Spore', 'Flash', 'Psywave', 'Splash',
  'AcidArmor', 'Crabhammer', 'Explosion', 'FurySwipes', 'Bonemerang', 'Rest',
  'RockSlide', 'HyperFang', 'Sharpen', 'Conversion', 'TriAttack', 'SuperFang',
  'Slash', 'Substitute', 'Struggle',
]

// The 5 evolution stones in Pokemon R/B (from ItemId enum). Used by the
// Pokemon editor's "evolve by item" dropdown.
export const EVOLUTION_ITEMS: string[] = [
  'MoonStone', 'FireStone', 'ThunderStone', 'WaterStone', 'LeafStone',
]
