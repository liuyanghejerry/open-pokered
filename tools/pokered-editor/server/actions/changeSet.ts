// ───────────────────────────────────────────────────────────────────────────
// ChangeSet — the propose→review→apply unit.
//
// Agents NEVER write to the project directly. Their "propose_*" tools append a
// Proposal (target + before/after + computed diff) to a ChangeSet and emit a
// `proposal` event. The client renders the diffs in a review tray and applies
// the accepted ones via the existing mutation endpoints; reverting re-applies
// `before`. This module is pure (no fs/AI) and fully unit-testable.
// ───────────────────────────────────────────────────────────────────────────

export type DiffOpType = 'ctx' | 'add' | 'del'
export interface DiffOp { type: DiffOpType; text: string }

/** What a proposal targets — enough for the client to apply it via the matching
 *  existing mutation endpoint, and to label/group it in the review tray. */
export interface ChangeTarget {
  kind: 'story' | 'data' | 'scene' | 'gui' | 'map' | 'project-config' | 'project-scaffold' | 'map-create'
  /** story: the kind (characters/quests/arcs). */
  storyKind?: string
  /** data: the table id. */
  table?: string
  /** story/data: the record id. */
  id?: string
  /** scene: the scene name/stem. */
  scene?: string
  /** gui: the layout name. */
  name?: string
  /** map / map-create: the map directory name. */
  map?: string
  /** project-scaffold: target directory (slug under the editor root, or absolute). */
  dir?: string
  /** Human-readable path/label for the review tray. */
  path: string
}

export interface Proposal {
  id: string
  target: ChangeTarget
  title: string
  rationale?: string
  /** Current content (null when creating something new). */
  before: string | null
  /** Proposed content. */
  after: string
  diff: DiffOp[]
}

/** A minimal LCS-based line diff (Myers is overkill for review rendering). */
export function lineDiff(before: string, after: string): DiffOp[] {
  const a = before === '' ? [] : before.split('\n')
  const b = after === '' ? [] : after.split('\n')
  const m = a.length, n = b.length
  // lcs[i][j] = length of the longest common subsequence of a[i:] and b[j:]
  const lcs: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0))
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1])
    }
  }
  const ops: DiffOp[] = []
  let i = 0, j = 0
  while (i < m && j < n) {
    if (a[i] === b[j]) { ops.push({ type: 'ctx', text: a[i] }); i++; j++ }
    else if (lcs[i + 1][j] >= lcs[i][j + 1]) { ops.push({ type: 'del', text: a[i] }); i++ }
    else { ops.push({ type: 'add', text: b[j] }); j++ }
  }
  while (i < m) ops.push({ type: 'del', text: a[i++] })
  while (j < n) ops.push({ type: 'add', text: b[j++] })
  return ops
}

export interface ProposalInput {
  target: ChangeTarget
  title: string
  rationale?: string
  before: string | null
  after: string
}

/** Accumulates the proposals produced during one agent run. */
export class ChangeSet {
  readonly proposals: Proposal[] = []
  private seq = 0

  add(input: ProposalInput): Proposal {
    const proposal: Proposal = {
      id: `p${++this.seq}`,
      ...input,
      diff: lineDiff(input.before ?? '', input.after),
    }
    this.proposals.push(proposal)
    return proposal
  }

  /** Lightweight summaries (no diffs) for an action's final `done` result. */
  summaries(): Array<{ id: string; target: ChangeTarget; title: string }> {
    return this.proposals.map(p => ({ id: p.id, target: p.target, title: p.title }))
  }
}
