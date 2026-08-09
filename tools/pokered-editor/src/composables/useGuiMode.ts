import { StreamLanguage, foldService, type StringStream } from '@codemirror/language'
import type { Text, EditorState } from '@codemirror/state'

// Keywords for .gui DSL
const GUI_COMPONENTS = new Set([
  'screen', 'panel', 'container', 'text', 'button', 'tile', 'divider',
  'flex_list', 'list', 'image', 'input', 'dropdown',
])

const GUI_PROPERTIES = new Set([
  'rect', 'style', 'value', 'color', 'font', 'wrap', 'line_spacing',
  'tile_id', 'tiles', 'repeat', 'orientation', 'cursor', 'selected',
  'max_visible', 'footer', 'item_template', 'item_layout', 'gap',
  'padding', 'clip', 'flip_x', 'flip_y', 'palette', 'visible', 'align',
  'on_click', 'source', 'layout',
])

export function guiLanguage() {
  return StreamLanguage.define({
    startState: () => ({}),
    token(stream: StringStream, _state: unknown) {
      // Comments
      if (stream.match('//')) {
        stream.skipToEnd()
        return 'comment'
      }

      // Strings (with template variable highlighting)
      if (stream.match(/"(?:[^"\\]|\\.)*"/) || stream.match(/'(?:[^'\\]|\\.)*'/)) {
        return 'string'
      }

      // Numbers
      if (stream.match(/0x[0-9a-fA-F]+/) || stream.match(/\d+(\.\d+)?/)) {
        return 'number'
      }

      // Brackets
      if (stream.match(/[{}()[\]]/)) {
        return 'bracket'
      }

      // Operators
      if (stream.match(/[|&!=<>+\-*/%?:,;]+/)) {
        return 'operator'
      }

      // Identifiers and keywords
      if (stream.match(/[a-zA-Z_]\w*/)) {
        const word = stream.current()
        if (GUI_COMPONENTS.has(word)) return 'keyword'
        if (GUI_PROPERTIES.has(word)) return 'propertyName'
        if (word === 'true' || word === 'false') return 'atom'
        return 'variableName'
      }

      stream.next()
      return null
    },
  })
}

/// Brace-based code folding for .gui files.
/// Same logic as dslFold() — makes every multi-line { … } block foldable.
export function guiFold() {
  return foldService.of((state: EditorState, lineStart: number) => {
    const doc: Text = state.doc
    const line = doc.lineAt(lineStart)
    let open = -1
    let str: string | null = null
    const t = line.text
    for (let i = 0; i < t.length; i++) {
      const c = t[i]
      if (str) {
        if (c === '\\') i++
        else if (c === str) str = null
        continue
      }
      if (c === '"' || c === "'") { str = c; continue }
      if (c === '/' && t[i + 1] === '/') break
      if (c === '{') open = i
    }
    if (open < 0) return null

    const from = line.from + open + 1
    let depth = 1
    let s: string | null = null
    for (let ln = line.number; ln <= doc.lines; ln++) {
      const l = doc.line(ln)
      const text = l.text
      for (let i = ln === line.number ? open + 1 : 0; i < text.length; i++) {
        const c = text[i]
        if (s) {
          if (c === '\\') i++
          else if (c === s) s = null
          continue
        }
        if (c === '"' || c === "'") { s = c; continue }
        if (c === '/' && text[i + 1] === '/') break
        if (c === '{') depth++
        else if (c === '}') {
          depth--
          if (depth === 0) {
            return ln > line.number ? { from, to: l.from + i } : null
          }
        }
      }
    }
    return null
  })
}
