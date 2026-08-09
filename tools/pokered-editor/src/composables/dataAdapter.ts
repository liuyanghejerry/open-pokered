// ───────────────────────────────────────────────────────────────────────────
// Data access adapter — the single seam between the editor stores and the
// data backend.
//
//   dev   (npm run dev / Electron): every request goes to the Node /api
//         backend exactly as before.
//   static (GitHub Pages): no /api backend. Reads resolve from the IndexedDB
//         delta store first, then the bundled baseline files under
//         <base>/data/ (produced by the deploy workflow); writes persist to
//         the delta store only.
//
// Stores keep calling `fetch('/api/...')`; they swap the global fetch for
// `dataFetch` (same signature, returns a `Response`). Detection is automatic
// and cached after the first probe.
// ───────────────────────────────────────────────────────────────────────────

import { getDelta, putDelta, putDeltaBlob, getDeltaBlob, listDeltas } from './useDataStore'

let backendOk: boolean | null = null

/** Probe the /api backend once; `true` = dev/Electron mode. */
export async function detectBackend(): Promise<boolean> {
  if (backendOk !== null) return backendOk
  try {
    const res = await fetch('/api/maps')
    backendOk = res.ok
  } catch {
    backendOk = false
  }
  return backendOk
}

/** Whether we're currently in static mode (no /api backend). */
export async function isStaticMode(): Promise<boolean> {
  return !(await detectBackend())
}

function jsonResponse(content: string, status = 200): Response {
  return new Response(content, {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

/** text/plain response — used for .gui layout sources (mirrors the dev server). */
function textResponse(content: string, status = 200): Response {
  return new Response(content, {
    status,
    headers: { 'Content-Type': 'text/plain' },
  })
}

function errorResponse(status: number, message: string): Response {
  return new Response(JSON.stringify({ error: message }), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

const FAMILIES = ['trainers', 'pokemon', 'moves'] as const

/**
 * Static-mode fetch: routes /api/* onto the delta store + bundled baselines.
 * Falls back to the real /api when a route isn't data-backed (AI, sprites…).
 */
async function staticFetch(url: string, init?: RequestInit): Promise<Response> {
  const method = (init?.method ?? 'GET').toUpperCase()
  const path = url.replace(/^\/api\//, '')
  const base = import.meta.env.BASE_URL

  // Map files: /api/maps, /api/maps/{name}/map.json|map.blk|script_config.json|script.scene|script.js
  const mapMatch = path.match(/^maps\/([^/]+)\/(map\.json|map\.blk|script_config\.json|script\.scene|script\.js)$/)
  if (mapMatch) {
    const name = mapMatch[1]
    const file = mapMatch[2]
    const deltaKey = `maps/${name}/${file}`
    if (method === 'PUT') {
      await putDelta(deltaKey, String(init?.body ?? ''))
      return jsonResponse('{}')
    }
    if (method === 'GET') {
      const delta = await getDelta(deltaKey)
      if (delta !== null) return jsonResponse(delta)
      // blk baselines are bundled as JSON arrays (server serves them as JSON).
      const baseFile = file === 'map.blk' ? 'map.blk.json' : file
      const res = await fetch(`${base}data/maps/${name}/${baseFile}`)
      return res.ok ? res : errorResponse(404, `no baseline for ${deltaKey}`)
    }
  }
  if (path === 'maps') {
    const res = await fetch(`${base}maps.json`)
    return res.ok ? res : errorResponse(404, 'no map list')
  }

  // Family files: /api/{trainers|pokemon|moves}, /api/{family}/{Id}
  for (const family of FAMILIES) {
    if (path === family) {
      // Creating records needs the dev server (it owns the templates + the
      // real pokered-data filesystem); the static build is read-mostly.
      if (method === 'POST') {
        return errorResponse(501, `Creating new ${family} records requires the dev server (npm run dev).`)
      }
      const res = await fetch(`${base}data/list/${family}.json`)
      return res.ok ? res : errorResponse(404, `no ${family} list`)
    }
    const member = path.match(new RegExp(`^${family}/([^/]+)$`))
    if (member) {
      const id = member[1]
      const deltaKey = `${family}/${id}.json`
      if (method === 'PUT') {
        await putDelta(deltaKey, String(init?.body ?? ''))
        return jsonResponse('{}')
      }
      const delta = await getDelta(deltaKey)
      if (delta !== null) return jsonResponse(delta)
      const res = await fetch(`${base}data/${family}/${id}.json`)
      return res.ok ? res : errorResponse(404, `no baseline for ${id}.json`)
    }
  }

  // Items: /api/items, /api/items/{Id}
  if (path === 'items') {
    if (method === 'POST') {
      return errorResponse(501, 'Creating new items requires the dev server (npm run dev).')
    }
    const res = await fetch(`${base}data/list/items.json`)
    return res.ok ? res : errorResponse(404, 'no items list')
  }
  const itemMatch = path.match(/^items\/([^/]+)$/)
  if (itemMatch) {
    const id = itemMatch[1]
    const deltaKey = `items/${id}.json`
    if (method === 'PUT') {
      await putDelta(deltaKey, String(init?.body ?? ''))
      return jsonResponse('{}')
    }
    const delta = await getDelta(deltaKey)
    if (delta !== null) return jsonResponse(delta)
    const res = await fetch(`${base}data/items/${id}.json`)
    return res.ok ? res : errorResponse(404, `no baseline for ${id}.json`)
  }

  // UI layouts: /api/ui-layouts, /api/ui-layouts/{Name}
  // Static baselines ship the .gui DSL source (source of truth, compiled
  // in-browser by the WASM preview bridge); legacy v1 JSON baselines are
  // still honored as a fallback. Mirrors the dev server, which serves the
  // .gui file as text/plain and v1 JSON as application/json.
  if (path === 'ui-layouts') {
    const res = await fetch(`${base}data/list/ui_layouts.json`)
    return res.ok ? res : errorResponse(404, 'no layouts list')
  }
  const layoutMatch = path.match(/^ui-layouts\/([^/]+)$/)
  if (layoutMatch) {
    const name = layoutMatch[1]
    if (method === 'PUT') {
      const contentType = init?.headers instanceof Headers
        ? init.headers.get('content-type') ?? ''
        : ((init?.headers as Record<string, string> | undefined)?.['Content-Type'] ?? '')
      const ext = contentType.includes('text/plain') ? 'gui' : 'json'
      await putDelta(`ui_layouts/${name}.${ext}`, String(init?.body ?? ''))
      return jsonResponse('{}')
    }
    // .gui delta/baseline first (DSL is the default editing mode)…
    const guiDelta = await getDelta(`ui_layouts/${name}.gui`)
    if (guiDelta !== null) return textResponse(guiDelta)
    const guiRes = await fetch(`${base}data/ui_layouts/${name}.gui`)
    if (guiRes.ok) {
      // Normalize to text/plain: layoutStore's loadLayout branches on the
      // Content-Type (text/plain ⇒ .gui DSL, else JSON). Static hosts serve
      // the unknown .gui extension as application/octet-stream (GitHub Pages
      // does), which used to push the DSL source into the JSON branch and
      // fail with "Unexpected token 's', "screen Bat"... is not valid JSON".
      // Mirrors the dev server, which explicitly sets text/plain for .gui.
      return textResponse(await guiRes.text())
    }
    // …then legacy v1 JSON.
    const jsonDelta = await getDelta(`ui_layouts/${name}.json`)
    if (jsonDelta !== null) return jsonResponse(jsonDelta)
    const res = await fetch(`${base}data/ui_layouts/${name}.json`)
    return res.ok ? res : errorResponse(404, `no baseline for ${name}.gui/.json`)
  }

  // Town-map extras: /api/town-map-extras (full object, replace on PUT).
  if (path === 'town-map-extras') {
    const deltaKey = 'town_map_extras.json'
    if (method === 'PUT') {
      await putDelta(deltaKey, String(init?.body ?? ''))
      return jsonResponse('{}')
    }
    const delta = await getDelta(deltaKey)
    if (delta !== null) return jsonResponse(delta)
    const res = await fetch(`${base}data/town_map_extras.json`)
    // The dev server returns {} when the file doesn't exist — match that.
    return res.ok ? res : jsonResponse('{}')
  }

  // User-created tilesets: /api/tileset-extras (read-only baseline).
  if (path === 'tileset-extras') {
    const res = await fetch(`${base}data/tileset_extras.json`)
    return res.ok ? res : jsonResponse('{}')
  }

  // Blocksets: GET /api/blocksets merges the baseline with per-tileset
  // sparse overrides (`{ blocks: { id: number[16] } }`); PUT /api/blocksets/:name
  // persists the sparse body as a delta.
  if (path === 'blocksets') {
    const res = await fetch(`${base}data/blocksets.json`)
    if (!res.ok) return errorResponse(404, 'no blocksets baseline')
    const baseline = await res.json() as Record<string, Record<string, number[]>>
    for (const [name, body] of await deltasWithPrefix('blocksets_overrides/')) {
      const key = name.replace(/^blocksets_overrides\//, '').replace(/\.json$/, '')
      let blocks: Record<string, number[]>
      try { blocks = JSON.parse(body).blocks ?? {} } catch { continue }
      const target = baseline[key] ?? {}
      for (const [id, tiles] of Object.entries(blocks)) target[id] = tiles
      baseline[key] = target
    }
    return jsonResponse(JSON.stringify(baseline))
  }
  const blocksetMatch = path.match(/^blocksets\/([A-Za-z][A-Za-z0-9_]*)$/)
  if (blocksetMatch && method === 'PUT') {
    await putDelta(`blocksets_overrides/${blocksetMatch[1]}.json`, String(init?.body ?? ''))
    return jsonResponse('{}')
  }

  // Passable tiles: GET /api/passable-tiles merges the baseline with
  // per-tileset overrides (`{ tiles: number[] }`); PUT /api/passable-tiles/:name
  // persists the override as a delta.
  if (path === 'passable-tiles') {
    const res = await fetch(`${base}data/passable_tiles.json`)
    if (!res.ok) return errorResponse(404, 'no passable-tiles baseline')
    const baseline = await res.json() as Record<string, number[]>
    for (const [name, body] of await deltasWithPrefix('passable_tiles_overrides/')) {
      const key = name.replace(/^passable_tiles_overrides\//, '').replace(/\.json$/, '')
      let tiles: number[]
      try { tiles = JSON.parse(body).tiles ?? [] } catch { continue }
      baseline[key] = tiles
    }
    return jsonResponse(JSON.stringify(baseline))
  }
  const passableMatch = path.match(/^passable-tiles\/([A-Za-z][A-Za-z0-9_]*)$/)
  if (passableMatch && method === 'PUT') {
    await putDelta(`passable_tiles_overrides/${passableMatch[1]}.json`, String(init?.body ?? ''))
    return jsonResponse('{}')
  }

  // Shops: /api/shops, /api/shops/{Id}
  if (path === 'shops') {
    const res = await fetch(`${base}data/list/shops.json`)
    return res.ok ? res : errorResponse(404, 'no shops list')
  }
  const shopMatch = path.match(/^shops\/([^/]+)$/)
  if (shopMatch) {
    const id = shopMatch[1]
    const deltaKey = `shops/${id}.json`
    if (method === 'PUT') {
      await putDelta(deltaKey, String(init?.body ?? ''))
      return jsonResponse('{}')
    }
    const delta = await getDelta(deltaKey)
    if (delta !== null) return jsonResponse(delta)
    const res = await fetch(`${base}data/shops/${id}.json`)
    return res.ok ? res : errorResponse(404, `no baseline for ${id}.json`)
  }

  // Anything else (AI assistant, sprites…) has no static path.
  return errorResponse(503, `no static data for ${url}`)
}

/** All deltas whose path starts with `prefix`, as [path, content] pairs
 *  (text entries only — binary /gfx deltas are handled by staticGfxFetch). */
async function deltasWithPrefix(prefix: string): Promise<[string, string][]> {
  const all = await listDeltas()
  return all
    .filter((d) => d.path.startsWith(prefix) && typeof d.content === 'string')
    .map((d) => [d.path, d.content as string])
}

/**
 * Static-mode fetch for /gfx assets: GET serves the IndexedDB delta when one
 * exists (an edited sprite / tileset), otherwise the bundled baseline; PUT
 * persists the new PNG as a binary delta.
 */
async function staticGfxFetch(url: string, init?: RequestInit): Promise<Response> {
  const method = (init?.method ?? 'GET').toUpperCase()
  const rel = url.replace(/^\/gfx\//, '')
  const deltaKey = `gfx/${rel}`
  const base = import.meta.env.BASE_URL

  if (method === 'PUT') {
    const body = init?.body
    if (body instanceof Blob) {
      await putDeltaBlob(deltaKey, body)
    } else {
      await putDelta(deltaKey, String(body ?? ''))
    }
    return jsonResponse('{}')
  }

  const delta = await getDeltaBlob(deltaKey)
  if (delta !== null) {
    return new Response(delta, { headers: { 'Content-Type': 'image/png' } })
  }
  return fetch(`${base}gfx/${rel}`)
}

/**
 * Drop-in `fetch` replacement for the data stores. In dev mode it is the
 * browser fetch; in static mode it routes /api/* onto deltas + baselines and
 * /gfx/* (sprite/tileset images) onto binary deltas + the bundled gfx tree.
 */
export async function dataFetch(url: string, init?: RequestInit): Promise<Response> {
  if (url.startsWith('/gfx/')) {
    if (await isStaticMode()) return staticGfxFetch(url, init)
    return fetch(url, init)
  }
  if (!url.startsWith('/api/')) return fetch(url, init)
  if (await isStaticMode()) return staticFetch(url, init)
  return fetch(url, init)
}
