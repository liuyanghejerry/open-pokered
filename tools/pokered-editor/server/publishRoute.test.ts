import path from 'path'
import { describe, expect, it } from 'vitest'
import { collectGameDataFiles, publishOutDir } from './publishRoute'

// These tests run against the real workspace root (server/api/projectConfig
// derives it from this module's import.meta.url — see the comment there), so
// they exercise the actual crates/pokered-data tree.
describe('collectGameDataFiles', () => {
  const files = collectGameDataFiles()

  it('collects the full replayable data set', () => {
    const paths = new Set(files.map((f) => f.path))
    expect(paths.has('maps/PalletTown/map.json')).toBe(true)
    expect(paths.has('maps/PalletTown/map.blk')).toBe(true)
    expect(paths.has('maps/PalletTown/script.scene')).toBe(true)
    // All 248 maps, and the data tables.
    expect(files.filter((f) => f.path.endsWith('/map.json')).length).toBeGreaterThan(200)
    expect(paths.has('trainers/Brock.json')).toBe(true)
    expect(files.some((f) => f.path.startsWith('pokemon/'))).toBe(true)
  })

  it('encodes the binary map.blk as a number-array JSON string', () => {
    const blk = files.find((f) => f.path === 'maps/PalletTown/map.blk')!
    const parsed: unknown = JSON.parse(blk.content)
    expect(Array.isArray(parsed)).toBe(true)
    expect((parsed as unknown[]).every((n) => typeof n === 'number')).toBe(true)
  })

  it('returns text content for every entry (player replay contract)', () => {
    for (const f of files) {
      expect(typeof f.content, f.path).toBe('string')
      expect(f.content.startsWith('data:'), f.path).toBe(false)
    }
  })
})

describe('publishOutDir', () => {
  it('lives under <projectRoot>/dist/publish', () => {
    expect(publishOutDir().endsWith(path.join('dist', 'publish'))).toBe(true)
  })
})
