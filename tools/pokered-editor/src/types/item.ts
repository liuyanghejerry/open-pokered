export interface EffectParams {
  [key: string]: any
}

export interface ActiveEffect {
  type: string
  params: EffectParams
}

export interface HeldEffect {
  trigger: string
  type: string
  params: EffectParams
}

export interface BerryData {
  growth_time: number
  max_yield: number
  min_yield: number
  flavor_spicy: number
  flavor_dry: number
  flavor_sweet: number
  flavor_bitter: number
  flavor_sour: number
  smoothness: number
  natural_gift_power: number
  natural_gift_type: string
}

export interface ItemFile {
  id: string
  name: string
  price: number
  category: string
  effect: ActiveEffect | null
  held_effect: HeldEffect | null
  berry: BerryData | null
  use_outside_battle: boolean
  use_in_battle: boolean
  consume: boolean
  key_item: boolean
  sellable: boolean
  description: string
  tags: string[]
}

export interface CategoryDef {
  id: string
  label: string
  icon_tile: number
  color: string
}

export interface EffectParamDef {
  name: string
  type: 'number' | 'select' | 'stat' | 'species' | 'move' | 'pokemonType' | 'string'
  options?: string[]
  min?: number
  max?: number
  default?: any
}

export interface EffectDef {
  type: string
  params: EffectParamDef[]
}

export interface EffectRegistry {
  active_effects: EffectDef[]
  held_effects: EffectDef[]
  held_triggers: string[]
}

export interface ShopData {
  id: string
  name: string
  items: string[]
}

export interface ItemListJson {
  items: string[]
  count: number
}

// ── Effect Registry Data ──────────────────────────────────────────────────

const STONE_OPTS = ['Fire', 'Water', 'Thunder', 'Leaf', 'Moon']
const BATTLE_FLAG_OPTS = ['protect', 'guard_spec', 'focus']
const BATTLE_STAT_OPTS = ['attack', 'defense', 'speed', 'special']
const POKEMON_TYPE_OPTS = [
  'Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock',
  'Bug', 'Ghost', 'Fire', 'Water', 'Grass', 'Electric',
  'Psychic', 'Ice', 'Dragon',
]
const STAT_OPTS = ['hp', 'attack', 'defense', 'speed', 'special']
const STATUS_OPTS = ['poison', 'burn', 'freeze', 'paralysis', 'sleep']

export const EFFECT_REGISTRY: EffectRegistry = {
  held_triggers: ['None', 'Passive', 'OnTurnEnd', 'OnStatusInflicted', 'OnAttacking', 'OnDamaged'],

  active_effects: [
    { type: 'None', params: [] },
    { type: 'Heal', params: [{ name: 'amount', type: 'number', min: 1, max: 255, default: 20 }] },
    { type: 'CurePoison', params: [] },
    { type: 'CureBurn', params: [] },
    { type: 'CureFreeze', params: [] },
    { type: 'CureParalysis', params: [] },
    { type: 'CureSleep', params: [] },
    { type: 'CureAllStatus', params: [] },
    { type: 'FullHeal', params: [] },
    { type: 'FullRestore', params: [] },
    {
      type: 'Revive',
      params: [{ name: 'full', type: 'select', options: ['true', 'false'], default: 'false' }],
    },
    {
      type: 'PpRestore',
      params: [
        { name: 'all', type: 'select', options: ['true', 'false'], default: 'false' },
        { name: 'amount', type: 'number', min: 0, max: 40, default: 10 },
      ],
    },
    { type: 'PpUp', params: [] },
    { type: 'RareCandy', params: [{ name: 'levels', type: 'number', min: 1, max: 100, default: 1 }] },
    {
      type: 'EvolutionStone',
      params: [{ name: 'stone', type: 'select', options: STONE_OPTS, default: 'Fire' }],
    },
    {
      type: 'Vitamin',
      params: [
        { name: 'statIdx', type: 'select', options: ['0', '1', '2', '3', '4'], default: '0' },
        { name: 'amount', type: 'number', min: 1, max: 255, default: 10 },
      ],
    },
    {
      type: 'BattleStat',
      params: [
        { name: 'stage', type: 'select', options: BATTLE_STAT_OPTS, default: 'attack' },
      ],
    },
    {
      type: 'BattleFlag',
      params: [{ name: 'flag', type: 'select', options: BATTLE_FLAG_OPTS, default: 'protect' }],
    },
    { type: 'Escape', params: [] },
    { type: 'PokeDoll', params: [] },
  ],

  held_effects: [
    { type: 'None', params: [] },
    {
      type: 'Leftovers',
      params: [{ name: 'amount', type: 'number', min: 1, max: 255, default: 15 }],
    },
    {
      type: 'BoostType',
      params: [
        { name: 'type', type: 'pokemonType', options: POKEMON_TYPE_OPTS, default: 'Normal' },
        { name: 'multiplier', type: 'number', min: 10, max: 30, default: 12 },
      ],
    },
    {
      type: 'BoostStat',
      params: [
        { name: 'stat', type: 'stat', options: STAT_OPTS, default: 'speed' },
        { name: 'multiplier', type: 'number', min: 10, max: 30, default: 15 },
      ],
    },
    {
      type: 'HealStatus',
      params: [{ name: 'condition', type: 'select', options: STATUS_OPTS, default: 'poison' }],
    },
    {
      type: 'PreventStatus',
      params: [{ name: 'condition', type: 'select', options: STATUS_OPTS, default: 'poison' }],
    },
    { type: 'MoneyBoost', params: [] },
    {
      type: 'ExpBoost',
      params: [{ name: 'multiplier', type: 'number', min: 10, max: 30, default: 15 }],
    },
    { type: 'FocusBand', params: [] },
    { type: 'PreventEscape', params: [] },
    {
      type: 'BoostCrit',
      params: [{ name: 'stages', type: 'number', min: 1, max: 3, default: 1 }],
    },
  ],
}

export function defaultEffectParams(effectType: string, kind: 'active' | 'held'): Record<string, any> {
  const list = kind === 'active' ? EFFECT_REGISTRY.active_effects : EFFECT_REGISTRY.held_effects
  const def = list.find(e => e.type === effectType)
  if (!def) return {}
  const out: Record<string, any> = {}
  for (const p of def.params) {
    if (p.default !== undefined) out[p.name] = p.default
    else if (p.type === 'number') out[p.name] = 0
    else out[p.name] = ''
  }
  return out
}

export function createDefaultBerry(): BerryData {
  return {
    growth_time: 4,
    max_yield: 5,
    min_yield: 2,
    flavor_spicy: 0,
    flavor_dry: 0,
    flavor_sweet: 0,
    flavor_bitter: 0,
    flavor_sour: 0,
    smoothness: 0,
    natural_gift_power: 60,
    natural_gift_type: 'Normal',
  }
}
