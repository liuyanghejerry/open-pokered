import { StreamLanguage, foldService } from '@codemirror/language'

const DSL_KEYWORDS = new Set([
  'if', 'else', 'while', 'for', 'function', 'return',
  'await', 'async', 'const', 'let', 'var', 'true', 'false', 'null', 'undefined',
])

export function dslLanguage() {
  return StreamLanguage.define<{ inRun?: boolean }>({
    startState: () => ({ inRun: false }),

    token(stream, state) {
      if (stream.match('//')) {
        stream.skipToEnd()
        return 'comment'
      }

      if (stream.match(/@(storyline|speaker|choice|run|load|if|else|each|variables|theme|style|atlas|option|command|trigger)\b/)) {
        const directive = stream.current().slice(1)
        if (directive === 'run') state.inRun = true
        return 'keyword'
      }

      if (stream.match(/"(?:[^"\\]|\\.)*"/) || stream.match(/'(?:[^'\\]|\\.)*'/)) {
        return 'string'
      }

      if (stream.match(/0x[0-9a-fA-F]+/) || stream.match(/\d+(\.\d+)?/)) {
        return 'number'
      }

      if (stream.match(/[{}()[\]]/)) {
        if (stream.current() === '}') state.inRun = false
        return 'bracket'
      }

      if (stream.match(/[|&!=<>+\-*/%?:]+/)) {
        return 'operator'
      }

      if (stream.match(/[a-zA-Z_]\w*/)) {
        const word = stream.current()
        if (DSL_KEYWORDS.has(word)) {
          return 'atom'
        }
        if (word === 'game' || state.inRun) {
          return 'variableName'
        }
        return 'variableName'
      }

      stream.next()
      return null
    },
  })
}

/// Brace-based code folding for the DSL. A `StreamLanguage` provides no fold
/// ranges on its own, so `foldGutter()` would show nothing; this fold service
/// makes every multi-line `{ … }` block foldable — `@storyline { … }`,
/// `@if/@else { … }`, `@choice/@option { … }`, `@run/@load { … }`, `game_scene { … }`.
/// Braces inside strings and `//` comments are ignored.
export function dslFold() {
  return foldService.of((state, lineStart) => {
    const doc = state.doc
    const line = doc.lineAt(lineStart)
    // Last opening brace on this line that is not inside a string or comment.
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

    // Scan forward for the matching close brace.
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
            // Only fold blocks that actually span multiple lines.
            return ln > line.number ? { from, to: l.from + i } : null
          }
        }
      }
    }
    return null
  })
}
