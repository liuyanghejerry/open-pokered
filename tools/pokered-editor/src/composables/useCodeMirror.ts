import { ref, onUnmounted, type Ref } from 'vue'
import { EditorState } from '@codemirror/state'
import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view'
import { json, jsonParseLinter } from '@codemirror/lang-json'
import { jsonSchema } from 'codemirror-json-schema'
import { oneDark } from '@codemirror/theme-one-dark'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search'
import {
  syntaxHighlighting,
  defaultHighlightStyle,
  bracketMatching,
  foldGutter,
  foldKeymap,
} from '@codemirror/language'
import { linter } from '@codemirror/lint'
import { dslLanguage, dslFold } from './useDslMode'
import { guiLanguage, guiFold } from './useGuiMode'
import { dslLinter } from './useDslLinter'
import layoutSchema from '../schemas/layout.schema.json'

export interface ScriptFunction {
  name: string
  line: number
  exported: boolean
}

export type { DslBlock } from './parseDsl'
export { parseDsl } from './parseDsl'

const FUNC_RE = /^(export\s+)?(async\s+)?function\s+(\w+)/

export function parseFunctions(content: string): ScriptFunction[] {
  const result: ScriptFunction[] = []
  const lines = content.split('\n')
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(FUNC_RE)
    if (m) {
      result.push({
        name: m[3],
        line: i + 1,
        exported: !!m[1],
      })
    }
  }
  return result
}

export function useCodeMirror(
  containerRef: Ref<HTMLElement | null>,
  onChange: (content: string) => void,
) {
  const view = ref<EditorView | null>(null)

  function create(initialContent: string, mode: 'js' | 'dsl' | 'gui' = 'js') {
    destroy()
    const container = containerRef.value
    if (!container) return

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChange(update.state.doc.toString())
      }
    })

    const langExtensions = mode === 'gui'
      ? [guiLanguage(), guiFold()]
      : mode === 'dsl'
      ? [dslLanguage(), dslFold(), dslLinter()]
      : [json(), linter(jsonParseLinter()), jsonSchema(layoutSchema)]

    const state = EditorState.create({
      doc: initialContent,
      extensions: [
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        history(),
        bracketMatching(),
        foldGutter(),
        highlightSelectionMatches(),
        ...langExtensions,
        oneDark,
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        keymap.of([
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          ...foldKeymap,
        ]),
        updateListener,
        EditorView.theme({
          '&': { height: '100%' },
          '.cm-scroller': { overflow: 'auto' },
        }),
      ],
    })

    view.value = new EditorView({ state, parent: container })
  }

  function setContent(content: string) {
    const v = view.value
    if (!v) return
    const current = v.state.doc.toString()
    if (current === content) return
    v.dispatch({
      changes: { from: 0, to: v.state.doc.length, insert: content },
    })
  }

  function getContent(): string {
    return view.value?.state.doc.toString() ?? ''
  }

  function jumpToLine(lineNumber: number) {
    const v = view.value
    if (!v) return
    const lineCount = v.state.doc.lines
    if (lineNumber < 1 || lineNumber > lineCount) return
    const line = v.state.doc.line(lineNumber)
    v.dispatch({
      selection: { anchor: line.from },
      effects: EditorView.scrollIntoView(line.from, { y: 'start', yMargin: 50 }),
    })
    v.focus()
  }

  function jumpToFunction(funcName: string) {
    const v = view.value
    if (!v) return
    const content = v.state.doc.toString()
    const funcs = parseFunctions(content)
    const target = funcs.find(f => f.name === funcName)
    if (target) {
      jumpToLine(target.line)
    }
  }

  function destroy() {
    if (view.value) {
      view.value.destroy()
      view.value = null
    }
  }

  onUnmounted(destroy)

  return {
    view,
    create,
    setContent,
    getContent,
    jumpToLine,
    jumpToFunction,
    destroy,
  }
}
