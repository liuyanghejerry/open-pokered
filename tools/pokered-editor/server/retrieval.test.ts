import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { createProjectContext } from './context/projectContext'
import { buildCorpus, cosineSim, topK, type Chunk } from './retrieval'

let ROOT = ''
function write(rel: string, content: string) {
  const abs = path.join(ROOT, rel)
  fs.mkdirSync(path.dirname(abs), { recursive: true })
  fs.writeFileSync(abs, content, 'utf-8')
}

beforeAll(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-rag-'))
  write('.jrpg-editor.json', JSON.stringify({
    name: 'F', dataRoot: '.', activities: [
      { id: 'story', type: 'story', config: { storiesDir: 'data/story', scenesDir: 'data/maps', scene: { ext: '.scene' } } },
      { id: 'data', type: 'data', config: { tables: [{ id: 'skills', dir: 'data/skills', idField: 'id' }] } },
    ],
  }))
  write('data/story/characters/hero.json', JSON.stringify({ id: 'hero', name: 'Hero' }))
  write('data/story/quests/q1.json', JSON.stringify({ id: 'rescue', name: 'Rescue' }))
  write('data/maps/Town/script.scene', '@storyline("town")\n')
  write('data/skills/fireball.json', JSON.stringify({ id: 'fireball', power: 10 }))
})
afterAll(() => { try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ } })

describe('buildCorpus', () => {
  it('emits one chunk per record/scene across kinds', () => {
    const corpus = buildCorpus(createProjectContext(ROOT))
    const kinds = new Set(corpus.map(c => c.kind))
    expect(kinds).toEqual(new Set(['character', 'quest', 'scene', 'data']))
    expect(corpus.find(c => c.id === 'character:hero')?.text).toContain('Hero')
    expect(corpus.find(c => c.id === 'data:skills:fireball')?.text).toContain('power')
  })
})

describe('cosineSim', () => {
  it('is 1 for identical, 0 for orthogonal vectors', () => {
    expect(cosineSim([1, 2, 3], [1, 2, 3])).toBeCloseTo(1)
    expect(cosineSim([1, 0], [0, 1])).toBeCloseTo(0)
    expect(cosineSim([0, 0], [1, 1])).toBe(0)
  })
})

describe('topK', () => {
  it('returns the nearest chunks by cosine similarity', () => {
    const chunks: Chunk[] = [
      { id: 'a', kind: 'x', text: 'a' },
      { id: 'b', kind: 'x', text: 'b' },
      { id: 'c', kind: 'x', text: 'c' },
    ]
    const vectors = [[1, 0], [0.9, 0.1], [0, 1]]
    const hits = topK([1, 0], chunks, vectors, 2)
    expect(hits.map(h => h.id)).toEqual(['a', 'b'])
  })
})
