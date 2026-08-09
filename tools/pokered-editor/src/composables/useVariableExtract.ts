/**
 * Utility for extracting and providing mock values for template variables
 * found in UI layout JSON files (crates/pokered-data/ui_layouts/*.json).
 *
 * Template variables are strings like {name}, {balance}, {items} that appear
 * in element values within layout JSON. This module can:
 * 1. Scan any layout JSON and extract all unique variables
 * 2. Infer the likely type of each variable from its name
 * 3. Provide default mock values for preview/rendering
 */

export interface TemplateVariable {
  key: string
  type: 'string' | 'number' | 'boolean' | 'list'
  defaultValue: string
  currentValue: string
  usedIn: string[]
}

/**
 * Recursively scan a layout JSON object's elements array and extract all
 * {variable} template patterns appearing in string values.
 *
 * @param json - The parsed layout JSON (expected to have an `elements` array)
 * @returns Sorted array of unique TemplateVariable instances
 */
export function extractVariables(json: any): TemplateVariable[] {
  if (!json?.elements) return []

  const found = new Map<string, TemplateVariable>()
  const varRegex = /\{(\w+)\}/g

  function scan(obj: any, parentId?: string) {
    if (!obj || typeof obj !== 'object') return
    if (Array.isArray(obj)) {
      obj.forEach(v => scan(v, parentId))
      return
    }

    for (const value of Object.values(obj)) {
      if (typeof value === 'string' && value.includes('{')) {
        let m
        while ((m = varRegex.exec(value)) !== null) {
          const varname = m[1]
          const existing = found.get(varname)
          if (existing) {
            if (parentId && !existing.usedIn.includes(parentId)) {
              existing.usedIn.push(parentId)
            }
          } else {
            found.set(varname, {
              key: varname,
              type: inferType(varname),
              defaultValue: '',
              currentValue: '',
              usedIn: parentId ? [parentId] : [],
            })
          }
        }
      } else if (typeof value === 'object') {
        const elId = (obj as any).id || (obj as any).element_type || (obj as any).type || parentId
        scan(value, elId)
      }
    }
  }

  scan(json.elements)
  return Array.from(found.values()).sort((a, b) => a.key.localeCompare(b.key))
}

/**
 * Infer the data type of a template variable from its key name using
 * common naming conventions found in the game's UI layout system.
 *
 * - Suffix patterns like _items, _list, _options → 'list'
 * - Suffix patterns like _count, _num, _price → 'number'
 * - Prefix patterns like show_, has_ → 'boolean'
 * - Everything else → 'string'
 */
function inferType(key: string): 'string' | 'number' | 'boolean' | 'list' {
  if (/^(.*_)?(items|list|entries|options|members)$/.test(key)) return 'list'
  if (/^(.*_)?(count|num|balance|price|power|cursor|index|id)$/.test(key)) return 'number'
  if (/^(show_|has_)/.test(key)) return 'boolean'
  return 'string'
}

/**
 * Default mock values for each layout menu, mapping menu_name → variable_key → value.
 *
 * Only includes non-obvious variables (strings show as readable text, numbers as
 * realistic values, booleans as 'true', lists as multi-line item strings).
 * Variables not listed here will get their key name as the default string value,
 * '0' for numbers, or 'true' for booleans.
 */
export const DEFAULT_MOCK_VALUES: Record<string, Record<string, string>> = {
  pokedex: {
    name: "PIKACHU",
    species: "MOUSE POKEMON",
    dex_num: "25",
    height_ft: "1",
    height_in: "4",
    weight_tenths: "132",
    description_lines: "When several\nof these POKEMON\ngather, their\nelectricity could\nbuild up.",
  },
  pokedex_list: {
    seen_count: "30",
    owned_count: "2",
    selected_name: "BULBASAUR",
    selected_num: "001",
    sprite_index: "0",
  },
  mart: {
    balance: "9999",
    shop_items: "POTION 300\nANTIDOTE 100\nPOKE BALL 200",
    sell_items: "POTION 150 3\nANTIDOTE 50 1",
    confirm_message: "Are you sure?",
    item_name: "POTION",
    quantity: "3",
    unit_price: "300",
    result_message: "Here you are!",
    cursor_y: "2",
  },
  bag: {
    bag_items: "POKE BALL 5\nPOTION 3\nANTIDOTE 2\nFULL HEAL 2",
  },
  battle_text: {
    text: "Wild PIKACHU appeared!",
  },
  dialog: {
    text: "Hello there! Welcome to the world of POKEMON!",
  },
  oak_speech: {
    text: "Hello there! Welcome to the world of POKEMON! My name is OAK!",
  },
  naming: {
    prompt_text: "YOUR NAME?",
    entered_name: "ASH",
    gender_symbol: "♂",
    char_cursor_x: "3",
  },
  save: {
    player_name: "ASH",
    play_time: "12:34",
    badges: "2",
    seen_count: "30",
    save_message: "Save complete!",
  },
  stats: {
    name: "PIKACHU",
    level: "25",
    dex_num: "25",
    status: "OK",
    ot_name: "ASH",
  },
  battle_main: {
    cursor_tx: "1",
    cursor_ty: "0",
  },
  options: {
    text_speed_options: "FAST\nMID\nSLOW",
    animation_options: "ON\nOFF",
    style_options: "SHIFT\nSET",
  },
  party: {
    title: "PARTY",
  },
  title: {
    title_text: "POKEMON RED",
    copyright: "(C)2024 GAME FREAK inc.",
  },
  main: {
    items: "NEW GAME\nOPTION",
  },
  start: {
    items: "POKEDEX\nPOKEMON\nITEM\nASH\nSAVE\nOPTION\nEXIT",
  },
}
