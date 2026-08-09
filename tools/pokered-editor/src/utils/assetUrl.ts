// ───────────────────────────────────────────────────────────────────────────
// Base-aware asset URL helpers.
//
// The editor is deployed on GitHub Pages under a sub-path
// (VITE_BASE_PATH=/<repo>/editor/), so absolute paths like `/gfx/...` resolve
// to the site root and 404. Every static asset reference (tileset PNGs,
// pokemon sprites, wasm, …) must go through these helpers, which prefix the
// Vite base. In dev / Electron the base is `/`, so the paths stay identical
// to what the dev-server middleware expects.
// ───────────────────────────────────────────────────────────────────────────

/** Prefix a repo-relative path (may or may not start with `/`) with the Vite base. */
export function assetUrl(path: string): string {
  const base = import.meta.env.BASE_URL
  const clean = path.replace(/^\/+/, '')
  return base.endsWith('/') ? `${base}${clean}` : `${base}/${clean}`
}

/** Base-aware URL under the shared gfx tree: `gfxUrl('tilesets/x.png')`. */
export function gfxUrl(rel: string): string {
  return assetUrl(`gfx/${rel.replace(/^\/+/, '')}`)
}

/** Repo-relative /gfx path (no base) — for `dataFetch`, which routes /gfx
 *  through the delta store in static mode. */
export function gfxRel(rel: string): string {
  return `/gfx/${rel.replace(/^\/+/, '')}`
}
