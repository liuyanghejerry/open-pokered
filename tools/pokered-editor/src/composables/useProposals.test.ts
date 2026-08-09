// Per-hunk diff selection — the reconstruction that backs "Apply selected".
import { describe, it, expect } from 'vitest'
import { diffHunks, applyHunks, useProposals, type DiffOp } from './useProposals'

const d = (type: DiffOp['type'], text: string): DiffOp => ({ type, text })

// before: A B C D      after: A X C D E
//               (edit hunk B→X)   (add hunk E at the end)
const diff: DiffOp[] = [
  d('ctx', 'A'),
  d('del', 'B'), d('add', 'X'),
  d('ctx', 'C'),
  d('ctx', 'D'),
  d('add', 'E'),
]

describe('useProposals.replace', () => {
  it('swaps the tray contents and re-seeds uids past the restored ones', () => {
    const tray = useProposals() // no persistKey: in-memory only
    tray.replace([
      { uid: 'p7', target: {}, title: 'restored', diff: [], before: null, after: 'x', status: 'pending' },
    ])
    expect(tray.proposals.value.map(p => p.uid)).toEqual(['p7'])
    tray.add({ target: {}, title: 'new', diff: [], after: 'y' })
    // uid continues past the restored p7 instead of colliding with it
    expect(tray.proposals.value.map(p => p.uid)).toEqual(['p7', 'p8'])
    tray.replace([])
    expect(tray.proposals.value).toEqual([])
  })
})

describe('diffHunks / applyHunks', () => {
  it('groups changed runs into hunks bounded by context', () => {
    expect(diffHunks(diff)).toEqual([[1, 2], [5]])
  })
  it('accepting all hunks reproduces the full after', () => {
    expect(applyHunks(diff, new Set([0, 1]))).toBe(['A', 'X', 'C', 'D', 'E'].join('\n'))
  })
  it('accepting none reproduces before', () => {
    expect(applyHunks(diff, new Set())).toBe(['A', 'B', 'C', 'D'].join('\n'))
  })
  it('accepting only the first hunk applies B→X but drops the added E', () => {
    expect(applyHunks(diff, new Set([0]))).toBe(['A', 'X', 'C', 'D'].join('\n'))
  })
  it('accepting only the second hunk keeps B and adds E', () => {
    expect(applyHunks(diff, new Set([1]))).toBe(['A', 'B', 'C', 'D', 'E'].join('\n'))
  })
})
