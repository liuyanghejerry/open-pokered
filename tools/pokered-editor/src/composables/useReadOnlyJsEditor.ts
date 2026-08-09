import { ref, onUnmounted, type Ref } from 'vue'
import { EditorState } from '@codemirror/state'
import { EditorView, lineNumbers } from '@codemirror/view'
import { javascript } from '@codemirror/lang-javascript'
import { oneDark } from '@codemirror/theme-one-dark'
import {
  syntaxHighlighting,
  defaultHighlightStyle,
} from '@codemirror/language'

/**
 * A minimal read-only CodeMirror 6 editor for JavaScript, used by the
 * compiled-JS preview pane in the script editor. It mirrors the styling of the
 * main editor (oneDark + line numbers) but is non-editable.
 */
export function useReadOnlyJsEditor(containerRef: Ref<HTMLElement | null>) {
  const view = ref<EditorView | null>(null)

  function create(initialContent: string) {
    destroy()
    const container = containerRef.value
    if (!container) return

    const state = EditorState.create({
      doc: initialContent,
      extensions: [
        lineNumbers(),
        javascript(),
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        oneDark,
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
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
    if (v.state.doc.toString() === content) return
    v.dispatch({
      changes: { from: 0, to: v.state.doc.length, insert: content },
    })
  }

  function destroy() {
    if (view.value) {
      view.value.destroy()
      view.value = null
    }
  }

  onUnmounted(destroy)

  return { view, create, setContent, destroy }
}
