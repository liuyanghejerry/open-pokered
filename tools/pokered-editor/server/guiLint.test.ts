import { describe, expect, it } from 'vitest'
import { lintGui } from './guiLint'

describe('lintGui', () => {
  const valid = `// a menu
screen Bag {
  text("行囊") {
    rect = {tx: 2, ty: 1, tw: 20, th: 2}
    color = "#F0D070"
  }
  flex_list("{items}") {
    rect = {tx: 2, ty: 4, tw: 44, th: 18}
    item_layout = [{field: "name", width: 32}]
  }
}
`

  it('passes a well-formed layout', () => {
    expect(lintGui(valid)).toHaveLength(0)
  })

  it('flags an empty buffer', () => {
    expect(lintGui('   \n  ')[0].message).toMatch(/empty/i)
  })

  it('flags a truncated file (unclosed brace)', () => {
    const f = lintGui('screen X {\n  text("hi") {\n    rect = {tx: 1}\n')
    expect(f.some(x => /[Uu]nbalanced brace/.test(x.message))).toBe(true)
  })

  it('flags an extra closing bracket', () => {
    const f = lintGui('screen X {\n  item_layout = [a]]\n}\n')
    expect(f.some(x => /[Uu]nbalanced bracket/.test(x.message))).toBe(true)
  })

  it('flags a file with no top-level block', () => {
    const f = lintGui('text("orphan") { rect = {tx: 1, ty: 1} }\n')
    expect(f.some(x => /top-level block/.test(x.message))).toBe(true)
  })

  it('does not count delimiters inside strings or comments', () => {
    // braces/brackets inside a string value and a comment must not unbalance.
    const src = 'screen X {\n  text("a } ] ) string") { // a } comment\n    rect = {tx: 1, ty: 1}\n  }\n}\n'
    expect(lintGui(src)).toHaveLength(0)
  })
})
