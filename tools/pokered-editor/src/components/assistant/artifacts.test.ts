// Session artifacts: derived from the proposal tray (applied-only), with
// per-kind icons/jump targets and the aggregate summary line.
import { describe, it, expect } from 'vitest'
import { buildArtifacts, summarize } from './artifacts'
import type { AssistantProposal, DiffOp } from '../../composables/useProposals'

let seq = 0
function prop(
  kind: string, status: AssistantProposal['status'],
  diff: DiffOp[] = [], target: Record<string, unknown> = {},
): AssistantProposal {
  return {
    uid: `u${++seq}`, target: { kind, path: `${kind}/thing`, ...target },
    title: `${kind} thing`, diff, before: null, after: '', status,
  }
}

const d = (type: DiffOp['type'], text: string): DiffOp => ({ type, text })

describe('buildArtifacts', () => {
  it('keeps only APPLIED proposals (pending/reverted/failed/conflict drop out)', () => {
    const list = buildArtifacts([
      prop('scene', 'applied'),
      prop('scene', 'reverted'), // was applied, then reverted → excluded
      prop('data', 'pending'),
      prop('data', 'failed'),
      prop('data', 'conflict'),
    ])
    expect(list.map(a => a.kind)).toEqual(['scene'])
  })

  it('maps each kind to an icon and its activity type; scaffold is not navigable', () => {
    const list = buildArtifacts([
      prop('story', 'applied'), prop('data', 'applied'), prop('scene', 'applied'),
      prop('gui', 'applied'), prop('map', 'applied'), prop('map-create', 'applied'),
      prop('project-config', 'applied'), prop('project-scaffold', 'applied'),
    ])
    const byKind = Object.fromEntries(list.map(a => [a.kind, a]))
    expect(byKind.story.activityType).toBe('story')
    expect(byKind.data.activityType).toBe('data')
    expect(byKind.scene.activityType).toBe('script')
    expect(byKind.gui.activityType).toBe('ui')
    expect(byKind.map.activityType).toBe('map')
    expect(byKind['map-create'].activityType).toBe('map')
    expect(byKind['project-config'].activityType).toBe('settings')
    expect(byKind['project-scaffold'].activityType).toBeNull()
    expect(byKind.story.icon).toBe('📖')
    expect(byKind['project-scaffold'].icon).toBe('🏗')
  })

  it('counts +/- diff ops per row and falls back to the title without a path', () => {
    const diff = [d('ctx', 'a'), d('del', 'b'), d('add', 'x'), d('add', 'y')]
    const [a] = buildArtifacts([prop('scene', 'applied', diff, { path: undefined })])
    expect(a.add).toBe(2)
    expect(a.del).toBe(1)
    expect(a.path).toBe('scene thing')
  })
})

describe('summarize', () => {
  it('aggregates files (one per applied proposal) and summed line counts', () => {
    const artifacts = buildArtifacts([
      prop('scene', 'applied', [d('add', 'x'), d('add', 'y'), d('del', 'z')]),
      prop('data', 'applied', [d('add', 'x')]),
      prop('map-create', 'applied'), // no diff → +0/−0
    ])
    expect(summarize(artifacts)).toEqual({ files: 3, add: 3, del: 1 })
    expect(summarize([])).toEqual({ files: 0, add: 0, del: 0 })
  })
})
