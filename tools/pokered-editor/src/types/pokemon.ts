export interface BaseStats {
  hp: number
  attack: number
  defense: number
  speed: number
  special: number
}

export type EvolutionMethod = 'level' | 'item' | 'trade'

export interface Evolution {
  method: EvolutionMethod
  species: string
  level?: number
  item?: string
  minLevel?: number
}

export interface LevelUpMove {
  level: number
  moveId: string
}

export interface PokedexInfo {
  category: string
  heightFeet: number
  heightInches: number
  weightDecipounds: number
  flavorTextPages: string[]
}

export interface PokemonFile {
  species: string
  baseStats: BaseStats
  type1: string
  type2: string
  catchRate: number
  baseExp: number
  growthRate: string
  initialMoves: [string, string, string, string]
  tmHmFlags: number[]
  pokedex: PokedexInfo
  evolutions: Evolution[]
  learnset: LevelUpMove[]
}

// Maps a JSON 'species' value (PascalCase, matching `Species` enum variant)
// to the actual sprite filename stem on disk under `gfx/pokemon/front` and
// `gfx/pokemon/back`. The base rule mirrors species_to_sprite_name() in
// pokered-app/src/render/mod.rs (lowercase + strip space/hyphen/apostrophe),
// with one override: MrMime → 'mr.mime' because the on-disk PNG keeps the
// dot — see `gfx/pokemon/front/mr.mime.png`.
export function speciesToSpriteName(species: string): string {
  if (species === 'MrMime') return 'mr.mime'
  return species.toLowerCase().replace(/[ \-']/g, '')
}


export interface MoveFile {
  id: string
  effect: string
  power: number
  type: string
  accuracy: number
  pp: number
}
