// ───────────────────────────────────────────────────────────────────────────
// Action: generate-scene-snippet — NL → a `.scene` DSL snippet to insert at the
// cursor in the script editor. A tool-using agent (list_scenes / read_scene /
// submit_snippet) grounded by assembleContext (real scenes), so the game.* API +
// syntax match the engine — directly attacking the hallucinated-API problem.
// ───────────────────────────────────────────────────────────────────────────
import type { AiAction, ActionContext } from '../types'
import type { ProjectContext } from '../../context/projectContext'
import { buildModel } from '../../ai'

function snippetSystem(project: ProjectContext): string {
  const context = project.assembleContext()
  return [
    'You write a `.scene` DSL snippet that an author will INSERT into an existing map script for a 2D JRPG.',
    'Rules:',
    '- Study existing scenes with list_scenes / read_scene so your syntax and the game.* API match THIS engine EXACTLY. Do NOT invent APIs, flags, or syntax.',
    '- Output ONLY the snippet to insert — no surrounding file scaffolding unless the request clearly implies a whole new block. Use real flag/character names where relevant.',
    '- When ready, call submit_snippet exactly once with only the snippet text.',
    context ? '\nProject context:\n' + context : '',
  ].filter(Boolean).join('\n')
}

function extractFenced(text: string): string {
  const m = text.match(/```(?:scene|dsl|js|javascript)?\s*\n([\s\S]*?)```/)
  return m ? m[1].trim() : ''
}

export const generateSceneSnippetAction: AiAction = {
  id: 'generate-scene-snippet',
  kind: 'agent',
  title: 'Generate scene snippet',
  async run(ctx: ActionContext) {
    const { streamText, tool, stepCountIs, hasToolCall } = await import('ai')
    const { z } = await import('zod')

    const prompt = String(ctx.input.prompt ?? '').trim()
    if (!prompt) throw new Error('prompt is required')
    const existing = typeof ctx.input.existingContent === 'string' ? ctx.input.existingContent.slice(0, 6000) : ''

    const model = await buildModel(ctx.profile, ctx.apiKey)
    let captured = ''
    let full = ''

    const tools = {
      list_scenes: tool({
        description: 'List existing scene file paths (for style/API reference).', inputSchema: z.object({}),
        execute: async () => { ctx.emit('tool-call', { name: 'list_scenes' }); return ctx.project.listScenes().map(s => s.path).join('\n') || '(none)' },
      }),
      read_scene: tool({
        description: 'Read a scene by its scenesDir-relative path.', inputSchema: z.object({ path: z.string() }),
        execute: async ({ path }: { path: string }) => { ctx.emit('tool-call', { name: 'read_scene', path }); try { return ctx.project.readScene(path).slice(0, 9000) } catch (e) { return 'ERROR: ' + (e as Error).message } },
      }),
      submit_snippet: tool({
        description: 'Submit the snippet to insert. Call exactly once with only the snippet text.', inputSchema: z.object({ content: z.string() }),
        execute: async ({ content }: { content: string }) => { captured = content; return { ok: true } },
      }),
    }

    const userPrompt = [
      `Write a .scene snippet for: ${prompt}`,
      existing ? '\nThe current file (for context and conventions):\n' + existing : '',
      '\nStudy existing scenes with the tools, then call submit_snippet.',
    ].filter(Boolean).join('\n')

    const result = streamText({
      model, system: snippetSystem(ctx.project), prompt: userPrompt, tools,
      stopWhen: [stepCountIs(8), hasToolCall('submit_snippet')], abortSignal: ctx.signal,
    })

    for await (const part of result.fullStream) {
      if (part.type === 'text-delta') { full += part.text; ctx.emit('text', { delta: part.text }) }
      else if (part.type === 'reasoning-delta') ctx.emit('reasoning', { delta: part.text })
      else if (part.type === 'error') ctx.emit('error', { message: String((part as any).error) })
    }
    try { ctx.emit('usage', await (result as any).totalUsage) } catch { /* usage unavailable */ }

    const content = captured || extractFenced(full)
    if (!content) throw new Error('The model did not produce a snippet.')
    return { content }
  },
}
