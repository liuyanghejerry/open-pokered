import { describe, expect, it } from 'vitest'
import {
  validateNewRecordName,
  pokemonTemplate,
  moveTemplate,
  itemTemplate,
  formatItemList,
  RECORD_NAME_RE,
} from './pokeredDataCreate'

describe('validateNewRecordName', () => {
  const existing = ['Bulbasaur', 'Abra', 'Thundershock']

  it('accepts a valid PascalCase name', () => {
    expect(validateNewRecordName('NewMon', existing)).toBeNull()
    expect(validateNewRecordName('NewMove', existing)).toBeNull()
  })

  it('rejects names that are not valid Rust identifiers / schema patterns', () => {
    // lowercase start, digits, underscores, empty, spaces — all rejected.
    expect(validateNewRecordName('pikachu', existing)?.status).toBe(400)
    expect(validateNewRecordName('Mon2', existing)?.status).toBe(400)
    expect(validateNewRecordName('New_Mon', existing)?.status).toBe(400)
    expect(validateNewRecordName('', existing)?.status).toBe(400)
    expect(validateNewRecordName('New Mon', existing)?.status).toBe(400)
  })

  it('rejects the reserved None variant', () => {
    const r = validateNewRecordName('None', existing)
    expect(r?.status).toBe(400)
    expect(r?.error).toMatch(/reserved/)
  })

  it('rejects duplicates case-insensitively', () => {
    const r = validateNewRecordName('BULBASAUR', existing)
    expect(r?.status).toBe(409)
    expect(r?.error).toMatch(/already exists/)
  })
})

describe('templates satisfy the build.rs contract', () => {
  it('pokemonTemplate covers every field build.rs reads', () => {
    const t = pokemonTemplate('NewMon')
    // generate_pokemon_and_evos_data requires: species, baseStats{h,a,d,s,sp},
    // type1, type2, catchRate, baseExp, growthRate, initialMoves[4],
    // tmHmFlags[7], pokedex{category,heightFeet,heightInches,weightDecipounds,
    // flavorTextPages}, evolutions[], learnset[].
    expect(t.species).toBe('NewMon')
    expect(t.baseStats).toMatchObject({ hp: 50, attack: 50, defense: 50, speed: 50, special: 50 })
    expect(t.initialMoves).toHaveLength(4)
    expect(t.tmHmFlags).toHaveLength(7)
    const dex = t.pokedex as Record<string, unknown>
    expect(dex.flavorTextPages).toEqual(['A newly added Pokemon.'])
    expect(t.evolutions).toEqual([])
    expect(t.learnset).toEqual([])
    // The name itself must pass the identifier rule (it becomes an enum variant).
    expect(RECORD_NAME_RE.test(t.species as string)).toBe(true)
  })

  it('moveTemplate covers every field build.rs reads', () => {
    const t = moveTemplate('NewMove')
    // generate_move_data requires: id, effect, power, type, accuracy, pp.
    expect(t).toEqual({
      $schema: '../schemas/move.schema.json',
      id: 'NewMove',
      effect: 'NoAdditionalEffect',
      power: 40,
      type: 'Normal',
      accuracy: 100,
      pp: 35,
    })
    expect(RECORD_NAME_RE.test(t.id as string)).toBe(true)
  })

  it('itemTemplate covers every field build.rs reads', () => {
    const t = itemTemplate('NewItem')
    // generate_item_data requires: id, name, price, key_item.
    expect(t.id).toBe('NewItem')
    expect(t.name).toBe('NEWITEM')
    expect(t.price).toBe(300)
    expect(t.key_item).toBe(false)
    // The rest is editor UI data (shape matches data/items/Potion.json).
    expect(t.category).toBe('Medicine')
    expect(t.effect).toMatchObject({ type: 'Heal' })
    expect(t.tags).toEqual([])
    expect(RECORD_NAME_RE.test(t.id as string)).toBe(true)
  })

  it('formatItemList writes the canonical 5-per-line layout', () => {
    const items = ['A', 'B', 'C', 'D', 'E', 'F']
    const out = formatItemList(items)
    expect(out).toBe(
      '{\n' +
      '  "items": [\n' +
      '    "A", "B", "C", "D", "E",\n' +
      '    "F"\n' +
      '  ],\n' +
      '  "count": 6\n' +
      '}\n',
    )
    // Round-trips through JSON.parse.
    expect(JSON.parse(out)).toEqual({ items, count: 6 })
  })
})
