// ───────────────────────────────────────────────────────────────────────────
// Byte-level progress fetch, shared by the wasm loaders (pokered-runner-web
// game engine + dotzuki-web layout preview) so the boot loading screen can show
// download progress for the .wasm binaries on static hosting.
// ───────────────────────────────────────────────────────────────────────────

/** Fetch a URL with byte-level progress. Falls back to a plain fetch (no
 *  total) when Content-Length is absent or the body isn't streamable. */
export async function fetchWithProgress(
  url: string,
  onProgress?: (loaded: number, total: number) => void,
): Promise<ArrayBuffer> {
  const resp = await fetch(url)
  if (!resp.ok) throw new Error(`HTTP ${resp.status} for ${url}`)
  const total = Number(resp.headers.get('Content-Length') ?? 0)
  if (!resp.body || !onProgress || total === 0) return resp.arrayBuffer()
  const reader = resp.body.getReader()
  const chunks: Uint8Array[] = []
  let loaded = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    if (value) {
      chunks.push(value)
      loaded += value.length
      onProgress(loaded, total)
    }
  }
  const out = new Uint8Array(loaded)
  let offset = 0
  for (const chunk of chunks) {
    out.set(chunk, offset)
    offset += chunk.length
  }
  return out.buffer
}
