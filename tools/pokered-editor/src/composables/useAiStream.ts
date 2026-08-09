// ───────────────────────────────────────────────────────────────────────────
// Small helpers for the browser side of the AI features: per-provider API keys
// (stored only in localStorage) and an SSE-over-fetch reader for our POST
// streaming endpoints.
// ───────────────────────────────────────────────────────────────────────────

export function getStoredKey(providerId: string): string | null {
  return localStorage.getItem('jrpg-ai-key-' + providerId)
}
export function setStoredKey(providerId: string, key: string): void {
  localStorage.setItem('jrpg-ai-key-' + providerId, key)
}

/**
 * POST `body` to `url` and parse the Server-Sent-Events response, invoking
 * `onEvent(eventName, data)` for each event block. Throws on non-OK responses.
 */
export async function streamSse(
  url: string,
  body: unknown,
  onEvent: (event: string, data: any) => void,
  signal?: AbortSignal,
): Promise<void> {
  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal,
  })
  if (!resp.ok || !resp.body) {
    throw new Error(await resp.json().then((j: any) => j.error).catch(() => `HTTP ${resp.status}`))
  }
  const reader = resp.body.getReader()
  const dec = new TextDecoder()
  let buf = ''
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    buf += dec.decode(value, { stream: true })
    const blocks = buf.split('\n\n')
    buf = blocks.pop() || ''
    for (const block of blocks) {
      const ev = block.match(/^event:\s*(.*)$/m)?.[1]?.trim()
      const dataLine = block.match(/^data:\s*(.*)$/m)?.[1]
      if (!dataLine) continue
      onEvent(ev || 'message', JSON.parse(dataLine))
    }
  }
}
