import { speciesToSpriteName } from './pokemon'
import { SPECIES_LIST } from './constants'

export type AssetCategory =
  | 'pokemon-front'
  | 'pokemon-back'
  | 'trainer'
  | 'npc'
  | 'tileset'
  | 'ui'
  | 'effects'

export interface AssetEntry {
  category: AssetCategory
  id: string
  filename: string
  displayName: string
  tilePixelWidth?: number
  tilePixelHeight?: number
  tileCount?: number
}

export type DrawTool = 'pencil' | 'fill' | 'eyedropper' | 'erase'

// ── Color mode types ────────────────────────────────────────────────────

export type TileColorMode = 'dmg' | 'gba' | 'fullcolor'

export interface Rgba {
  r: number // 0-255
  g: number // 0-255
  b: number // 0-255
  a: number // 0-255
}

export interface DmgPalette {
  colors: [number, number, number, number]
}

export interface GbaSubPalette {
  colors: [
    number, number, number, number, number, number, number, number,
    number, number, number, number, number, number, number, number,
  ]
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface FullColorPalette {
  // Dynamic — editor picks any color at runtime
}

/**
 * The four DMG grey shades as 24-bit RGB hex values.
 * Index 0 = white (lightest), 3 = black (darkest).
 */
export const DMG_COLORS: [number, number, number, number] = [
  0xFFFFFF, 0xAAAAAA, 0x555555, 0x000000,
]

/**
 * Create 16 placeholder GBA sub-palettes with distinct color families.
 * Each sub-palette contains 16 colors.
 */
export function createDefaultGbaPalettes(): GbaSubPalette[] {
  function makePalette(...colors: number[]): GbaSubPalette {
    if (colors.length !== 16) throw new Error('Palette must have exactly 16 colors')
    return { colors: colors as GbaSubPalette['colors'] }
  }

  return [
    // 0: Grayscale
    makePalette(
      0xFFFFFF, 0xE0E0E0, 0xC0C0C0, 0xA0A0A0, 0x808080, 0x606060, 0x404040, 0x202020,
      0xF8F8F8, 0xD0D0D0, 0xB0B0B0, 0x909090, 0x707070, 0x505050, 0x303030, 0x101010,
    ),
    // 1: Warm (reds, oranges, yellows)
    makePalette(
      0xFF0000, 0xFF4000, 0xFF8000, 0xFFC000, 0xFFFF00, 0xFFE080, 0xFFA040, 0xFF6000,
      0xE00000, 0xC04000, 0xA06000, 0x808000, 0xC08040, 0xE0A060, 0xFFC080, 0xFFE0A0,
    ),
    // 2: Cool (blues, cyans, greens)
    makePalette(
      0x0000FF, 0x0040FF, 0x0080FF, 0x00C0FF, 0x00FFFF, 0x00FFC0, 0x00FF80, 0x00FF40,
      0x0000E0, 0x0040C0, 0x0080A0, 0x00C080, 0x4080C0, 0x60A0E0, 0x80C0FF, 0xA0E0FF,
    ),
    // 3: Pastels
    makePalette(
      0xFFB0B0, 0xFFD0B0, 0xFFF0B0, 0xE0FFB0, 0xB0FFB0, 0xB0FFE0, 0xB0E0FF, 0xB0C0FF,
      0xD0B0FF, 0xFFB0E0, 0xFFD0D0, 0xFFF0D0, 0xE0FFD0, 0xD0FFD0, 0xD0E0FF, 0xD0D0FF,
    ),
    // 4: Earth tones
    makePalette(
      0x8B4513, 0xA0522D, 0xCD853F, 0xDEB887, 0xF5DEB3, 0xD2B48C, 0xBC8F8F, 0x8B7355,
      0x6B3A1F, 0x7B5B3A, 0x9B7B5A, 0xBBA07A, 0xD4C49A, 0xE8D8B8, 0xC4A882, 0xA08060,
    ),
    // 5: Neon
    makePalette(
      0xFF00FF, 0x00FFFF, 0xFFFF00, 0x00FF00, 0xFF0000, 0xFF8000, 0x80FF00, 0x00FF80,
      0x0080FF, 0x8000FF, 0xFF0080, 0xFF4080, 0x40FF80, 0x80FF40, 0x4080FF, 0xFF40FF,
    ),
    // 6: Dark/Gothic
    makePalette(
      0x1A001A, 0x330033, 0x4D004D, 0x660066, 0x1A1A2E, 0x2E2E4D, 0x0D0D1A, 0x262640,
      0x400020, 0x600030, 0x200040, 0x400060, 0x003030, 0x004040, 0x102030, 0x203040,
    ),
    // 7: Ocean
    makePalette(
      0x000080, 0x0000A0, 0x0000C0, 0x0020E0, 0x0040FF, 0x0060E0, 0x0080C0, 0x00A0A0,
      0x004080, 0x006060, 0x00A0C0, 0x00C0E0, 0x0080FF, 0x2080C0, 0x40A0E0, 0x80C0FF,
    ),
    // 8: Forest
    makePalette(
      0x003300, 0x006600, 0x009900, 0x00CC00, 0x00FF00, 0x33AA00, 0x66BB00, 0x99CC00,
      0x004400, 0x006633, 0x008844, 0x00AA55, 0x228B22, 0x2E8B57, 0x3CB371, 0x66CDAA,
    ),
    // 9: Sunset
    makePalette(
      0xFF4500, 0xFF6347, 0xFF7F50, 0xFFA07A, 0xFFD700, 0xFFA500, 0xFF8C00, 0xFF6600,
      0xE65C00, 0xCC5200, 0xB34700, 0x993D00, 0xFFB347, 0xFFC966, 0xFFD98A, 0xFFE4B5,
    ),
    // 10: Ice
    makePalette(
      0xE0FFFF, 0xC0F0FF, 0xA0E0FF, 0x80D0FF, 0xB0E0E6, 0xADD8E6, 0x87CEEB, 0x87CEFA,
      0xE0F0FF, 0xD0E8FF, 0xC0E0FF, 0xB0D8FF, 0x4682B4, 0x5F9EA0, 0x6495ED, 0x4169E1,
    ),
    // 11: Fire
    makePalette(
      0xFF0000, 0xFF2000, 0xFF4000, 0xFF6000, 0xFF8000, 0xFFA000, 0xFFC000, 0xFFE000,
      0xE00000, 0xC02000, 0xA04000, 0x806000, 0xFFFF00, 0xFFE040, 0xFFC080, 0xFFA0C0,
    ),
    // 12: Purple/Magenta
    makePalette(
      0x4B0082, 0x6A0DAD, 0x8A2BE2, 0x9370DB, 0xBA55D3, 0xDA70D6, 0xEE82EE, 0xDDA0DD,
      0x800080, 0x9932CC, 0xB266FF, 0xCC99FF, 0x9400D3, 0xBF00FF, 0xD02090, 0xFF1493,
    ),
    // 13: Gold/Brass
    makePalette(
      0xFFD700, 0xDAA520, 0xB8860B, 0xCDAD00, 0xC5B358, 0xB5A642, 0xD4AF37, 0xE6C229,
      0xFFC125, 0xF4C430, 0xE8B820, 0xDCAD10, 0x8B6508, 0xCD950C, 0xEEAD0E, 0xFFB90F,
    ),
    // 14: Skin tones
    makePalette(
      0xFFE0BD, 0xFFCD94, 0xFFD39B, 0xE8B960, 0xD2A05A, 0xC68642, 0xA06A3A, 0x8B5A2B,
      0xFDDCB5, 0xEDC9A0, 0xDDB78F, 0xCDA57E, 0xE0AC69, 0xC68E4E, 0xB0703A, 0x96613C,
    ),
    // 15: Game Boy Classic (16-step grayscale)
    makePalette(
      0xFFFFFF, 0xF0F0F0, 0xE0E0E0, 0xD0D0D0, 0xC0C0C0, 0xB0B0B0, 0xA0A0A0, 0x909090,
      0x808080, 0x707070, 0x606060, 0x505050, 0x404040, 0x303030, 0x202020, 0x000000,
    ),
  ]
}

/**
 * Convert a 24-bit hex color to RGBA tuple (alpha always 255).
 */
export function hexToRgba(hex: number): [number, number, number, number] {
  return [(hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff, 255]
}

export interface PixelHistoryEntry {
  before: number[][]
  after: number[][]
  timestamp: number
}

export interface TilesetTileMeta {
  tilesetName: string
  tileIndex: number
  x: number
  y: number
  label?: string
}

/**
 * Returns the on-disk back-sprite filename for a given species.
 * Convention: front stem + 'b' -> e.g. 'bulbasaurb.png', 'mr.mimeb.png'.
 */
export function backSpriteFilename(species: string): string {
  return `${speciesToSpriteName(species)}b.png`
}

const POKEMON_SPECIES: string[] = SPECIES_LIST.slice(1)

export function getPokemonFrontAssets(): AssetEntry[] {
  const entries: AssetEntry[] = POKEMON_SPECIES.map((species) => ({
    category: 'pokemon-front' as const,
    id: species,
    filename: `${speciesToSpriteName(species)}.png`,
    displayName: species,
  }))

  entries.push({
    category: 'pokemon-front',
    id: 'FossilKabutops',
    filename: 'fossilkabutops.png',
    displayName: 'Fossil Kabutops',
  })
  entries.push({
    category: 'pokemon-front',
    id: 'FossilAerodactyl',
    filename: 'fossilaerodactyl.png',
    displayName: 'Fossil Aerodactyl',
  })

  return entries
}

export function getPokemonBackAssets(): AssetEntry[] {
  return POKEMON_SPECIES.map((species) => ({
    category: 'pokemon-back' as const,
    id: species,
    filename: backSpriteFilename(species),
    displayName: species,
  }))
}

interface TrainerAssetDef {
  filename: string
  displayName: string
}

const TRAINER_ASSETS: TrainerAssetDef[] = [
  { filename: 'agatha.png', displayName: 'Agatha' },
  { filename: 'beauty.png', displayName: 'Beauty' },
  { filename: 'biker.png', displayName: 'Biker' },
  { filename: 'birdkeeper.png', displayName: 'Bird Keeper' },
  { filename: 'blackbelt.png', displayName: 'Blackbelt' },
  { filename: 'blaine.png', displayName: 'Blaine' },
  { filename: 'brock.png', displayName: 'Brock' },
  { filename: 'bruno.png', displayName: 'Bruno' },
  { filename: 'bugcatcher.png', displayName: 'Bug Catcher' },
  { filename: 'burglar.png', displayName: 'Burglar' },
  { filename: 'channeler.png', displayName: 'Channeler' },
  { filename: 'cooltrainerf.png', displayName: 'Cool Trainer F' },
  { filename: 'cooltrainerm.png', displayName: 'Cool Trainer M' },
  { filename: 'cueball.png', displayName: 'Cue Ball' },
  { filename: 'engineer.png', displayName: 'Engineer' },
  { filename: 'erika.png', displayName: 'Erika' },
  { filename: 'fisher.png', displayName: 'Fisher' },
  { filename: 'gambler.png', displayName: 'Gambler' },
  { filename: 'gentleman.png', displayName: 'Gentleman' },
  { filename: 'giovanni.png', displayName: 'Giovanni' },
  { filename: 'hiker.png', displayName: 'Hiker' },
  { filename: 'jr.trainerf.png', displayName: 'Jr. Trainer F' },
  { filename: 'jr.trainerm.png', displayName: 'Jr. Trainer M' },
  { filename: 'juggler.png', displayName: 'Juggler' },
  { filename: 'koga.png', displayName: 'Koga' },
  { filename: 'lance.png', displayName: 'Lance' },
  { filename: 'lass.png', displayName: 'Lass' },
  { filename: 'lorelei.png', displayName: 'Lorelei' },
  { filename: 'lt.surge.png', displayName: 'Lt. Surge' },
  { filename: 'misty.png', displayName: 'Misty' },
  { filename: 'pokemaniac.png', displayName: 'Pokemaniac' },
  { filename: 'prof.oak.png', displayName: 'Prof. Oak' },
  { filename: 'psychic.png', displayName: 'Psychic' },
  { filename: 'rival1.png', displayName: 'Rival 1' },
  { filename: 'rival2.png', displayName: 'Rival 2' },
  { filename: 'rival3.png', displayName: 'Rival 3' },
  { filename: 'rocker.png', displayName: 'Rocker' },
  { filename: 'rocket.png', displayName: 'Rocket' },
  { filename: 'sabrina.png', displayName: 'Sabrina' },
  { filename: 'sailor.png', displayName: 'Sailor' },
  { filename: 'scientist.png', displayName: 'Scientist' },
  { filename: 'supernerd.png', displayName: 'Super Nerd' },
  { filename: 'swimmer.png', displayName: 'Swimmer' },
  { filename: 'tamer.png', displayName: 'Tamer' },
  { filename: 'youngster.png', displayName: 'Youngster' },
]

export function getTrainerAssets(): AssetEntry[] {
  return TRAINER_ASSETS.map((t) => ({
    category: 'trainer' as const,
    id: t.filename.replace(/\.png$/, ''),
    filename: t.filename,
    displayName: t.displayName,
  }))
}

interface NpcAssetDef {
  filename: string
  displayName: string
}

const NPC_ASSETS: NpcAssetDef[] = [
  { filename: 'agatha.png', displayName: 'Agatha' },
  { filename: 'balding_guy.png', displayName: 'Balding Guy' },
  { filename: 'beauty.png', displayName: 'Beauty' },
  { filename: 'bike_shop_clerk.png', displayName: 'Bike Shop Clerk' },
  { filename: 'biker.png', displayName: 'Biker' },
  { filename: 'bird.png', displayName: 'Bird' },
  { filename: 'blue.png', displayName: 'Blue' },
  { filename: 'boulder.png', displayName: 'Boulder' },
  { filename: 'bruno.png', displayName: 'Bruno' },
  { filename: 'brunette_girl.png', displayName: 'Brunette Girl' },
  { filename: 'captain.png', displayName: 'Captain' },
  { filename: 'channeler.png', displayName: 'Channeler' },
  { filename: 'clerk.png', displayName: 'Clerk' },
  { filename: 'clipboard.png', displayName: 'Clipboard' },
  { filename: 'cook.png', displayName: 'Cook' },
  { filename: 'cooltrainer_f.png', displayName: 'Cool Trainer F' },
  { filename: 'cooltrainer_m.png', displayName: 'Cool Trainer M' },
  { filename: 'daisy.png', displayName: 'Daisy' },
  { filename: 'fairy.png', displayName: 'Fairy' },
  { filename: 'fisher.png', displayName: 'Fisher' },
  { filename: 'fishing_guru.png', displayName: 'Fishing Guru' },
  { filename: 'fossil.png', displayName: 'Fossil' },
  { filename: 'gambler.png', displayName: 'Gambler' },
  { filename: 'gambler_asleep.png', displayName: 'Gambler (Asleep)' },
  { filename: 'gameboy_kid.png', displayName: 'Gameboy Kid' },
  { filename: 'gentleman.png', displayName: 'Gentleman' },
  { filename: 'giovanni.png', displayName: 'Giovanni' },
  { filename: 'girl.png', displayName: 'Girl' },
  { filename: 'gramps.png', displayName: 'Gramps' },
  { filename: 'granny.png', displayName: 'Granny' },
  { filename: 'guard.png', displayName: 'Guard' },
  { filename: 'gym_guide.png', displayName: 'Gym Guide' },
  { filename: 'hiker.png', displayName: 'Hiker' },
  { filename: 'koga.png', displayName: 'Koga' },
  { filename: 'lance.png', displayName: 'Lance' },
  { filename: 'link_receptionist.png', displayName: 'Link Receptionist' },
  { filename: 'little_boy.png', displayName: 'Little Boy' },
  { filename: 'little_girl.png', displayName: 'Little Girl' },
  { filename: 'lorelei.png', displayName: 'Lorelei' },
  { filename: 'middle_aged_man.png', displayName: 'Middle Aged Man' },
  { filename: 'middle_aged_woman.png', displayName: 'Middle Aged Woman' },
  { filename: 'mom.png', displayName: 'Mom' },
  { filename: 'monster.png', displayName: 'Monster' },
  { filename: 'mr_fuji.png', displayName: 'Mr. Fuji' },
  { filename: 'nurse.png', displayName: 'Nurse' },
  { filename: 'oak.png', displayName: 'Oak' },
  { filename: 'old_amber.png', displayName: 'Old Amber' },
  { filename: 'paper.png', displayName: 'Paper' },
  { filename: 'poke_ball.png', displayName: 'Pok\u00e9 Ball' },
  { filename: 'pokedex.png', displayName: 'Pok\u00e9dex' },
  { filename: 'red.png', displayName: 'Red' },
  { filename: 'red_bike.png', displayName: 'Red (Bike)' },
  { filename: 'rocker.png', displayName: 'Rocker' },
  { filename: 'rocket.png', displayName: 'Rocket' },
  { filename: 'safari_zone_worker.png', displayName: 'Safari Zone Worker' },
  { filename: 'sailor.png', displayName: 'Sailor' },
  { filename: 'scientist.png', displayName: 'Scientist' },
  { filename: 'seel.png', displayName: 'Seel' },
  { filename: 'silph_president.png', displayName: 'Silph President' },
  { filename: 'silph_worker_f.png', displayName: 'Silph Worker F' },
  { filename: 'silph_worker_m.png', displayName: 'Silph Worker M' },
  { filename: 'snorlax.png', displayName: 'Snorlax' },
  { filename: 'super_nerd.png', displayName: 'Super Nerd' },
  { filename: 'swimmer.png', displayName: 'Swimmer' },
  { filename: 'waiter.png', displayName: 'Waiter' },
  { filename: 'warden.png', displayName: 'Warden' },
  { filename: 'youngster.png', displayName: 'Youngster' },
]

export function getNpcAssets(): AssetEntry[] {
  return NPC_ASSETS.map((n) => ({
    category: 'npc' as const,
    id: n.filename.replace(/\.png$/, ''),
    filename: n.filename,
    displayName: n.displayName,
  }))
}

interface TilesetAssetDef {
  filename: string
  displayName: string
  tilePixelWidth: number
  tilePixelHeight: number
}

const TILESET_ASSETS: TilesetAssetDef[] = [
  { filename: 'overworld.png', displayName: 'Overworld', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'cavern.png', displayName: 'Cavern', tilePixelWidth: 128, tilePixelHeight: 40 },
  { filename: 'cemetery.png', displayName: 'Cemetery', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'club.png', displayName: 'Club', tilePixelWidth: 128, tilePixelHeight: 40 },
  { filename: 'facility.png', displayName: 'Facility', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'forest.png', displayName: 'Forest', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'gate.png', displayName: 'Gate', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'gym.png', displayName: 'Gym', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'house.png', displayName: 'House', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'interior.png', displayName: 'Interior', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'lab.png', displayName: 'Lab', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'lobby.png', displayName: 'Lobby', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'mansion.png', displayName: 'Mansion', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'plateau.png', displayName: 'Plateau', tilePixelWidth: 128, tilePixelHeight: 40 },
  { filename: 'pokecenter.png', displayName: 'Pok\u00e9center', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'reds_house.png', displayName: "Red's House", tilePixelWidth: 128, tilePixelHeight: 40 },
  { filename: 'ship.png', displayName: 'Ship', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'ship_port.png', displayName: 'Ship Port', tilePixelWidth: 128, tilePixelHeight: 48 },
  { filename: 'underground.png', displayName: 'Underground', tilePixelWidth: 128, tilePixelHeight: 16 },
]

export function getTilesetAssets(): AssetEntry[] {
  return TILESET_ASSETS.map((t) => ({
    category: 'tileset' as const,
    id: t.filename.replace(/\.png$/, ''),
    filename: t.filename,
    displayName: t.displayName,
    tilePixelWidth: t.tilePixelWidth,
    tilePixelHeight: t.tilePixelHeight,
    tileCount: (t.tilePixelWidth / 8) * (t.tilePixelHeight / 8),
  }))
}

// ── UI assets ──────────────────────────────────────────────────────────

interface UiAssetDef {
  filename: string
  displayName: string
}

const FONT_ASSETS: UiAssetDef[] = [
  { filename: 'font/font.png', displayName: 'Font' },
  { filename: 'font/font_extra.png', displayName: 'Font Extra' },
  { filename: 'font/font_battle_extra.png', displayName: 'Font Battle Extra' },
  { filename: 'font/AB.png', displayName: 'Font AB' },
  { filename: 'font/ED.png', displayName: 'Font ED' },
  { filename: 'font/P.png', displayName: 'Font P' },
]

const BATTLE_UI_ASSETS: UiAssetDef[] = [
  { filename: 'battle/balls.png', displayName: 'Balls' },
  { filename: 'battle/battle_hud_1.png', displayName: 'Battle HUD 1' },
  { filename: 'battle/battle_hud_2.png', displayName: 'Battle HUD 2' },
  { filename: 'battle/battle_hud_3.png', displayName: 'Battle HUD 3' },
  { filename: 'battle/ghost.png', displayName: 'Ghost' },
  { filename: 'battle/move_anim_0.png', displayName: 'Move Anim 0' },
  { filename: 'battle/move_anim_1.png', displayName: 'Move Anim 1' },
  { filename: 'battle/oldmanb.png', displayName: 'Old Man Battle' },
]

const PLAYER_ASSETS: UiAssetDef[] = [
  { filename: 'player/red.png', displayName: 'Red' },
  { filename: 'player/redb.png', displayName: 'Red Back' },
  { filename: 'player/shrink1.png', displayName: 'Shrink 1' },
  { filename: 'player/shrink2.png', displayName: 'Shrink 2' },
]

const TRAINER_CARD_ASSETS: UiAssetDef[] = [
  { filename: 'trainer_card/badge_numbers.png', displayName: 'Badge Numbers' },
  { filename: 'trainer_card/badges.png', displayName: 'Badges' },
  { filename: 'trainer_card/blank_leader_names.png', displayName: 'Blank Leader Names' },
  { filename: 'trainer_card/circle_tile.png', displayName: 'Circle Tile' },
  { filename: 'trainer_card/trainer_info.png', displayName: 'Trainer Info' },
]

const POKEDEX_ASSETS: UiAssetDef[] = [
  { filename: 'pokedex/pokedex.png', displayName: 'Pokédex' },
]

const TOWN_MAP_ASSETS: UiAssetDef[] = [
  { filename: 'town_map/mon_nest_icon.png', displayName: 'Mon Nest Icon' },
  { filename: 'town_map/town_map.png', displayName: 'Town Map' },
  { filename: 'town_map/town_map_cursor.png', displayName: 'Town Map Cursor' },
  { filename: 'town_map/up_arrow.png', displayName: 'Up Arrow' },
]

const TITLE_ASSETS: UiAssetDef[] = [
  { filename: 'title/blue_version.png', displayName: 'Blue Version' },
  { filename: 'title/gamefreak_inc.png', displayName: 'Game Freak Inc.' },
  { filename: 'title/player.png', displayName: 'Title Player' },
  { filename: 'title/pokemon_logo.png', displayName: 'Pokémon Logo' },
  { filename: 'title/red_version.png', displayName: 'Red Version' },
]

const CREDITS_ASSETS: UiAssetDef[] = [
  { filename: 'credits/the_end.png', displayName: 'The End' },
]

const SGB_ASSETS: UiAssetDef[] = [
  { filename: 'sgb/blue_border.png', displayName: 'Blue Border' },
  { filename: 'sgb/green_border.png', displayName: 'Green Border' },
  { filename: 'sgb/red_border.png', displayName: 'Red Border' },
]

function mapUiAssets(list: UiAssetDef[]): AssetEntry[] {
  return list.map((a) => ({
    category: 'ui' as const,
    id: a.filename.replace(/\.png$/, '').replace(/\//g, '-'),
    filename: a.filename,
    displayName: a.displayName,
  }))
}

export function getFontAssets(): AssetEntry[] { return mapUiAssets(FONT_ASSETS) }
export function getBattleAssets(): AssetEntry[] { return mapUiAssets(BATTLE_UI_ASSETS) }
export function getPlayerAssets(): AssetEntry[] { return mapUiAssets(PLAYER_ASSETS) }
export function getTrainerCardAssets(): AssetEntry[] { return mapUiAssets(TRAINER_CARD_ASSETS) }
export function getPokedexAssets(): AssetEntry[] { return mapUiAssets(POKEDEX_ASSETS) }
export function getTownMapAssets(): AssetEntry[] { return mapUiAssets(TOWN_MAP_ASSETS) }
export function getTitleAssets(): AssetEntry[] { return mapUiAssets(TITLE_ASSETS) }
export function getCreditsAssets(): AssetEntry[] { return mapUiAssets(CREDITS_ASSETS) }
export function getSgbAssets(): AssetEntry[] { return mapUiAssets(SGB_ASSETS) }

export function getUiAssets(): AssetEntry[] {
  return [
    ...getFontAssets(),
    ...getBattleAssets(),
    ...getPlayerAssets(),
    ...getTrainerCardAssets(),
    ...getPokedexAssets(),
    ...getTownMapAssets(),
    ...getTitleAssets(),
    ...getCreditsAssets(),
    ...getSgbAssets(),
  ]
}

// ── Effects assets ─────────────────────────────────────────────────────

interface EffectAssetDef {
  filename: string
  displayName: string
}

const OVERWORLD_FX_ASSETS: EffectAssetDef[] = [
  { filename: 'overworld/battle_transition.png', displayName: 'Battle Transition' },
  { filename: 'overworld/fishing_rod.png', displayName: 'Fishing Rod' },
  { filename: 'overworld/heal_machine.png', displayName: 'Heal Machine' },
  { filename: 'overworld/red_fish_back.png', displayName: 'Red Fish Back' },
  { filename: 'overworld/red_fish_front.png', displayName: 'Red Fish Front' },
  { filename: 'overworld/red_fish_side.png', displayName: 'Red Fish Side' },
  { filename: 'overworld/shadow.png', displayName: 'Shadow' },
  { filename: 'overworld/smoke.png', displayName: 'Smoke' },
  { filename: 'overworld/spinners.png', displayName: 'Spinners' },
]

const INTRO_ASSETS: EffectAssetDef[] = [
  { filename: 'intro/blue_jigglypuff_1.png', displayName: 'Blue Jigglypuff 1' },
  { filename: 'intro/blue_jigglypuff_2.png', displayName: 'Blue Jigglypuff 2' },
  { filename: 'intro/blue_jigglypuff_3.png', displayName: 'Blue Jigglypuff 3' },
  { filename: 'intro/gengar.png', displayName: 'Gengar' },
  { filename: 'intro/red_nidorino_1.png', displayName: 'Red Nidorino 1' },
  { filename: 'intro/red_nidorino_2.png', displayName: 'Red Nidorino 2' },
  { filename: 'intro/red_nidorino_3.png', displayName: 'Red Nidorino 3' },
]

const TRADE_ASSETS: EffectAssetDef[] = [
  { filename: 'trade/bubble.png', displayName: 'Bubble' },
  { filename: 'trade/cable_ball.png', displayName: 'Cable Ball' },
  { filename: 'trade/game_boy.png', displayName: 'Game Boy' },
  { filename: 'trade/link_cable.png', displayName: 'Link Cable' },
]

const SLOTS_ASSETS: EffectAssetDef[] = [
  { filename: 'slots/blue_slots_1.png', displayName: 'Blue Slots 1' },
  { filename: 'slots/blue_slots_2.png', displayName: 'Blue Slots 2' },
  { filename: 'slots/green_slots_1.png', displayName: 'Green Slots 1' },
  { filename: 'slots/green_slots_2.png', displayName: 'Green Slots 2' },
  { filename: 'slots/red_slots_1.png', displayName: 'Red Slots 1' },
  { filename: 'slots/red_slots_2.png', displayName: 'Red Slots 2' },
]

const SPLASH_ASSETS: EffectAssetDef[] = [
  { filename: 'splash/copyright.png', displayName: 'Copyright' },
  { filename: 'splash/falling_star.png', displayName: 'Falling Star' },
  { filename: 'splash/gamefreak_logo.png', displayName: 'Game Freak Logo' },
  { filename: 'splash/gamefreak_presents.png', displayName: 'Game Freak Presents' },
]

const EMOTES_ASSETS: EffectAssetDef[] = [
  { filename: 'emotes/happy.png', displayName: 'Happy' },
  { filename: 'emotes/question.png', displayName: 'Question' },
  { filename: 'emotes/shock.png', displayName: 'Shock' },
]

const ICONS_ASSETS: EffectAssetDef[] = [
  { filename: 'icons/bug.png', displayName: 'Bug' },
  { filename: 'icons/plant.png', displayName: 'Plant' },
  { filename: 'icons/quadruped.png', displayName: 'Quadruped' },
  { filename: 'icons/snake.png', displayName: 'Snake' },
]

function mapEffectAssets(list: EffectAssetDef[]): AssetEntry[] {
  return list.map((a) => ({
    category: 'effects' as const,
    id: a.filename.replace(/\.png$/, '').replace(/\//g, '-'),
    filename: a.filename,
    displayName: a.displayName,
  }))
}

export function getOverworldFxAssets(): AssetEntry[] { return mapEffectAssets(OVERWORLD_FX_ASSETS) }
export function getIntroAssets(): AssetEntry[] { return mapEffectAssets(INTRO_ASSETS) }
export function getTradeAssets(): AssetEntry[] { return mapEffectAssets(TRADE_ASSETS) }
export function getSlotsAssets(): AssetEntry[] { return mapEffectAssets(SLOTS_ASSETS) }
export function getSplashAssets(): AssetEntry[] { return mapEffectAssets(SPLASH_ASSETS) }
export function getEmotesAssets(): AssetEntry[] { return mapEffectAssets(EMOTES_ASSETS) }
export function getIconsAssets(): AssetEntry[] { return mapEffectAssets(ICONS_ASSETS) }

export function getEffectsAssets(): AssetEntry[] {
  return [
    ...getOverworldFxAssets(),
    ...getIntroAssets(),
    ...getTradeAssets(),
    ...getSlotsAssets(),
    ...getSplashAssets(),
    ...getEmotesAssets(),
    ...getIconsAssets(),
  ]
}

export function getAllPixelAssets(): AssetEntry[] {
  return [
    ...getPokemonFrontAssets(),
    ...getPokemonBackAssets(),
    ...getTrainerAssets(),
    ...getNpcAssets(),
    ...getTilesetAssets(),
    ...getUiAssets(),
    ...getEffectsAssets(),
  ]
}

export { speciesToSpriteName }
