// ───────────────────────────────────────────────────────────────────────────
// Per-kind auto-apply settings for the assistant review tray.
//
// CONTENT kinds (story/data/scene/gui/map) may be auto-applied when the user
// opts in per kind. META kinds (project-config / project-scaffold / map-create)
// are NEVER auto-applied: they reshape the project itself (the config file, a
// whole scaffold, new map dirs), so a human must always review them —
// shouldAutoApply hard-codes that guard and the UI offers no toggle for them.
// ───────────────────────────────────────────────────────────────────────────

/** Proposal target kinds that edit game content (auto-appliable per opt-in). */
export const CONTENT_KINDS = ['story', 'data', 'scene', 'gui', 'map'] as const
export type ContentKind = (typeof CONTENT_KINDS)[number]

/** Proposal target kinds that reshape the project — manual review only, always. */
export const META_KINDS = ['project-config', 'project-scaffold', 'map-create'] as const

/** Per-kind on/off switches; only content kinds are switchable. */
export type AutoApplySettings = Record<ContentKind, boolean>

export function isMetaKind(kind: unknown): boolean {
  return (META_KINDS as readonly string[]).includes(String(kind))
}

/** Default switches — everything off (review-first), or all content kinds on. */
export function defaultAutoApplySettings(allOn = false): AutoApplySettings {
  return { story: allOn, data: allOn, scene: allOn, gui: allOn, map: allOn }
}

/**
 * Whether a proposal of `kind` may be applied without human review.
 * Meta kinds ALWAYS return false regardless of the switches — this is the hard
 * guard on the auto-apply path; never weaken it.
 */
export function shouldAutoApply(kind: unknown, settings: Partial<Record<string, boolean>>): boolean {
  if (isMetaKind(kind)) return false
  return settings[String(kind)] === true
}
