// ───────────────────────────────────────────────────────────────────────────
// proxy.ts — outbound proxy support for AI providers (text + image). Many
// networks can't reach api.anthropic.com / api.openai.com /
// generativelanguage.googleapis.com directly.
//
// IMPORTANT: a `ProxyAgent` from the userland `undici` package must be used with
// undici's OWN `fetch` — passing it as `dispatcher` to Node's built-in global
// `fetch` throws UND_ERR_INVALID_ARG. So when a proxy is in play we route through
// undici.fetch.
// ───────────────────────────────────────────────────────────────────────────

/** Resolve a proxy URL from an explicit value, else common env vars. */
export function resolveProxy(explicit?: string): string | undefined {
  const v = explicit?.trim()
  if (v) return v
  return (
    process.env.GEMINI_PROXY ||
    process.env.HTTPS_PROXY || process.env.https_proxy ||
    process.env.HTTP_PROXY || process.env.http_proxy ||
    process.env.ALL_PROXY || process.env.all_proxy ||
    undefined
  )
}

/** Fetch `input` optionally through `proxyUrl` (or a proxy env var). */
export async function proxiedFetch(proxyUrl: string | undefined, input: string, init: any): Promise<Response> {
  const proxy = resolveProxy(proxyUrl)
  if (!proxy) return fetch(input, init)
  const { fetch: uFetch, ProxyAgent } = await import('undici')
  return uFetch(input, { ...init, dispatcher: new ProxyAgent(proxy) }) as unknown as Response
}

/**
 * A custom fetch function bound to a proxy, for libraries that accept one (e.g.
 * the AI SDK's `createAnthropic({ fetch })` / `createOpenAICompatible({ fetch })`).
 * Returns undefined when no proxy applies, so the caller can omit the option.
 */
export async function proxyFetchFn(proxyUrl: string | undefined): Promise<typeof fetch | undefined> {
  const proxy = resolveProxy(proxyUrl)
  if (!proxy) return undefined
  try {
    const { fetch: uFetch, ProxyAgent } = await import('undici')
    const agent = new ProxyAgent(proxy)
    return ((input: any, init?: any) => uFetch(input, { ...init, dispatcher: agent })) as unknown as typeof fetch
  } catch {
    return undefined
  }
}
