// ───────────────────────────────────────────────────────────────────────────
// Session artifacts — pure derivations backing the assistant panel's "produced
// this session" section: which proposals are currently applied, their diff
// stats, which activity a row jumps to, and the aggregate summary line.
// Store-free so it is unit-testable; the panel resolves activity ids itself.
// ───────────────────────────────────────────────────────────────────────────
import type { AssistantProposal } from '../../composables/useProposals'

/** One applied proposal rendered as a row in the artifacts list. */
export interface Artifact {
  uid: string
  /** Proposal target kind (story/data/scene/gui/map/map-create/project-*). */
  kind: string
  /** Display label: the target path (or the proposal title as fallback). */
  path: string
  /** Icon glyph for the kind. */
  icon: string
  /** Editor activity TYPE this artifact belongs to; null = not navigable. */
  activityType: string | null
  add: number
  del: number
}

// Kind glyphs, aligned with the activity icons in App.vue.
const KIND_ICON: Record<string, string> = {
  story: '📖', data: '📊', scene: '📝', gui: '🎨', map: '🗺',
  'map-create': '🗺', 'project-config': '⚙', 'project-scaffold': '🏗',
}

// Kind → activity TYPE (App.vue's loadActivity switches on the type; the panel
// resolves the concrete activity id from the project config). project-scaffold
// is deliberately absent — there is nothing meaningful to jump to after a
// scaffold applies, so its row renders non-clickable.
const KIND_ACTIVITY: Record<string, string> = {
  story: 'story', data: 'data', scene: 'script', gui: 'ui',
  map: 'map', 'map-create': 'map', 'project-config': 'settings',
}

/**
 * The artifacts of this session: proposals currently APPLIED. A reverted
 * proposal flips status to 'reverted' and drops out on its own; clearing the
 * chat empties the tray and therefore this list. One row per proposal.
 */
export function buildArtifacts(proposals: AssistantProposal[]): Artifact[] {
  return proposals
    .filter(p => p.status === 'applied')
    .map(p => {
      const kind = String(p.target?.kind ?? '')
      return {
        uid: p.uid,
        kind,
        path: String(p.target?.path ?? '') || p.title,
        icon: KIND_ICON[kind] ?? '📄',
        activityType: KIND_ACTIVITY[kind] ?? null,
        add: p.diff.filter(o => o.type === 'add').length,
        del: p.diff.filter(o => o.type === 'del').length,
      }
    })
}

export interface ArtifactSummary { files: number; add: number; del: number }

/** Aggregate header stats: files = applied proposals, lines = diff ops summed. */
export function summarize(artifacts: Artifact[]): ArtifactSummary {
  return {
    files: artifacts.length,
    add: artifacts.reduce((n, a) => n + a.add, 0),
    del: artifacts.reduce((n, a) => n + a.del, 0),
  }
}
