<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { MergeView } from '@codemirror/merge'
import { EditorView, lineNumbers } from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { javascript } from '@codemirror/lang-javascript'
import { oneDark } from '@codemirror/theme-one-dark'
import { syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language'

const props = defineProps<{
  /** Original (left) — typically the on-disk saved snapshot */
  original: string
  /** Modified (right) — typically the current edited rawJson */
  modified: string
  /** Label for the original side */
  originalLabel?: string
  /** Label for the modified side */
  modifiedLabel?: string
}>()

defineEmits<{
  close: []
}>()

const container = ref<HTMLElement | null>(null)
let mergeView: MergeView | null = null

function buildExtensions() {
  return [
    lineNumbers(),
    javascript(),
    oneDark,
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    EditorView.editable.of(false),
    EditorState.readOnly.of(true),
    EditorView.theme({
      '&': { height: '100%' },
      '.cm-scroller': { overflow: 'auto' },
    }),
  ]
}

function create() {
  if (!container.value) return
  destroy()
  mergeView = new MergeView({
    a: { doc: props.original, extensions: buildExtensions() },
    b: { doc: props.modified, extensions: buildExtensions() },
    parent: container.value,
    revertControls: undefined, // both sides read-only; no revert UI
    highlightChanges: true,
    gutter: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
  })
}

function destroy() {
  if (mergeView) {
    mergeView.destroy()
    mergeView = null
  }
}

onMounted(create)
onUnmounted(destroy)

// Recreate when content changes — MergeView doesn't support live doc swap cleanly
watch(() => [props.original, props.modified], () => {
  create()
})
</script>

<template>
  <div class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4" @click.self="$emit('close')">
    <div class="bg-bg border border-accent rounded-lg shadow-2xl w-full max-w-7xl h-[85vh] flex flex-col">
      <div class="flex justify-between items-center p-3 border-b border-accent shrink-0">
        <div class="flex gap-6 items-center text-xs">
          <span class="font-bold text-text">Diff vs saved</span>
          <span class="text-text-muted">
            <span class="inline-block w-3 h-3 align-middle mr-1 bg-[#3a4a3a] border border-[#5a7a5a]" /> {{ modifiedLabel ?? 'Current (edited)' }}
          </span>
          <span class="text-text-muted">
            <span class="inline-block w-3 h-3 align-middle mr-1 bg-[#4a3a3a] border border-[#7a5a5a]" /> {{ originalLabel ?? 'Saved on disk' }}
          </span>
        </div>
        <button
          class="px-3 py-1 bg-[#333] text-text rounded text-[11px] font-bold hover:bg-[#444]"
          @click="$emit('close')"
        >
          ✕ Close
        </button>
      </div>
      <div ref="container" class="flex-1 min-h-0 overflow-hidden diff-merge-host" />
    </div>
  </div>
</template>

<style scoped>
.diff-merge-host :deep(.cm-mergeView),
.diff-merge-host :deep(.cm-mergeViewEditors) {
  height: 100%;
}
.diff-merge-host :deep(.cm-editor) {
  height: 100%;
}
</style>
