// ───────────────────────────────────────────────────────────────────────────
// Quick prompts — context-aware canned instructions rendered as chips above
// the assistant input. The matching logic is pure (unit-tested); AssistantPanel
// sends the prompt text exactly like a typed user message.
// ───────────────────────────────────────────────────────────────────────────

/** One clickable chip: icon + label + the prompt text to send. */
export interface QuickPrompt {
  id: string
  icon: string
  label: string
  prompt: string
}

/** What the panel is looking at when the chips render. */
export interface QuickPromptContext {
  /** Active activity id ('map' | 'script' | 'pokemon' | …), null if none. */
  activity: string | null
}

/** Cap on simultaneously visible chips; first matches win, in definition order. */
export const MAX_QUICK_PROMPTS = 4

/**
 * The chips matching the current context. Each prompt's text is a natural-
 * language instruction aligned with the agent's tools (read + propose_* tools).
 */
export function quickPromptsFor(ctx: QuickPromptContext): QuickPrompt[] {
  const out: QuickPrompt[] = []

  if (ctx.activity === 'map') {
    out.push({
      id: 'new-map', icon: '🗺', label: 'New map',
      prompt: 'Help me create a new map. Ask about its purpose and size first, then give me a proposal.',
    })
  }

  if (ctx.activity === 'script') {
    out.push({
      id: 'write-scene', icon: '📝', label: 'Write a scene',
      prompt: 'Help me write a new .scene script for this map. Ask what story beat it should serve first, then give me a proposal.',
    })
  }

  if (ctx.activity === 'pokemon') {
    out.push({
      id: 'tune-pokemon', icon: '🐾', label: 'Tune species',
      prompt: 'Review the open Pokémon species data (base stats, learnset, evolutions) and propose balance tweaks.',
    })
  }

  if (ctx.activity === 'move') {
    out.push({
      id: 'design-move', icon: '⚔', label: 'Design move',
      prompt: 'Review the open move data (power, accuracy, effect) and propose improvements.',
    })
  }

  if (ctx.activity === 'trainer') {
    out.push({
      id: 'trainer-team', icon: '🎓', label: 'Trainer team',
      prompt: 'Review the open trainer class and propose a stronger party (species, levels, movesets).',
    })
  }

  // Fallback: nothing context-specific matched.
  if (!out.length) {
    out.push({
      id: 'whats-next', icon: '🧭', label: "What's next",
      prompt: 'What should I do next? Look at the current project state and plan the next three steps for me.',
    })
  }

  return out.slice(0, MAX_QUICK_PROMPTS)
}
