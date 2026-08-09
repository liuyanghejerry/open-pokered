// ───────────────────────────────────────────────────────────────────────────
// useProposals — a reusable review-tray store for `proposal` events. Shared by
// the chat assistant and the per-activity generate panels (e.g. data set-gen).
//
// Application is mode-aware:
//   dev/Electron — POST /api/ai/apply-change (server writes the real files).
//   static hosting — no /api backend: the proposal's target is mapped onto the
//     dataFetch route for that record (maps/trainers/pokemon/moves/items/ui-
//     layouts), whose staticFetch PUT persists to the IndexedDB delta store.
// The agent never writes directly.
// ───────────────────────────────────────────────────────────────────────────
import { ref } from 'vue'
import { dataFetch } from './dataAdapter'
import { deleteDelta } from './useDataStore'
import { staticMode } from './useStaticMode'

export type DiffOp = { type: 'ctx' | 'add' | 'del'; text: string }

export interface AssistantProposal {
  uid: string
  target: any
  title: string
  rationale?: string
  diff: DiffOp[]
  /** File content the diff was computed against (null = the proposal creates the file). */
  before?: string | null
  after: string
  status: 'pending' | 'applied' | 'reverted' | 'failed' | 'conflict'
  backup?: string | null
  error?: string
}

async function apiApply(body: unknown): Promise<{ ok: boolean; backup: string | null; path: string; conflict?: boolean }> {
  const resp = await fetch('/api/ai/apply-change', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  })
  const data = await resp.json()
  if (!resp.ok) throw new Error(data.error || 'apply failed')
  return data
}

// ── Static-mode apply: target → dataFetch route (IndexedDB delta store) ─────

/** The dataFetch PUT route for a static-mode proposal target. Returns null
 *  when the target kind has no static route (unsupported in static mode). */
function staticRouteFor(target: any): string | null {
  if (!target || typeof target !== 'object') return null
  switch (target.kind) {
    case 'pokemon': return target.id ? `/api/pokemon/${encodeURIComponent(target.id)}` : null
    case 'move': return target.id ? `/api/moves/${encodeURIComponent(target.id)}` : null
    case 'trainer': return target.id ? `/api/trainers/${encodeURIComponent(target.id)}` : null
    case 'item': return target.id ? `/api/items/${encodeURIComponent(target.id)}` : null
    case 'layout': return target.id ? `/api/ui-layouts/${encodeURIComponent(target.id)}` : null
    case 'map': {
      if (!target.map) return null
      const file = target.file === 'script.scene' ? 'script.scene' : target.file === 'script_config.json' ? 'script_config.json' : 'map.json'
      return `/api/maps/${encodeURIComponent(target.map)}/${file}`
    }
    default: return null
  }
}

/** Static-mode apply: PUT the `after` content through dataFetch (persists to
 *  the IndexedDB delta store). Content-Type selects .gui DSL vs JSON for
 *  layouts. Returns the previous content as `backup` for Revert, and flags a
 *  drift conflict when the stored content no longer matches `expect` (unless
 *  `force`). */
async function staticApply(
  target: any,
  after: string,
  opts: { force?: boolean; expect?: string | null } = {},
): Promise<{ ok: boolean; backup: string | null; path: string; conflict?: boolean }> {
  const url = staticRouteFor(target)
  if (!url) throw new Error(`Static mode cannot apply target kind "${target?.kind}"`)
  const cur = await dataFetch(url)
  const backup = cur.ok ? await cur.text() : null
  if (!opts.force && opts.expect != null && backup !== opts.expect) {
    return { ok: false, backup, path: url, conflict: true }
  }
  const headers: Record<string, string> = { 'Content-Type': target.kind === 'layout' ? 'text/plain' : 'application/json' }
  const res = await dataFetch(url, { method: 'PUT', headers, body: after })
  if (!res.ok) throw new Error(`Static apply failed: HTTP ${res.status}`)
  return { ok: true, backup, path: url }
}

/** The IndexedDB delta path for a static-mode target (used to delete a
 *  created file on revert). Mirrors staticFetch's delta keys. */
function staticDeltaPath(target: any): string | null {
  if (!target || typeof target !== 'object') return null
  switch (target.kind) {
    case 'pokemon': case 'move': case 'trainer': case 'item':
      return target.id ? `${target.kind}/${target.id}.json` : null
    case 'layout':
      return target.id ? `ui_layouts/${target.id}.gui` : null
    case 'map':
      if (!target.map) return null
      const file = target.file === 'script.scene' ? 'script.scene' : target.file === 'script_config.json' ? 'script_config.json' : 'map.json'
      return `maps/${target.map}/${file}`
    default: return null
  }
}

/**
 * A review tray. Pass `persistKey` to survive page reloads via localStorage
 * (the chat assistant does; transient per-activity generators don't).
 */
export function useProposals(persistKey?: string) {
  const proposals = ref<AssistantProposal[]>(loadTray(persistKey))
  let seq = proposals.value.length
  const save = () => saveTray(persistKey, proposals.value)

  /** Append a proposal from a `proposal` event payload ({target,title,diff,before,after,…}). */
  function add(d: any): void {
    proposals.value.push({
      uid: `p${++seq}`, target: d.target, title: d.title, rationale: d.rationale,
      diff: Array.isArray(d.diff) ? d.diff : [], before: d.before ?? null, after: d.after ?? '', status: 'pending',
    })
    save()
  }

  function clear(): void { proposals.value = []; save() }

  /** Replace the whole tray (a chat-thread switch restores its snapshot).
   *  Re-seeds the uid counter past the highest restored uid so later additions
   *  never collide with an existing proposal. */
  function replace(list: AssistantProposal[]): void {
    proposals.value = list
    seq = list.reduce((n, p) => {
      const m = /^p(\d+)$/.exec(p?.uid ?? '')
      return m ? Math.max(n, Number(m[1])) : n
    }, list.length)
    save()
  }

  /** Apply a proposal. Sends the `before` it was built on so the server can
   *  refuse a stale write; `force` overrides that guard (used by "Apply anyway").
   *  `content` overrides the written text (used by per-hunk "Apply selected").
   *  In static mode the write goes to the IndexedDB delta store instead. */
  async function applyProposal(p: AssistantProposal, opts: { force?: boolean; content?: string } = {}): Promise<void> {
    if (p.status === 'applied') return
    const after = opts.content ?? p.after
    try {
      if (staticMode.value) {
        const res = await staticApply(p.target, after, { force: opts.force, expect: p.before ?? null })
        if (res.conflict && !opts.force) { p.status = 'conflict'; p.backup = res.backup; p.error = undefined; return }
        p.backup = res.backup; p.status = 'applied'; p.error = undefined
      } else {
        const res = await apiApply({ target: p.target, after, expect: p.before ?? null, force: opts.force })
        if (res.conflict && !opts.force) { p.status = 'conflict'; p.backup = res.backup; p.error = undefined; return }
        p.backup = res.backup; p.status = 'applied'; p.error = undefined
      }
    } catch (e: any) { p.status = 'failed'; p.error = e?.message || 'apply failed' }
    finally { save() }
  }

  /** Overwrite despite a detected drift (from the conflict banner). */
  function forceApply(p: AssistantProposal): Promise<void> { return applyProposal(p, { force: true }) }

  /** Apply only the selected hunks (by hunk index), reconstructing the file. */
  function applySubset(p: AssistantProposal, accepted: Set<number>): Promise<void> {
    return applyProposal(p, { content: applyHunks(p.diff, accepted) })
  }

  /** Apply every pending proposal, in order. `filter` skips proposals (the
   *  chat assistant uses it to keep meta operations manual-only). */
  async function applyAll(filter?: (p: AssistantProposal) => boolean): Promise<void> {
    for (const p of proposals.value) if (p.status === 'pending' && (!filter || filter(p))) await applyProposal(p)
  }

  async function revertProposal(p: AssistantProposal): Promise<void> {
    try {
      if (staticMode.value) {
        if (p.backup == null) {
          // Created by this proposal: delete the delta so the baseline shows again.
          const path = staticDeltaPath(p.target)
          if (path) await deleteDelta(path)
        } else {
          await staticApply(p.target, p.backup, { force: true })
        }
        p.status = 'reverted'; p.error = undefined
      } else {
        const body = p.backup == null ? { target: p.target, op: 'delete' } : { target: p.target, after: p.backup }
        await apiApply(body)
        p.status = 'reverted'; p.error = undefined
      }
    } catch (e: any) { p.error = e?.message || 'revert failed' }
    finally { save() }
  }

  function discard(p: AssistantProposal): void {
    proposals.value = proposals.value.filter(x => x !== p)
    save()
  }

  return { proposals, add, clear, replace, applyProposal, forceApply, applySubset, applyAll, revertProposal, discard }
}

// ── per-hunk diff selection (pure, testable) ──────────────────────────────────

/** Group a line-diff into hunks: maximal runs of consecutive add/del ops,
 *  bounded by unchanged context. Returns each hunk as the list of its op indices. */
export function diffHunks(diff: DiffOp[]): number[][] {
  const hunks: number[][] = []
  let cur: number[] = []
  diff.forEach((op, i) => {
    if (op.type === 'ctx') { if (cur.length) { hunks.push(cur); cur = [] } }
    else cur.push(i)
  })
  if (cur.length) hunks.push(cur)
  return hunks
}

/**
 * Reconstruct the file content applying ONLY the accepted hunks. Context lines
 * are always kept; an accepted hunk takes its `after` side (adds in, dels out),
 * a rejected hunk keeps its `before` side (dels stay, adds dropped). Accepting
 * every hunk reproduces the full `after`; accepting none reproduces `before`.
 */
export function applyHunks(diff: DiffOp[], accepted: Set<number>): string {
  const opHunk = new Map<number, number>()
  diffHunks(diff).forEach((ops, h) => ops.forEach(i => opHunk.set(i, h)))
  const out: string[] = []
  diff.forEach((op, i) => {
    if (op.type === 'ctx') { out.push(op.text); return }
    const isAccepted = accepted.has(opHunk.get(i)!)
    if (op.type === 'add') { if (isAccepted) out.push(op.text) }
    else { if (!isAccepted) out.push(op.text) } // rejected del → keep the original line
  })
  return out.join('\n')
}

// ── localStorage persistence (browser only; no-op under node/test) ────────────
function loadTray(key?: string): AssistantProposal[] {
  if (!key || typeof localStorage === 'undefined') return []
  try { const s = localStorage.getItem(key); const v = s ? JSON.parse(s) : []; return Array.isArray(v) ? v : [] }
  catch { return [] }
}
function saveTray(key: string | undefined, list: AssistantProposal[]): void {
  if (!key || typeof localStorage === 'undefined') return
  try { localStorage.setItem(key, JSON.stringify(list)) } catch { /* quota / disabled — best effort */ }
}
