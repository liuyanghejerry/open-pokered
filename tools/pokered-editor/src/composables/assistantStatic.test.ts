import { describe, it, expect, vi, beforeEach } from 'vitest'
import { lineDiff, buildStaticSystem, buildStaticTools, toolPartsFromSteps, autoApplyKindFor, type StaticProposalPayload } from './assistantStatic'
import type { DiffOp } from './useProposals'

// ── lineDiff (pure) ────────────────────────────────────────────────────────

function ops(before: string, after: string): { add: number; del: number; ctx: number } {
  const d = lineDiff(before, after)
  return {
    add: d.filter(o => o.type === 'add').reduce((n, o) => n + o.text.split('\n').length, 0),
    del: d.filter(o => o.type === 'del').reduce((n, o) => n + o.text.split('\n').length, 0),
    ctx: d.filter(o => o.type === 'ctx').reduce((n, o) => n + o.text.split('\n').length, 0),
  }
}

describe('lineDiff', () => {
  it('is empty for identical content', () => {
    expect(lineDiff('a\nb\nc', 'a\nb\nc')).toEqual([{ type: 'ctx', text: 'a\nb\nc' }])
  })

  it('detects an added line', () => {
    const d = ops('a\nc', 'a\nb\nc')
    expect(d.add).toBe(1)
    expect(d.del).toBe(0)
  })

  it('detects a removed line', () => {
    const d = ops('a\nb\nc', 'a\nc')
    expect(d.del).toBe(1)
    expect(d.add).toBe(0)
  })

  it('reports line-count deltas on a JSON edit', () => {
    const before = '{\n  "hp": 45,\n  "attack": 49\n}'
    const after = '{\n  "hp": 50,\n  "attack": 49\n}'
    const d = ops(before, after)
    expect(d.add).toBe(1)
    expect(d.del).toBe(1)
  })
})

describe('buildStaticSystem', () => {
  it('mentions the current activity and static-mode semantics', () => {
    const s = buildStaticSystem({ activity: 'pokemon', route: '/pokemon/Pikachu' })
    expect(s).toContain('pokemon')
    expect(s).toContain('static mode')
    expect(s).toContain('propose_*')
  })
})

// ── Tools (dataFetch mocked) ───────────────────────────────────────────────

const fetchMock = vi.fn()
vi.mock('./dataAdapter', () => ({
  dataFetch: (...args: unknown[]) => fetchMock(...args),
}))

function jsonBody(text: string | null) {
  return text === null
    ? { ok: false, status: 404, text: async () => '' }
    : { ok: true, status: 200, text: async () => text }
}

describe('buildStaticTools', () => {
  beforeEach(() => {
    fetchMock.mockReset()
  })

  it('read_pokemon returns the record text', async () => {
    fetchMock.mockResolvedValueOnce(jsonBody('{"species":"Pikachu","baseStats":{"hp":35}}'))
    const emit = vi.fn()
    const tools = await buildStaticTools(emit as any)
    const out = await (tools.read_pokemon as any).execute({ id: 'Pikachu' })
    expect(out).toContain('"species":"Pikachu"')
    expect(fetchMock).toHaveBeenCalledWith('/api/pokemon/Pikachu')
  })

  it('read_pokemon reports a missing record', async () => {
    fetchMock.mockResolvedValueOnce(jsonBody(null))
    const tools = await buildStaticTools(vi.fn() as any)
    const out = await (tools.read_pokemon as any).execute({ id: 'NopeMon' })
    expect(out).toContain('ERROR')
  })

  it('propose_pokemon_edit stages a proposal with before/after/diff', async () => {
    const before = '{"species":"Pikachu","baseStats":{"hp":35}}'
    fetchMock.mockResolvedValueOnce(jsonBody(before))
    const emit = vi.fn()
    const tools = await buildStaticTools(emit as any)
    const after = '{"species":"Pikachu","baseStats":{"hp":50}}'
    const out = await (tools.propose_pokemon_edit as any).execute({ id: 'Pikachu', content: after, rationale: 'buff' })
    expect(out).toContain('ok')
    const payload = emit.mock.calls[0][1] as StaticProposalPayload
    expect(payload.target).toEqual({ kind: 'pokemon', id: 'Pikachu', path: 'pokemon/Pikachu' })
    expect(payload.before).toBe(before)
    expect(payload.after).toBe(after)
    expect(payload.diff.some((o: DiffOp) => o.type === 'add')).toBe(true)
    expect(payload.diff.some((o: DiffOp) => o.type === 'del')).toBe(true)
  })

  it('propose_scene_edit maps to the map/script.scene target', async () => {
    fetchMock.mockResolvedValueOnce(jsonBody(null)) // no existing scene → create
    const emit = vi.fn()
    const tools = await buildStaticTools(emit as any)
    await (tools.propose_scene_edit as any).execute({ map: 'PalletTown', content: '@scene\n@say("hi")\n' })
    const payload = emit.mock.calls[0][1] as StaticProposalPayload
    expect(payload.target.kind).toBe('map')
    expect(payload.target.file).toBe('script.scene')
    expect(payload.target.map).toBe('PalletTown')
  })

  it('update_plan emits the plan payload', async () => {
    const emit = vi.fn()
    const tools = await buildStaticTools(emit as any)
    const out = await (tools.update_plan as any).execute({ steps: [{ title: 'read', status: 'active' }] })
    expect(out.ok).toBe(true)
    expect(emit).toHaveBeenCalledWith('plan', { steps: [{ title: 'read', status: 'active' }] })
  })

  it('rejects malformed JSON in propose tools', async () => {
    const tools = await buildStaticTools(vi.fn() as any)
    const out = await (tools.propose_pokemon_edit as any).execute({ id: 'Pikachu', content: 'not json' })
    expect(out).toContain('ERROR')
  })
})

// ── toolPartsFromSteps (chat round-trip across turns) ───────────────────────

describe('toolPartsFromSteps', () => {
  it('pairs each tool call with its result in the v6 ToolUIPart shape', () => {
    const parts = toolPartsFromSteps([
      {
        toolCalls: [{ toolCallId: 'c1', toolName: 'read_pokemon', input: { id: 'Pikachu' } }],
        toolResults: [{ toolCallId: 'c1', toolName: 'read_pokemon', output: '{"species":"Pikachu"}' }],
      },
    ])
    expect(parts).toEqual([
      {
        type: 'tool-read_pokemon', toolCallId: 'c1', toolName: 'read_pokemon',
        input: { id: 'Pikachu' }, state: 'output-available',
        output: '{"species":"Pikachu"}', providerExecuted: true,
      },
    ])
  })

  it('keeps a tool call without a result but never an undefined output', () => {
    const parts = toolPartsFromSteps([
      { toolCalls: [{ toolCallId: 'c2', toolName: 'read_map', input: { map: 'PalletTown' } }], toolResults: [] },
    ])
    expect(parts[0].output).not.toBeUndefined()
    expect(parts[0].state).toBe('output-available')
  })

  it('round-trips through the AI SDK so the next turn sees args + results', async () => {
    const { convertToModelMessages } = await import('ai')
    const parts = toolPartsFromSteps([
      {
        toolCalls: [{ toolCallId: 'c3', toolName: 'read_pokemon', input: { id: 'Pikachu' } }],
        toolResults: [{ toolCallId: 'c3', toolName: 'read_pokemon', output: '{"species":"Pikachu","baseStats":{"hp":35}}' }],
      },
    ])
    const out = await convertToModelMessages([
      { id: 'a1', role: 'assistant', parts },
      { id: 'u2', role: 'user', parts: [{ type: 'text', text: 'now edit it' }] },
    ] as any)
    const call = out[0].content.find((c: any) => c.type === 'tool-call')
    const result = out[0].content.find((c: any) => c.type === 'tool-result')
    expect(call.input).toEqual({ id: 'Pikachu' })
    expect(result.output?.value).toContain('baseStats')
  })
})

// ── autoApplyKindFor (static kinds → shared auto-apply switches) ────────────

describe('autoApplyKindFor', () => {
  it('maps static proposal kinds onto the shared switch vocabulary', () => {
    expect(autoApplyKindFor({ kind: 'pokemon' })).toBe('data')
    expect(autoApplyKindFor({ kind: 'move' })).toBe('data')
    expect(autoApplyKindFor({ kind: 'trainer' })).toBe('data')
    expect(autoApplyKindFor({ kind: 'item' })).toBe('data')
    expect(autoApplyKindFor({ kind: 'layout' })).toBe('gui')
    expect(autoApplyKindFor({ kind: 'map' })).toBe('map')
    expect(autoApplyKindFor(null)).toBe('data')
    expect(autoApplyKindFor(undefined)).toBe('data')
  })
})
