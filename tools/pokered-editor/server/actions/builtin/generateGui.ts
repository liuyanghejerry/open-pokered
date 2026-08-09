// ───────────────────────────────────────────────────────────────────────────
// Action: generate-gui — NL → a complete `.gui` layout file.
//
// A tool-using agent (list_gui / read_gui / submit_gui) that studies the
// project's real layouts before writing, so syntax/components match the engine.
// Streams the standard vocab; returns { content }. The client applies it to the
// editor and validates with the WASM compiler (compileScreen) — on a compile
// error it re-invokes with `previousError`, closing the generate→compile→fix loop.
// ───────────────────────────────────────────────────────────────────────────
import type { AiAction, ActionContext } from '../types'
import type { ProjectContext } from '../../context/projectContext'
import { buildModel } from '../../ai'

function pickExamples(project: ProjectContext, n = 2): string {
  const sized = project.listGui()
    .map(name => { try { return { name, text: project.readGui(name) } } catch { return null } })
    .filter((s): s is { name: string; text: string } => !!s && s.text.length > 30)
    .sort((a, b) => a.text.length - b.text.length)
  return sized.slice(0, n).map(s => `# ${s.name}\n${s.text.slice(0, 2500)}`).join('\n\n')
}

function guiSystemPrompt(project: ProjectContext): string {
  const examples = pickExamples(project)
  return [
    'You are a UI-layout agent. You write ONE `.gui` DSL file for a 2D JRPG built on a custom engine.',
    'The `.gui` DSL is a declarative tile-grid + flex layout language (screen / panel / text / flex_list / cursor + custom:* components, with rect tile coordinates and style refs).',
    'Rules:',
    '- Study the existing `.gui` layouts with list_gui / read_gui so your components, properties, and syntax match THIS engine EXACTLY. Do NOT invent components or syntax.',
    '- Produce a COMPLETE, compilable file. When ready, call submit_gui exactly once with the full file content and nothing else.',
    examples ? '\nExample layouts from this project:\n' + examples : '',
  ].filter(Boolean).join('\n')
}

function guiUserPrompt(prompt: string, existing: string | null, previousError: string | null): string {
  const parts = [`Create a \`.gui\` layout for: ${prompt}`]
  if (existing && existing.trim()) parts.push('', 'Current file to revise (keep the working parts):', existing)
  if (previousError) parts.push('', 'The previous attempt FAILED to compile with this error — fix it:', previousError)
  parts.push('', 'Study existing layouts with the tools, then call submit_gui with the complete file.')
  return parts.join('\n')
}

function extractFenced(text: string): string {
  const m = text.match(/```(?:gui|dsl)?\s*\n([\s\S]*?)```/)
  return m ? m[1].trim() : ''
}

export const generateGuiAction: AiAction = {
  id: 'generate-gui',
  kind: 'agent',
  title: 'Generate GUI layout',
  async run(ctx: ActionContext) {
    const { streamText, tool, stepCountIs, hasToolCall } = await import('ai')
    const { z } = await import('zod')

    const prompt = String(ctx.input.prompt ?? '').trim()
    if (!prompt) throw new Error('prompt is required')
    const existing = typeof ctx.input.existingContent === 'string' ? ctx.input.existingContent : null
    const previousError = typeof ctx.input.previousError === 'string' ? ctx.input.previousError : null

    const model = await buildModel(ctx.profile, ctx.apiKey)
    let captured = ''
    let full = ''

    const tools = {
      list_gui: tool({
        description: 'List existing .gui layout file names.', inputSchema: z.object({}),
        execute: async () => { ctx.emit('tool-call', { name: 'list_gui' }); return ctx.project.listGui().join('\n') || '(none)' },
      }),
      read_gui: tool({
        description: 'Read a .gui layout by name for reference.', inputSchema: z.object({ name: z.string() }),
        execute: async ({ name }: { name: string }) => { ctx.emit('tool-call', { name: 'read_gui', path: name }); try { return ctx.project.readGui(name).slice(0, 9000) } catch (e) { return 'ERROR: ' + (e as Error).message } },
      }),
      submit_gui: tool({
        description: 'Submit the final, complete .gui file content. Call exactly once with only the file content.', inputSchema: z.object({ content: z.string() }),
        execute: async ({ content }: { content: string }) => { captured = content; return { ok: true } },
      }),
    }

    const result = streamText({
      model,
      system: guiSystemPrompt(ctx.project),
      prompt: guiUserPrompt(prompt, existing, previousError),
      tools,
      stopWhen: [stepCountIs(10), hasToolCall('submit_gui')],
      abortSignal: ctx.signal,
    })

    for await (const part of result.fullStream) {
      if (part.type === 'text-delta') { full += part.text; ctx.emit('text', { delta: part.text }) }
      else if (part.type === 'reasoning-delta') ctx.emit('reasoning', { delta: part.text })
      else if (part.type === 'error') ctx.emit('error', { message: String((part as any).error) })
    }
    try { ctx.emit('usage', await (result as any).totalUsage) } catch { /* usage unavailable */ }

    const content = captured || extractFenced(full)
    if (!content) throw new Error('The model did not produce a .gui file.')
    return { content }
  },
}
