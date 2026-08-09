// ───────────────────────────────────────────────────────────────────────────
// Browser-side AI surface for static hosting (GitHub Pages): no /api backend
// exists, so the AI Assistant talks to the provider directly from the browser
// and reads/writes project data through `dataFetch` (IndexedDB deltas layered
// over the bundled baselines).
//
// Model: OpenAI-compatible endpoints only (DeepSeek, OpenAI, OpenRouter, …) —
// `buildBrowserModel` creates the provider with the transient key from
// localStorage, same shape as the dev-server `buildModel`.
//
// Tools: a pokered-shaped subset of the server surface (the server tools
// speak jrpg-editor's ProjectContext model — characters/quests/.gui — which
// doesn't exist in pokered). READ tools return project data; PROPOSE tools
// stage a proposal into the review tray via `emit` and never write directly.
//
// The chat state machine lives in useAssistantChat (useStaticAssistantChat);
// this module only holds pure, testable pieces.
// ───────────────────────────────────────────────────────────────────────────
import { tool } from 'ai'
import { z } from 'zod'
import { dataFetch } from './dataAdapter'
import type { DiffOp } from './useProposals'
import type { ProviderProfile } from '../types/ai'

/** What a PROPOSE tool emits; the review tray consumes this shape. */
export interface StaticProposalPayload {
  target: any
  title: string
  rationale?: string
  before: string | null
  after: string
  diff: DiffOp[]
}

/** emit('proposal' | 'plan', payload) — wired by the chat state machine. */
export interface StaticAiEmit {
  (type: 'proposal', payload: StaticProposalPayload): void
  (type: 'plan', payload: { steps: unknown }): void
}

// ── Model ──────────────────────────────────────────────────────────────────

/**
 * Build a browser LanguageModel for an OpenAI-compatible profile. Anthropic
 * profiles are rejected (the browser build doesn't bundle the Anthropic SDK).
 */
export async function buildBrowserModel(profile: ProviderProfile, apiKey: string): Promise<any> {
  if (profile.kind !== 'openai') {
    throw new Error(
      `Provider "${profile.id}" (kind "${profile.kind}") is not supported in static mode — ` +
        `use an OpenAI-compatible provider (DeepSeek, OpenAI, OpenRouter, …).`,
    )
  }
  const { createOpenAICompatible } = await import('@ai-sdk/openai-compatible')
  const provider = createOpenAICompatible({
    name: profile.id || 'openai',
    apiKey,
    baseURL: profile.baseURL,
  })
  return provider(profile.model)
}

// ── Line diff (review tray) ────────────────────────────────────────────────

/** Longest-common-subsequence line diff → DiffOp[] for the review card. */
export function lineDiff(before: string, after: string): DiffOp[] {
  const a = before.split('\n')
  const b = after.split('\n')
  // Standard LCS DP; rows = before, cols = after.
  const n = a.length
  const m = b.length
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0))
  for (let i = n - 1; i >= 0; i--) {
    for (let k = m - 1; k >= 0; k--) {
      dp[i][k] = a[i] === b[k] ? dp[i + 1][k + 1] + 1 : Math.max(dp[i + 1][k], dp[i][k + 1])
    }
  }
  const ops: DiffOp[] = []
  let i = 0
  let k = 0
  // Collapse adjacent same-kind ops into single runs (matches the server diff).
  const push = (type: DiffOp['type'], text: string) => {
    const last = ops[ops.length - 1]
    if (last && last.type === type) last.text += '\n' + text
    else ops.push({ type, text })
  }
  while (i < n && k < m) {
    if (a[i] === b[k]) { push('ctx', a[i]); i++; k++ }
    else if (dp[i + 1][k] >= dp[i][k + 1]) { push('del', a[i]); i++ }
    else { push('add', b[k]); k++ }
  }
  while (i < n) { push('del', a[i]); i++ }
  while (k < m) { push('add', b[k]); k++ }
  return ops
}

// ── Tool steps → UIMessage parts (chat round-trip) ─────────────────────────

/**
 * Turn streamText's `steps` into `tool-<name>` UIMessage parts, pairing each
 * tool call with its result. AI SDK v6 ToolUIPart contract: `input` (not
 * `args`), a non-undefined `output`, and `providerExecuted` — otherwise the
 * next turn's convertToModelMessages rebuilds tool calls with no arguments
 * and null results, and a multi-turn conversation breaks after the first
 * tool-using turn.
 */
export function toolPartsFromSteps(steps: Array<{ toolCalls?: unknown[]; toolResults?: unknown[] }>): any[] {
  const parts: any[] = []
  for (const step of steps) {
    const results = new Map<string, any>()
    for (const r of step.toolResults ?? []) {
      const tr = r as any
      results.set(tr.toolCallId, tr)
    }
    for (const raw of step.toolCalls ?? []) {
      const tc = raw as any
      const tr = results.get(tc.toolCallId)
      parts.push({
        type: `tool-${tc.toolName}`,
        toolCallId: tc.toolCallId,
        toolName: tc.toolName,
        input: tc.input ?? tc.args,
        state: 'output-available',
        output: tr?.output ?? null,
        providerExecuted: true,
      })
    }
  }
  return parts
}

/**
 * Map a static-mode proposal target onto the shared auto-apply switch
 * vocabulary (story/data/scene/gui/map). Static tools emit pokered-shaped
 * kinds (pokemon/move/trainer/item/layout/map) that have no matching switch,
 * so without this the per-kind auto-apply toggles would be dead in static
 * mode. Meta kinds are never produced here — shouldAutoApply hard-blocks them.
 */
export function autoApplyKindFor(target: { kind?: string } | null | undefined): string {
  switch (target?.kind) {
    case 'layout': return 'gui'
    case 'map': return 'map'
    default: return 'data' // pokemon / move / trainer / item
  }
}

// ── System prompt ──────────────────────────────────────────────────────────

/** pokered-shaped assistant system prompt (mirrors the dev server's intent). */
export function buildStaticSystem(uiContext?: { activity?: string; route?: string }, memories: string[] = []): string {
  const where = uiContext?.activity ? `\nThe user is currently in the "${uiContext.activity}" editor activity.` : ''
  const mem = memories.length ? `\n\nAssistant memory (from earlier turns):\n${memories.join('\n')}` : ''
  return [
    'You are the editing assistant for PokéRed Editor, a browser tool that edits Pokémon Red/Blue game data',
    '(maps, wild encounters, trainers, Pokémon base stats, moves, items, UI layouts, per-map script.scene DSL).',
    'The project is running in static mode: your reads and proposals go through the editor\'s IndexedDB delta',
    'store layered over the bundled baseline data.',
    'You can inspect game data with the read_* tools and suggest edits with the propose_* tools.',
    'A propose_* tool NEVER applies the edit — it stages it for the user to review and apply manually.',
    'Never claim you applied or saved an edit; only that you proposed it.',
    'For .scene scripts: read the map\'s script_config.json and script.scene first, keep existing functions,',
    'and match the existing DSL style (@trigger / @say / @t bilingual lines).',
    'Answer in the user\'s language.',
    where,
    mem,
  ]
    .filter(Boolean)
    .join('\n')
}

// ── Read / propose tool impls (dataFetch-backed) ────────────────────────────

/** Read a dataFetch URL, returning the text or null on 404/no baseline. */
async function fetchText(url: string): Promise<string | null> {
  try {
    const res = await dataFetch(url)
    return res.ok ? await res.text() : null
  } catch {
    return null
  }
}

const NOTE = ' Does NOT apply — it stages the edit for human review. Never claim you applied it.'

/** Normalize a tool `content` arg to a pretty JSON string, validating it parses. */
function normalizeJson(content: unknown): { text: string; error?: string } {
  if (typeof content === 'string') {
    try { JSON.parse(content); return { text: content } } catch (e) {
      return { text: '', error: 'content is not valid JSON: ' + (e as Error).message }
    }
  }
  if (content && typeof content === 'object') return { text: JSON.stringify(content, null, 2) }
  return { text: '', error: 'content must be a JSON object or a JSON string' }
}

async function listJson(url: string, fallback: string[] = []): Promise<string> {
  const res = await dataFetch(url)
  if (!res.ok) return fallback.join('\n')
  const list = await res.json().catch(() => [])
  return Array.isArray(list) ? list.join('\n') : fallback.join('\n')
}

function propose(
  emit: StaticAiEmit,
  target: any,
  title: string,
  before: string | null,
  after: string,
  rationale?: string,
): { ok: boolean; proposalId?: string; error?: string } {
  if (after == null || after === '') return { ok: false, error: 'nothing to write' }
  const payload: StaticProposalPayload = {
    target,
    title,
    rationale,
    before,
    after,
    diff: before != null ? lineDiff(before, after) : [],
  }
  emit('proposal', payload)
  return { ok: true, proposalId: String(payload.target?.id ?? '') }
}

/**
 * Build the browser tool surface. `emit` is wired by the chat state machine:
 * 'proposal' → review tray, 'plan' → working checklist.
 */
export async function buildStaticTools(emit: StaticAiEmit): Promise<Record<string, unknown>> {
  const read = {
    list_maps: async () => listJson('/api/maps'),
    read_map: async ({ map }: { map: string }) => (await fetchText(`/api/maps/${encodeURIComponent(map)}/map.json`)) ?? 'ERROR: map not found',
    read_scene: async ({ map }: { map: string }) =>
      (await fetchText(`/api/maps/${encodeURIComponent(map)}/script.scene`)) ?? 'ERROR: no script.scene for this map',
    read_script_config: async ({ map }: { map: string }) =>
      (await fetchText(`/api/maps/${encodeURIComponent(map)}/script_config.json`)) ?? 'ERROR: no script_config.json for this map',
    list_pokemon: async () => listJson('/api/pokemon'),
    read_pokemon: async ({ id }: { id: string }) => (await fetchText(`/api/pokemon/${encodeURIComponent(id)}`)) ?? 'ERROR: not found',
    list_moves: async () => listJson('/api/moves'),
    read_move: async ({ id }: { id: string }) => (await fetchText(`/api/moves/${encodeURIComponent(id)}`)) ?? 'ERROR: not found',
    list_trainers: async () => listJson('/api/trainers'),
    read_trainer: async ({ id }: { id: string }) => (await fetchText(`/api/trainers/${encodeURIComponent(id)}`)) ?? 'ERROR: not found',
    list_items: async () => listJson('/api/items'),
    read_item: async ({ id }: { id: string }) => (await fetchText(`/api/items/${encodeURIComponent(id)}`)) ?? 'ERROR: not found',
    list_layouts: async () => listJson('/api/ui-layouts'),
    read_layout: async ({ name }: { name: string }) => (await fetchText(`/api/ui-layouts/${encodeURIComponent(name)}`)) ?? 'ERROR: not found',
  }

  const proposeFns = {
    propose_pokemon_edit: async ({ id, content, rationale }: { id: string; content: unknown; rationale?: string }) => {
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const before = await fetchText(`/api/pokemon/${encodeURIComponent(id)}`)
      const r = propose(emit, { kind: 'pokemon', id, path: `pokemon/${id}` }, `Edit pokemon "${id}"`, before, norm.text, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
    propose_move_edit: async ({ id, content, rationale }: { id: string; content: unknown; rationale?: string }) => {
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const before = await fetchText(`/api/moves/${encodeURIComponent(id)}`)
      const r = propose(emit, { kind: 'move', id, path: `moves/${id}` }, `Edit move "${id}"`, before, norm.text, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
    propose_trainer_edit: async ({ id, content, rationale }: { id: string; content: unknown; rationale?: string }) => {
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const before = await fetchText(`/api/trainers/${encodeURIComponent(id)}`)
      const r = propose(emit, { kind: 'trainer', id, path: `trainers/${id}` }, `Edit trainer "${id}"`, before, norm.text, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
    propose_item_edit: async ({ id, content, rationale }: { id: string; content: unknown; rationale?: string }) => {
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const before = await fetchText(`/api/items/${encodeURIComponent(id)}`)
      const r = propose(emit, { kind: 'item', id, path: `items/${id}` }, `Edit item "${id}"`, before, norm.text, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
    propose_layout_edit: async ({ name, content, rationale }: { name: string; content: string; rationale?: string }) => {
      if (typeof content !== 'string' || !content.trim()) return 'ERROR: content must be a non-empty string'
      const before = await fetchText(`/api/ui-layouts/${encodeURIComponent(name)}`)
      const r = propose(emit, { kind: 'layout', id: name, path: `ui_layouts/${name}` }, `Edit layout "${name}"`, before, content, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
    propose_map_edit: async ({ map, content, rationale }: { map: string; content: unknown; rationale?: string }) => {
      if (!map || typeof map !== 'string') return 'ERROR: map (the map directory name) is required'
      const norm = normalizeJson(content)
      if (norm.error) return 'ERROR: ' + norm.error
      const before = await fetchText(`/api/maps/${encodeURIComponent(map)}/map.json`)
      const r = propose(emit, { kind: 'map', map, file: 'map.json', id: map, path: `maps/${map}/map.json` }, `Edit map "${map}"`, before, norm.text, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
    propose_scene_edit: async ({ map, content, rationale }: { map: string; content: string; rationale?: string }) => {
      if (!map || typeof map !== 'string') return 'ERROR: map (the map directory name) is required'
      if (typeof content !== 'string' || !content.trim()) return 'ERROR: content must be a non-empty string'
      const before = await fetchText(`/api/maps/${encodeURIComponent(map)}/script.scene`)
      const r = propose(emit, { kind: 'map', map, file: 'script.scene', id: map, path: `maps/${map}/script.scene` }, `Edit scene for map "${map}"`, before, content, rationale)
      return r.error ? 'ERROR: ' + r.error : JSON.stringify(r)
    },
  }

  return {
    // READ
    list_maps: tool({ description: 'List all map directory names.', inputSchema: z.object({}), execute: read.list_maps }),
    read_map: tool({ description: 'Read a map\'s `map.json` (header, warps, NPCs, signs, wild encounters). `map` = the map directory name (see list_maps).', inputSchema: z.object({ map: z.string() }), execute: read.read_map }),
    read_scene: tool({ description: 'Read a map\'s `script.scene` DSL file. `map` = the map directory name.', inputSchema: z.object({ map: z.string() }), execute: read.read_scene }),
    read_script_config: tool({ description: 'Read a map\'s `script_config.json` (script bindings, triggers).', inputSchema: z.object({ map: z.string() }), execute: read.read_script_config }),
    list_pokemon: tool({ description: 'List all Pokémon species names.', inputSchema: z.object({}), execute: read.list_pokemon }),
    read_pokemon: tool({ description: 'Read a Pokémon\'s full data (baseStats, types, growth, moves, evolutions, pokedex).', inputSchema: z.object({ id: z.string() }), execute: read.read_pokemon }),
    list_moves: tool({ description: 'List all move names.', inputSchema: z.object({}), execute: read.list_moves }),
    read_move: tool({ description: 'Read a move\'s data (id, effect, power, type, accuracy, pp).', inputSchema: z.object({ id: z.string() }), execute: read.read_move }),
    list_trainers: tool({ description: 'List all trainer class names.', inputSchema: z.object({}), execute: read.list_trainers }),
    read_trainer: tool({ description: 'Read a trainer class\'s parties.', inputSchema: z.object({ id: z.string() }), execute: read.read_trainer }),
    list_items: tool({ description: 'List all item names.', inputSchema: z.object({}), execute: read.list_items }),
    read_item: tool({ description: 'Read an item\'s data (price, category, effect).', inputSchema: z.object({ id: z.string() }), execute: read.read_item }),
    list_layouts: tool({ description: 'List UI layout names (bag, battle_main, dialog, start, …).', inputSchema: z.object({}), execute: read.list_layouts }),
    read_layout: tool({ description: 'Read a `.gui` layout source by name.', inputSchema: z.object({ name: z.string() }), execute: read.read_layout }),
    // PROPOSE
    propose_pokemon_edit: tool({
      description: 'Propose replacing a Pokémon\'s full data record. `content` = the COMPLETE new record as a JSON string (read it first with read_pokemon).' + NOTE,
      inputSchema: z.object({ id: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_pokemon_edit,
    }),
    propose_move_edit: tool({
      description: 'Propose replacing a move\'s data record. `content` = the COMPLETE new record as a JSON string.' + NOTE,
      inputSchema: z.object({ id: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_move_edit,
    }),
    propose_trainer_edit: tool({
      description: 'Propose replacing a trainer class\'s parties. `content` = the COMPLETE new record as a JSON string.' + NOTE,
      inputSchema: z.object({ id: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_trainer_edit,
    }),
    propose_item_edit: tool({
      description: 'Propose replacing an item\'s data record. `content` = the COMPLETE new record as a JSON string.' + NOTE,
      inputSchema: z.object({ id: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_item_edit,
    }),
    propose_layout_edit: tool({
      description: 'Propose writing a `.gui` layout file. `content` = the COMPLETE file text (read it first with read_layout).' + NOTE,
      inputSchema: z.object({ name: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_layout_edit,
    }),
    propose_map_edit: tool({
      description: 'Propose replacing a map\'s `map.json` (header, warps, NPCs, wild encounters). `map` = the map directory name; `content` = the COMPLETE new map.json as a JSON string.' + NOTE,
      inputSchema: z.object({ map: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_map_edit,
    }),
    propose_scene_edit: tool({
      description: 'Propose writing a map\'s `script.scene` DSL file. `map` = the map directory name; `content` = the COMPLETE file text (read the existing scene first and preserve its functions).' + NOTE,
      inputSchema: z.object({ map: z.string(), content: z.string(), rationale: z.string().optional() }),
      execute: proposeFns.propose_scene_edit,
    }),
    // PLAN (UI-only)
    update_plan: tool({
      description: 'Publish or update your working plan for a MULTI-STEP task so the user can watch progress. Call it once when you begin a task with several steps, and AGAIN each time a step starts or finishes. Keep 2–6 concise steps. Mutates nothing.',
      inputSchema: z.object({
        steps: z.array(z.object({ title: z.string(), status: z.enum(['pending', 'active', 'done']).optional() })).min(1).max(12),
      }),
      execute: async ({ steps }: { steps: Array<{ title: string; status?: string }> }) => {
        const norm = steps.map((s) => ({ title: String(s.title), status: s.status ?? 'pending' }))
        emit('plan', { steps: norm })
        return { ok: true, steps: norm.length }
      },
    }),
  }
}

