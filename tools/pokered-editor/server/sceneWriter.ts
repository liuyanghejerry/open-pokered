// ───────────────────────────────────────────────────────────────────────────
// Scene writer — generates a `.scene` DSL file that implements a quest.
//
// Pluggable backend (SceneWriterBackend) so the agent that authors scenes can
// be AI-SDK-direct now and an embedded OpenCode server later. The default
// aiSdkSceneWriter runs a small tool-using agent loop: it may read project
// files for style (read_file / list_scenes), then submits the finished file
// via the submit_scene tool. Validation/writing to disk is the caller's job.
// ───────────────────────────────────────────────────────────────────────────
import { buildModel, type ProviderProfile } from './ai'

export interface SceneGenRequest {
  profile: ProviderProfile
  apiKey: string
  /** System prompt (rules + any inlined project context). */
  system: string
  /** User prompt (the quest, characters, flags, target, prior error). */
  prompt: string
  /** Sandboxed reader for the read_file tool (path relative to project root). */
  readFile: (p: string) => Promise<string>
  /** Lister for the list_scenes tool. */
  listScenes: () => Promise<string[]>
  /** Streaming sink: ("text" | "reasoning" | "tool" | "error", payload). */
  onEvent: (event: string, data: unknown) => void
}

export interface SceneWriterBackend {
  name: string
  generate(req: SceneGenRequest): Promise<string>
}

/** Pull a fenced code block out of free text, as a fallback when the model
 *  writes the file inline instead of calling submit_scene. */
function extractFenced(text: string): string {
  const m = text.match(/```(?:scene|dsl|js|javascript)?\s*\n([\s\S]*?)```/)
  return m ? m[1].trim() : ''
}

export const aiSdkSceneWriter: SceneWriterBackend = {
  name: 'ai-sdk',
  async generate(req: SceneGenRequest): Promise<string> {
    const { streamText, tool, stepCountIs, hasToolCall } = await import('ai')
    const { z } = await import('zod')

    const model = await buildModel(req.profile, req.apiKey)

    let captured = ''
    let fullText = ''

    const tools = {
      read_file: tool({
        description:
          'Read a UTF-8 text file from the project (e.g. an existing scene for style, the DSL guide, or the game API types). Path is relative to the project root.',
        inputSchema: z.object({ path: z.string() }),
        execute: async ({ path }: { path: string }) => {
          try {
            const text = await req.readFile(path)
            req.onEvent('tool', { name: 'read_file', path, bytes: text.length })
            return text.slice(0, 9000)
          } catch (e) {
            return 'ERROR: ' + (e as Error).message
          }
        },
      }),
      list_scenes: tool({
        description: 'List existing scene names in the project for reference.',
        inputSchema: z.object({}),
        execute: async () => {
          const list = await req.listScenes()
          req.onEvent('tool', { name: 'list_scenes', count: list.length })
          return list.join('\n')
        },
      }),
      submit_scene: tool({
        description:
          'Submit the final, complete .scene DSL file content. Call this exactly once when the file is ready. Do not include explanations — only the file content.',
        inputSchema: z.object({ content: z.string() }),
        execute: async ({ content }: { content: string }) => {
          captured = content
          return { ok: true }
        },
      }),
    }

    const result = streamText({
      model,
      system: req.system,
      prompt: req.prompt,
      tools,
      stopWhen: [stepCountIs(10), hasToolCall('submit_scene')],
    })

    for await (const part of result.fullStream) {
      if (part.type === 'text-delta') { fullText += part.text; req.onEvent('text', { text: part.text }) }
      else if (part.type === 'reasoning-delta') req.onEvent('reasoning', { text: part.text })
      else if (part.type === 'tool-call') req.onEvent('tool', { name: part.toolName })
      else if (part.type === 'error') req.onEvent('error', { message: String((part as any).error) })
    }

    try { req.onEvent('usage', await ((result as any).totalUsage ?? (result as any).usage)) } catch { /* usage unavailable */ }

    const content = captured || extractFenced(fullText)
    if (!content) throw new Error('The model did not produce a scene (no submit_scene call and no code block).')
    return content
  },
}

/** Registry of available backends. Add an OpenCode-backed writer here later. */
export function getSceneWriter(name?: string): SceneWriterBackend {
  switch (name) {
    // case 'opencode': return openCodeSceneWriter   // Phase 2.x: embed `opencode serve`
    default:
      return aiSdkSceneWriter
  }
}

export async function generateScene(req: SceneGenRequest, backendName?: string): Promise<string> {
  return getSceneWriter(backendName).generate(req)
}
