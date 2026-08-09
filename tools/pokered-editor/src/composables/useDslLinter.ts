import { linter, type Diagnostic } from '@codemirror/lint'
import type { EditorView } from '@codemirror/view'
import { useWasmPreview } from './useWasmPreview'

/**
 * A CodeMirror 6 `linter()` extension for `.scene` DSL files.
 *
 * It compiles the current document with the Rust→WASM DSL compiler
 * (`compile_scene`) and, on failure, surfaces the reported `line:col` error as
 * an inline error diagnostic. CodeMirror debounces invocations via the `delay`
 * option, so this only runs ~300ms after the user stops typing.
 *
 * WASM init is lazy + shared through `useWasmPreview`, so importing this does
 * not eagerly load the module.
 */
export function dslLinter() {
  const { compileScene } = useWasmPreview()

  return linter(
    async (view: EditorView): Promise<Diagnostic[]> => {
      const doc = view.state.doc
      const source = doc.toString()
      // An empty scene is trivially valid — nothing to compile.
      if (source.trim().length === 0) return []

      let result
      try {
        result = await compileScene(source)
      } catch (e) {
        // WASM failed to load / threw — report at the top of the document so
        // the user knows validation is unavailable rather than silently passing.
        return [{
          from: 0,
          to: Math.min(doc.length, doc.line(1).to),
          severity: 'warning',
          message: `DSL validator unavailable: ${(e as Error).message}`,
        }]
      }

      if (result.ok) return []

      // Clamp the reported position into the document and map line/col → offset.
      const lineNo = Math.min(Math.max(result.line, 1), doc.lines)
      const line = doc.line(lineNo)
      // col is 1-based; offset within the line is col-1, clamped to line length.
      const colOffset = Math.min(Math.max(result.col - 1, 0), line.length)
      const from = line.from + colOffset
      // Highlight from the error column to the end of the line for visibility.
      const to = line.to > from ? line.to : Math.min(doc.length, from + 1)

      return [{
        from,
        to,
        severity: 'error',
        message: result.error,
      }]
    },
    { delay: 300 },
  )
}
