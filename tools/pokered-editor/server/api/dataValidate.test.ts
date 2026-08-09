import { describe, expect, it } from 'vitest'
import { validateDataSave } from './dataValidate'

const base = { idField: 'id', fileName: 'zuiquan.json', existing: [] as Array<{ file: string; id: unknown }> }

describe('validateDataSave', () => {
  it('accepts a well-formed record', () => {
    const r = validateDataSave({ ...base, body: JSON.stringify({ id: '醉拳', power: 100 }) })
    expect(r.ok).toBe(true)
    if (r.ok) expect(r.json.power).toBe(100)
  })

  it('rejects malformed JSON', () => {
    const r = validateDataSave({ ...base, body: '{not json' })
    expect(r.ok).toBe(false)
    if (!r.ok) expect(r.error).toMatch(/not valid JSON/)
  })

  it('rejects a JSON value that is not an object (the "string" footgun)', () => {
    const r = validateDataSave({ ...base, body: '"just a string"' })
    expect(r.ok).toBe(false)
    if (!r.ok) expect(r.error).toMatch(/must be a JSON object/)
  })

  it('rejects an array body', () => {
    expect(validateDataSave({ ...base, body: '[1,2]' }).ok).toBe(false)
  })

  it('rejects a missing/empty id field', () => {
    expect(validateDataSave({ ...base, body: JSON.stringify({ power: 1 }) }).ok).toBe(false)
    expect(validateDataSave({ ...base, body: JSON.stringify({ id: '   ' }) }).ok).toBe(false)
  })

  it('rejects a duplicate id used by another file', () => {
    const r = validateDataSave({
      ...base,
      fileName: 'new.json',
      body: JSON.stringify({ id: '醉拳' }),
      existing: [{ file: 'zuiquan.json', id: '醉拳' }],
    })
    expect(r.ok).toBe(false)
    if (!r.ok) expect(r.error).toMatch(/already used by "zuiquan.json"/)
  })

  it('allows re-saving the SAME file with its own id (edit, not a clash)', () => {
    const r = validateDataSave({
      ...base,
      fileName: 'zuiquan.json',
      body: JSON.stringify({ id: '醉拳', power: 120 }),
      existing: [{ file: 'zuiquan.json', id: '醉拳' }],
    })
    expect(r.ok).toBe(true)
  })

  it('honors a custom idField', () => {
    const r = validateDataSave({ idField: 'key', fileName: 'a.json', existing: [], body: JSON.stringify({ key: 'a' }) })
    expect(r.ok).toBe(true)
  })
})
