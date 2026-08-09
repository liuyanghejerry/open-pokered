// ───────────────────────────────────────────────────────────────────────────
// pokeredDataCreate — record creation for the pokered data tables (species /
// moves). The editor's "New" flow POSTs a name; the server validates it and
// writes a template JSON. Templates must satisfy `pokered-data/build.rs`,
// which regenerates the `Species`/`MoveId` enums and the embedded tables
// from these files on the next `cargo build` — so the name also becomes a
// Rust enum variant, hence the strict PascalCase identifier rule (matching
// `schemas/pokemon.schema.json` / `schemas/move.schema.json` patterns).
// ───────────────────────────────────────────────────────────────────────────

/** Identifier rule shared by the pokemon/move JSON schemas and build.rs. */
export const RECORD_NAME_RE = /^[A-Z][A-Za-z]+$/

export interface NameError {
  status: number
  error: string
}

/** Validate a new species/move name against the identifier rule, the
 *  reserved `None` variant, and the existing table (case-insensitive). */
export function validateNewRecordName(name: string, existingNames: string[]): NameError | null {
  if (!RECORD_NAME_RE.test(name)) {
    return {
      status: 400,
      error: `Invalid name "${name}" (must match /^[A-Z][A-Za-z]+$/ — PascalCase, no digits/underscores)`,
    }
  }
  if (name === 'None') {
    return { status: 400, error: 'Name "None" is reserved (it is the empty-slot enum variant).' }
  }
  const clash = existingNames.find(n => n.toLowerCase() === name.toLowerCase())
  if (clash) {
    return { status: 409, error: `A record named "${clash}" already exists.` }
  }
  return null
}

/** Default species record. Every field is required by
 *  `pokered-data/build.rs::generate_pokemon_and_evos_data`. */
export function pokemonTemplate(species: string): Record<string, unknown> {
  return {
    $schema: '../schemas/pokemon.schema.json',
    species,
    baseStats: { hp: 50, attack: 50, defense: 50, speed: 50, special: 50 },
    type1: 'Normal',
    type2: 'Normal',
    catchRate: 45,
    baseExp: 64,
    growthRate: 'MediumFast',
    initialMoves: ['None', 'None', 'None', 'None'],
    tmHmFlags: [0, 0, 0, 0, 0, 0, 0],
    pokedex: {
      category: '???',
      heightFeet: 1,
      heightInches: 0,
      weightDecipounds: 100,
      flavorTextPages: ['A newly added Pokemon.'],
    },
    evolutions: [],
    learnset: [],
  }
}

/** Default move record. Every field is required by
 *  `pokered-data/build.rs::generate_move_data`. */
export function moveTemplate(id: string): Record<string, unknown> {
  return {
    $schema: '../schemas/move.schema.json',
    id,
    effect: 'NoAdditionalEffect',
    power: 40,
    type: 'Normal',
    accuracy: 100,
    pp: 35,
  }
}

/** Default item record (shape matches `data/items/Potion.json`). `build.rs`
 *  only reads `id` / `name` / `price` / `key_item`; the rest is editor UI
 *  data. The item must ALSO be appended to `data/items/item_list.json` (the
 *  enum-order source) or the build won't pick it up. */
export function itemTemplate(id: string): Record<string, unknown> {
  return {
    id,
    name: id.toUpperCase(),
    price: 300,
    category: 'Medicine',
    effect: { type: 'Heal', params: { amount: 20 } },
    held_effect: null,
    berry: null,
    use_outside_battle: true,
    use_in_battle: true,
    consume: true,
    key_item: false,
    sellable: true,
    description: '',
    tags: [],
  }
}

/** Serialize `data/items/item_list.json` in its canonical compact style —
 *  five names per line — so appending a new item doesn't rewrite the whole
 *  file into an expanded layout. Names are restricted to `[A-Za-z]` by
 *  [`validateNewRecordName`], so no JSON escaping is needed. */
export function formatItemList(items: string[]): string {
  const lines: string[] = ['{', '  "items": [']
  for (let i = 0; i < items.length; i += 5) {
    const chunk = items.slice(i, i + 5).map(s => `"${s}"`).join(', ')
    lines.push(`    ${chunk}${i + 5 < items.length ? ',' : ''}`)
  }
  lines.push('  ],', `  "count": ${items.length}`, '}')
  return lines.join('\n') + '\n'
}
