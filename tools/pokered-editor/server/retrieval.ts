// ───────────────────────────────────────────────────────────────────────────
// retrieval — optional embeddings RAG over the project corpus (story records,
// scenes, data records). Gated behind a provider's `embeddingModel`; when set,
// the chat augments its context with the top-K most relevant chunks for the
// user's message. The corpus is embedded once and cached in memory (keyed by a
// cheap content signature) so only the query is embedded per turn.
//
// Deterministic parts (buildCorpus / cosineSim) are exported for unit testing;
// the embedding call goes through the configured openai-compatible provider.
// ───────────────────────────────────────────────────────────────────────────
import type { ProjectContext } from './context/projectContext'
import type { ProviderProfile } from './ai'
import { proxyFetchFn } from './proxy'

export interface Chunk { id: string; kind: string; text: string }

/** Flatten the project into retrievable chunks (one per record / scene). */
export function buildCorpus(project: ProjectContext): Chunk[] {
  const chunks: Chunk[] = []
  const push = (id: string, kind: string, text: string) => { if (text && text.trim()) chunks.push({ id, kind, text: text.slice(0, 2500) }) }

  for (const c of project.listCharacters()) push(`character:${c.id ?? ''}`, 'character', JSON.stringify(c))
  for (const q of project.listQuests()) push(`quest:${q.id ?? ''}`, 'quest', JSON.stringify(q))
  for (const a of project.listArcs()) push(`arc:${a.id ?? ''}`, 'arc', JSON.stringify(a))
  for (const s of project.listScenes()) { try { push(`scene:${s.stem}`, 'scene', project.readScene(s.path)) } catch { /* skip */ } }
  for (const t of project.listTables()) {
    const idField = t.idField ?? 'id'
    for (const r of project.listRecords(t.id)) push(`data:${t.id}:${r[idField] ?? ''}`, 'data', JSON.stringify(r))
  }
  return chunks
}

export function cosineSim(a: number[], b: number[]): number {
  let dot = 0, na = 0, nb = 0
  const n = Math.min(a.length, b.length)
  for (let i = 0; i < n; i++) { dot += a[i] * b[i]; na += a[i] * a[i]; nb += b[i] * b[i] }
  const d = Math.sqrt(na) * Math.sqrt(nb)
  return d === 0 ? 0 : dot / d
}

/** Top-K chunks by embedding cosine similarity, scored against precomputed vectors. */
export function topK(queryVec: number[], chunks: Chunk[], vectors: number[][], k: number): Chunk[] {
  return chunks
    .map((c, i) => ({ c, s: cosineSim(queryVec, vectors[i] ?? []) }))
    .sort((a, b) => b.s - a.s)
    .slice(0, k)
    .map(x => x.c)
}

async function embeddingModelFor(profile: ProviderProfile, apiKey: string): Promise<any> {
  if (profile.kind === 'anthropic') throw new Error('Anthropic has no embedding model; use an openai-compatible embedding provider.')
  const fetchFn = await proxyFetchFn(profile.proxyUrl)
  const { createOpenAICompatible } = await import('@ai-sdk/openai-compatible')
  const provider = createOpenAICompatible({ name: profile.id || 'openai', apiKey, baseURL: profile.baseURL, ...(fetchFn ? { fetch: fetchFn } : {}) })
  return provider.textEmbeddingModel(profile.embeddingModel!)
}

let cache: { sig: string; chunks: Chunk[]; vectors: number[][] } | null = null

/**
 * Retrieve the top-K project chunks most relevant to `query`. Returns [] when no
 * embeddingModel is configured (the caller falls back to structured context).
 */
export async function retrieve(
  project: ProjectContext, profile: ProviderProfile, apiKey: string, query: string, k = 6,
): Promise<Chunk[]> {
  if (!profile.embeddingModel || !query.trim()) return []
  const corpus = buildCorpus(project)
  if (!corpus.length) return []
  const sig = corpus.map(c => `${c.id}:${c.text.length}`).join('|')

  const { embed, embedMany } = await import('ai')
  const model = await embeddingModelFor(profile, apiKey)

  if (!cache || cache.sig !== sig) {
    const { embeddings } = await embedMany({ model, values: corpus.map(c => c.text) })
    cache = { sig, chunks: corpus, vectors: embeddings as number[][] }
  }
  const { embedding } = await embed({ model, value: query })
  return topK(embedding as number[], cache.chunks, cache.vectors, k)
}
