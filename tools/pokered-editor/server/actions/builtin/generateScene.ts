// ───────────────────────────────────────────────────────────────────────────
// Action: generate-scene — quest → `.scene` DSL via the tool-using scene writer.
// Wraps server/sceneWriter.ts generateScene, gathering the quest + characters +
// flags + target path through ProjectContext.
// ───────────────────────────────────────────────────────────────────────────
import type { AiAction, ActionContext } from '../types'
import { generateScene } from '../../sceneWriter'

/** System prompt: the scripting-agent rules + any inlined project context. */
function sceneSystemPrompt(aiContext: string): string {
  return [
    'You are a game-scripting agent. You write ONE `.scene` DSL file that implements a quest for a 2D JRPG built on a custom engine.',
    'Rules:',
    "- Before writing, study any provided DSL guide, API typings, and example scenes using the read_file / list_scenes tools, so your syntax matches THIS game's engine exactly. Do NOT invent APIs or syntax.",
    '- Implement the quest flow: gate availability on its required flags, set its flags at the right beats, grant rewards, and voice each character per their profile (personality + speech style).',
    "- Match the example scenes' conventions for player-facing text (including any bilingual form).",
    '- When the file is ready, call submit_scene exactly once with the COMPLETE file content and nothing else.',
    aiContext ? '\nProject context:\n' + aiContext : '',
  ].filter(Boolean).join('\n')
}

/** User prompt: the quest, involved characters, known flags, target + prior error. */
function sceneUserPrompt(
  quest: any, characters: any[], flags: string[], storyline: string,
  existing: string | null, previousError: string | null,
): string {
  const parts = [
    'Generate the `.scene` file for this quest.', '',
    'Quest (JSON):', JSON.stringify(quest, null, 2), '',
    'Characters involved (profiles, for dialogue voice):', JSON.stringify(characters, null, 2), '',
    "Event flags known in this game (use exact names; the quest's requires/sets are authoritative):",
    flags.join(', ') || '(none scanned)', '',
    `Target storyline / handler name: ${storyline || quest.id}`,
  ]
  if (existing) parts.push('', 'Existing file to revise (keep working parts):', existing)
  if (previousError) parts.push('', 'A previous attempt FAILED validation with this output — fix it:', previousError)
  parts.push('', 'Write the complete file and call submit_scene.')
  return parts.join('\n')
}

export const generateSceneAction: AiAction = {
  id: 'generate-scene',
  kind: 'agent',
  title: 'Generate quest scene',
  async run(ctx: ActionContext) {
    const { questId, sceneName, storyline, previousError } = ctx.input
    const { quest, characters, flags } = ctx.project.gatherForQuest(questId)
    if (!quest) throw new Error('Quest not found')
    const q = quest as any

    const aiContext = ctx.project.assembleContext()
    const targetScene = sceneName || q.implementedBy?.[0]?.scene || q.maps?.[0] || q.id
    const storylineName = storyline || q.implementedBy?.[0]?.storyline || q.id
    // resolveSceneRel: if the quest already names an existing scene (by stem or
    // path), revise THAT file rather than templating a fresh (possibly nested) path.
    const targetRel = ctx.project.resolveSceneRel(targetScene)
    const existing = ctx.project.readDataFileOrNull(targetRel)

    const content = await generateScene({
      profile: ctx.profile,
      apiKey: ctx.apiKey,
      system: sceneSystemPrompt(aiContext),
      prompt: sceneUserPrompt(q, characters, flags, storylineName, existing, previousError),
      readFile: async (p: string) => ctx.project.readFileSandboxed(p),
      listScenes: async () => ctx.project.listSceneDirs(),
      onEvent: (event, data: any) => {
        if (event === 'text') ctx.emit('text', { delta: data?.text ?? '' })
        else if (event === 'reasoning') ctx.emit('reasoning', { delta: data?.text ?? '' })
        else if (event === 'tool') ctx.emit('tool-call', { name: data?.name, path: data?.path })
        else if (event === 'usage') ctx.emit('usage', data)
        else if (event === 'error') ctx.emit('error', { message: data?.message })
      },
    })

    return { content, scene: targetScene, storyline: storylineName, targetRel }
  },
}
