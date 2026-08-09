#!/usr/bin/env node
/**
 * Extract music mappings from songs.asm and verify/fix map.json files
 * Usage: node scripts/fix-map-music.js [--fix]
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SONGS_ASM = path.resolve(__dirname, '../../../../data/maps/songs.asm');
const MAPS_DIR = path.resolve(__dirname, '../../../crates/pokered-data/maps');

// Map MUSIC_* constants to our music names
const MUSIC_MAP = {
  'MUSIC_PALLET_TOWN': 'PalletTown',
  'MUSIC_CITIES1': 'Cities1',
  'MUSIC_CITIES2': 'Cities2',
  'MUSIC_LAVENDER': 'Lavender',
  'MUSIC_VERMILION': 'Vermilion',
  'MUSIC_CELADON': 'Celadon',
  'MUSIC_CINNABAR': 'Cinnabar',
  'MUSIC_INDIGO_PLATEAU': 'IndigoPlateau',
  'MUSIC_ROUTES1': 'Routes1',
  'MUSIC_ROUTES2': 'Routes2',
  'MUSIC_ROUTES3': 'Routes3',
  'MUSIC_ROUTES4': 'Routes4',
  'MUSIC_POKECENTER': 'Pokecenter',
  'MUSIC_GYM': 'Gym',
  'MUSIC_DUNGEON1': 'Dungeon1',
  'MUSIC_DUNGEON2': 'Dungeon2',
  'MUSIC_DUNGEON3': 'Dungeon3',
  'MUSIC_OAKS_LAB': 'OaksLab',
  'MUSIC_SS_ANNE': 'SSAnne',
  'MUSIC_POKEMON_TOWER': 'PokemonTower',
  'MUSIC_SILPH_CO': 'SilphCo',
  'MUSIC_CINNABAR_MANSION': 'CinnabarMansion',
  'MUSIC_SAFARI_ZONE': 'SafariZone',
  'MUSIC_GAME_CORNER': 'GameCorner',
};

// Map ID to map name (from map_constants.asm order)
const MAP_ID_TO_NAME = [
  'PalletTown',
  'ViridianCity',
  'PewterCity',
  'CeruleanCity',
  'LavenderTown',
  'VermilionCity',
  'CeladonCity',
  'FuchsiaCity',
  'CinnabarIsland',
  'IndigoPlateau',
  'SaffronCity',
  'UnusedMap0B',
  'Route1',
  'Route2',
  'Route3',
  'Route4',
  'Route5',
  'Route6',
  'Route7',
  'Route8',
  'Route9',
  'Route10',
  'Route11',
  'Route12',
  'Route13',
  'Route14',
  'Route15',
  'Route16',
  'Route17',
  'Route18',
  'Route19',
  'Route20',
  'Route21',
  'Route22',
  'Route23',
  'Route24',
  'Route25',
  'RedsHouse1F',
  'RedsHouse2F',
  'BluesHouse',
  'OaksLab',
  'ViridianPokecenter',
  'ViridianMart',
  'ViridianSchoolHouse',
  'ViridianNicknameHouse',
  'ViridianGym',
  'DiglettsCaveRoute2',
  'ViridianForestNorthGate',
  'Route2TradeHouse',
  'Route2Gate',
  'ViridianForestSouthGate',
  'ViridianForest',
  'Museum1F',
  'Museum2F',
  'PewterGym',
  'PewterNidoranHouse',
  'PewterMart',
  'PewterSpeechHouse',
  'PewterPokecenter',
  'MtMoon1F',
  'MtMoonB1F',
  'MtMoonB2F',
  'CeruleanTrashedHouse',
  'CeruleanTradeHouse',
  'CeruleanPokecenter',
  'CeruleanGym',
  'BikeShop',
  'CeruleanMart',
  'MtMoonPokecenter',
  'CeruleanTrashedHouseCopy',
  'Route5Gate',
  'UndergroundPathRoute5',
  'Daycare',
  'Route6Gate',
  'UndergroundPathRoute6',
  'UndergroundPathRoute6Copy',
  'Route7Gate',
  'UndergroundPathRoute7',
  'UndergroundPathRoute7Copy',
  'Route8Gate',
  'UndergroundPathRoute8',
  'RockTunnelPokecenter',
  'RockTunnel1F',
  'PowerPlant',
  'Route11Gate1F',
  'DiglettsCaveRoute11',
  'Route11Gate2F',
  'Route12Gate1F',
  'BillsHouse',
  'VermilionPokecenter',
  'PokemonFanClub',
  'VermilionMart',
  'VermilionGym',
  'VermilionPidgeyHouse',
  'VermilionDock',
  'SSAnne1F',
  'SSAnne2F',
  'SSAnne3F',
  'SSAnneB1F',
  'SSAnneBow',
  'SSAnneKitchen',
  'SSAnneCaptainsRoom',
  'SSAnne1FRooms',
  'SSAnne2FRooms',
  'SSAnneB1FRooms',
  'UnusedMap69',
  'UnusedMap6A',
  'UnusedMap6B',
  'VictoryRoad1F',
  'UnusedMap6D',
  'UnusedMap6E',
  'UnusedMap6F',
  'UnusedMap70',
  'LancesRoom',
  'UnusedMap72',
  'UnusedMap73',
  'UnusedMap74',
  'UnusedMap75',
  'HallOfFame',
  'UndergroundPathNorthSouth',
  'ChampionsRoom',
  'UndergroundPathWestEast',
  'CeladonMart1F',
  'CeladonMart2F',
  'CeladonMart3F',
  'CeladonMart4F',
  'CeladonMartRoof',
  'CeladonMartElevator',
  'CeladonMansion1F',
  'CeladonMansion2F',
  'CeladonMansion3F',
  'CeladonMansionRoof',
  'CeladonMansionRoofHouse',
  'CeladonPokecenter',
  'CeladonGym',
  'GameCorner',
  'CeladonMart5F',
  'GameCornerPrizeRoom',
  'CeladonDiner',
  'CeladonChiefHouse',
  'CeladonHotel',
  'LavenderPokecenter',
  'PokemonTower1F',
  'PokemonTower2F',
  'PokemonTower3F',
  'PokemonTower4F',
  'PokemonTower5F',
  'PokemonTower6F',
  'PokemonTower7F',
  'MrFujisHouse',
  'LavenderMart',
  'LavenderCuboneHouse',
  'FuchsiaMart',
  'FuchsiaBillsGrandpasHouse',
  'FuchsiaPokecenter',
  'WardensHouse',
  'SafariZoneGate',
  'FuchsiaGym',
  'FuchsiaMeetingRoom',
  'SeafoamIslandsB1F',
  'SeafoamIslandsB2F',
  'SeafoamIslandsB3F',
  'SeafoamIslandsB4F',
  'VermilionOldRodHouse',
  'FuchsiaGoodRodHouse',
  'PokemonMansion1F',
  'CinnabarGym',
  'CinnabarLab',
  'CinnabarLabTradeRoom',
  'CinnabarLabMetronomeRoom',
  'CinnabarLabFossilRoom',
  'CinnabarPokecenter',
  'CinnabarMart',
  'CinnabarMartCopy',
  'IndigoPlateauLobby',
  'CopycatsHouse1F',
  'CopycatsHouse2F',
  'FightingDojo',
  'SaffronGym',
  'SaffronPidgeyHouse',
  'SaffronMart',
  'SilphCo1F',
  'SaffronPokecenter',
  'MrPsychicsHouse',
  'Route15Gate1F',
  'Route15Gate2F',
  'Route16Gate1F',
  'Route16Gate2F',
  'Route16FlyHouse',
  'Route12SuperRodHouse',
  'Route18Gate1F',
  'Route18Gate2F',
  'SeafoamIslands1F',
  'Route22Gate',
  'VictoryRoad2F',
  'Route12Gate2F',
  'VermilionTradeHouse',
  'DiglettsCave',
  'VictoryRoad3F',
  'RocketHideoutB1F',
  'RocketHideoutB2F',
  'RocketHideoutB3F',
  'RocketHideoutB4F',
  'RocketHideoutElevator',
  'UnusedMapCC',
  'UnusedMapCD',
  'UnusedMapCE',
  'SilphCo2F',
  'SilphCo3F',
  'SilphCo4F',
  'SilphCo5F',
  'SilphCo6F',
  'SilphCo7F',
  'SilphCo8F',
  'PokemonMansion2F',
  'PokemonMansion3F',
  'PokemonMansionB1F',
  'SafariZoneEast',
  'SafariZoneNorth',
  'SafariZoneWest',
  'SafariZoneCenter',
  'SafariZoneCenterRestHouse',
  'SafariZoneSecretHouse',
  'SafariZoneWestRestHouse',
  'SafariZoneEastRestHouse',
  'SafariZoneNorthRestHouse',
  'CeruleanCave2F',
  'CeruleanCaveB1F',
  'CeruleanCave1F',
  'NameRatersHouse',
  'CeruleanBadgeHouse',
  'UnusedMapE7',
  'RockTunnelB1F',
  'SilphCo9F',
  'SilphCo10F',
  'SilphCo11F',
  'SilphCoElevator',
  'UnusedMapED',
  'UnusedMapEE',
  'TradeCenter',
  'Colosseum',
  'UnusedMapF1',
  'UnusedMapF2',
  'UnusedMapF3',
  'UnusedMapF4',
  'LoreleisRoom',
  'BrunosRoom',
  'AgathasRoom',
];

function parseSongsAsm() {
  const content = fs.readFileSync(SONGS_ASM, 'utf-8');
  const lines = content.split('\n');
  const musicMap = new Map();

  // Special acronym handling
  const ACRONYMS = ['SS'];

  for (const line of lines) {
    const match = line.match(/^\s*db\s+(MUSIC_\w+),\s*BANK\([^)]+\)\s*;\s*(\w+)/);
    if (match) {
      const [, musicConst, mapConstName] = match;
      const musicName = MUSIC_MAP[musicConst];
      if (musicName) {
        // Convert SCREAMING_SNAKE_CASE to PascalCase with special handling
        // e.g., PALLET_TOWN -> PalletTown, MT_MOON_1F -> MtMoon1F, SS_ANNE_1F -> SSAnne1F
        const mapName = mapConstName
          .toLowerCase()
          .split('_')
          .map(word => {
            // Preserve floor suffixes (1F, 2F, 10F, B1F, etc.)
            if (/^[0-9]+f$/i.test(word) || /^b[0-9]+f$/i.test(word)) {
              return word.toUpperCase();
            }
            // Preserve hex suffixes (0B, 6A, CC, etc.)
            if (/^[0-9a-f]{2}$/i.test(word)) {
              return word.toUpperCase();
            }
            // Handle acronyms (SS)
            const upper = word.toUpperCase();
            if (ACRONYMS.includes(upper)) {
              return upper;
            }
            return word.charAt(0).toUpperCase() + word.slice(1);
          })
          .join('');
        musicMap.set(mapName, musicName);
      } else {
        console.warn(`Unknown music constant: ${musicConst}`);
      }
    }
  }

  return musicMap;
}

function checkAndFixMaps(musicMap, shouldFix) {
  const mapDirs = fs.readdirSync(MAPS_DIR, { withFileTypes: true })
    .filter(d => d.isDirectory())
    .map(d => d.name);

  let mismatches = 0;
  let fixed = 0;
  let missing = 0;

  for (const mapName of mapDirs) {
    const mapJsonPath = path.join(MAPS_DIR, mapName, 'map.json');
    if (!fs.existsSync(mapJsonPath)) {
      missing++;
      continue;
    }

    const mapJson = JSON.parse(fs.readFileSync(mapJsonPath, 'utf-8'));
    const currentMusic = mapJson.header?.music;
    const expectedMusic = musicMap.get(mapName);

    if (!expectedMusic) {
      console.log(`  No music mapping for: ${mapName}`);
      continue;
    }

    if (currentMusic !== expectedMusic) {
      mismatches++;
      console.log(`  ${mapName}: "${currentMusic}" -> "${expectedMusic}"`);

      if (shouldFix) {
        mapJson.header.music = expectedMusic;
        fs.writeFileSync(mapJsonPath, JSON.stringify(mapJson, null, 2) + '\n');
        fixed++;
      }
    }
  }

  console.log(`\n=== Summary ===`);
  console.log(`Total maps: ${mapDirs.length}`);
  console.log(`Music mismatches: ${mismatches}`);
  console.log(`Fixed: ${shouldFix ? fixed : 0}`);
  console.log(`Missing map.json: ${missing}`);

  return { mismatches, fixed };
}

const shouldFix = process.argv.includes('--fix');
console.log('Parsing songs.asm...');
const musicMap = parseSongsAsm();
console.log(`Found ${musicMap.size} music mappings\n`);

console.log('Checking map.json files...');
checkAndFixMaps(musicMap, shouldFix);