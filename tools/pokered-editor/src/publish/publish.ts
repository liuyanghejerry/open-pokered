// ───────────────────────────────────────────────────────────────────────────
// Static-hosting game publishing (the editor's migration of dotzuki's
// `dotzuki export --web`, re-anchored for a backend-less deployment).
//
// dotzuki's editor shells out to the `dotzuki` CLI via POST /api/export —
// impossible on GitHub Pages. pokered's runner (pokered-runner-web) embeds
// the whole game (gfx + compiled data), and the editor's IndexedDB delta
// store holds every runtime-replayable edit, so publishing needs no backend:
//   runner wasm (bundled with the editor deploy) + delta set + player page
// → one self-contained HTML file, downloaded locally.
//
// Limitation (same as the in-editor playtest): binary deltas (edited sprites
// / tilesets under gfx/) are skipped — gfx is compiled into the wasm binary.
// ───────────────────────────────────────────────────────────────────────────

import { exportDeltasJson } from '../composables/useDataStore'
import { renderPlayerHtml } from './playerTemplate'

export interface PublishResult {
  fileName: string
  sizeBytes: number
  /** Deltas embedded in the artifact (text/data edits; binary ones skipped). */
  deltaCount: number
}

/** Title → file name slug ("Pokémon Red" → "pokemon-red.html"). */
export function slugifyTitle(title: string): string {
  const ascii = title.normalize('NFD').replace(/[\u0300-\u036f]/g, '')
  const slug = ascii
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return slug || 'pokemon-red'
}

async function blobToBase64(blob: Blob): Promise<string> {
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
  const idx = dataUrl.indexOf(',')
  if (idx < 0) throw new Error('unexpected data: URL shape')
  return dataUrl.slice(idx + 1)
}

/**
 * Assemble the published game HTML: the deploy-bundled runner wasm + glue and
 * the current delta set, wrapped in the single-file player template. Works in
 * every editor mode — dev, Electron and static hosting — because it reuses
 * the exact same `${base}wasm/…` URLs as the playtest bridge.
 */
export async function buildPublishedGame(
  title: string,
): Promise<{ html: string; deltaCount: number }> {
  const base = import.meta.env.BASE_URL
  const [glueRes, wasmRes, editsJson] = await Promise.all([
    fetch(new URL(`${base}wasm/pokered_runner_web.js`, window.location.origin).href),
    fetch(new URL(`${base}wasm/pokered_runner_web_bg.wasm`, window.location.origin).href),
    exportDeltasJson(),
  ])
  if (!glueRes.ok) {
    throw new Error(
      `runner glue not found (HTTP ${glueRes.status}) — build it with \`pnpm build:wasm-pokered\``,
    )
  }
  if (!wasmRes.ok) {
    throw new Error(
      `runner wasm not found (HTTP ${wasmRes.status}) — build it with \`pnpm build:wasm-pokered\``,
    )
  }
  const html = renderPlayerHtml({
    title,
    runnerGlueJs: await glueRes.text(),
    wasmBase64: await blobToBase64(await wasmRes.blob()),
    editsJson,
  })
  const deltaCount = (JSON.parse(editsJson) as unknown[]).length
  return { html, deltaCount }
}

/** Build + download the published game; resolves with the artifact summary. */
export async function publishGame(title: string): Promise<PublishResult> {
  const { html, deltaCount } = await buildPublishedGame(title)
  const blob = new Blob([html], { type: 'text/html' })
  const fileName = `${slugifyTitle(title)}.html`
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = fileName
  document.body.appendChild(a)
  a.click()
  a.remove()
  // Keep the URL alive past the click handler; revoke on a later tick.
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
  return { fileName, sizeBytes: blob.size, deltaCount }
}
