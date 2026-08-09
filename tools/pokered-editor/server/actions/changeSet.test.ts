import { describe, expect, it } from 'vitest'
import { ChangeSet, lineDiff } from './changeSet'

describe('lineDiff', () => {
  it('marks added / removed / context lines', () => {
    expect(lineDiff('a\nb\nc', 'a\nB\nc')).toEqual([
      { type: 'ctx', text: 'a' },
      { type: 'del', text: 'b' },
      { type: 'add', text: 'B' },
      { type: 'ctx', text: 'c' },
    ])
  })
  it('treats an empty before as all-add and equal inputs as all-context', () => {
    expect(lineDiff('', 'x\ny').every(o => o.type === 'add')).toBe(true)
    expect(lineDiff('p\nq', 'p\nq').every(o => o.type === 'ctx')).toBe(true)
  })
})

describe('ChangeSet', () => {
  it('adds proposals with incrementing ids, computed diff, and summaries', () => {
    const cs = new ChangeSet()
    const p1 = cs.add({ target: { kind: 'story', storyKind: 'characters', id: 'x', path: 'characters/x' }, title: 'Edit x', before: '{"a":1}', after: '{"a":2}' })
    expect(p1.id).toBe('p1')
    expect(p1.diff.some(o => o.type === 'add')).toBe(true)
    const p2 = cs.add({ target: { kind: 'gui', name: 'm', path: 'm' }, title: 'Create m', before: null, after: 'screen m {}' })
    expect(p2.id).toBe('p2')
    expect(cs.proposals.length).toBe(2)
    expect(cs.summaries()).toEqual([
      { id: 'p1', target: p1.target, title: 'Edit x' },
      { id: 'p2', target: p2.target, title: 'Create m' },
    ])
  })
})
