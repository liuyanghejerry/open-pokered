<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'
import { useCodeMirror, parseFunctions, parseDsl } from '../composables/useCodeMirror'
import { useReadOnlyJsEditor } from '../composables/useReadOnlyJsEditor'
import { useWasmPreview } from '../composables/useWasmPreview'
import { injectSceneScript } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import {
  sharedScriptFunctions,
  sharedActiveFunction,
  sharedDslBlocks,
  sharedActiveDslBlock,
  sharedScriptMode,
} from '../composables/useScriptState'

const store = useMapStore()
const playtestOverlay = usePlaytestOverlay()
const { currentMap, scriptEditorOpen, scriptJumpTarget, scriptDirty, sceneDirty } = storeToRefs(store)

const editorContainer = ref<HTMLElement | null>(null)
const isDslMode = ref(false)

// Compiled-JS preview state (DSL mode only).
const previewOpen = ref(false)
const previewContainer = ref<HTMLElement | null>(null)
const previewError = ref<string | null>(null)
const previewEditor = useReadOnlyJsEditor(previewContainer)
const { compileScene } = useWasmPreview()
let previewTimer: ReturnType<typeof setTimeout> | null = null

const cm = useCodeMirror(editorContainer, (content) => {
  if (currentMap.value) {
    if (isDslMode.value) {
      store.updateSceneContent(currentMap.value.name, content)
      sharedDslBlocks.value = parseDsl(content)
      schedulePreview(content)
    } else {
      store.updateScriptContent(currentMap.value.name, content)
      sharedScriptFunctions.value = parseFunctions(content)
    }
  }
})

// Debounced compile → refresh the read-only preview editor.
function schedulePreview(content: string) {
  if (!previewOpen.value) return
  if (previewTimer) clearTimeout(previewTimer)
  previewTimer = setTimeout(() => { void runPreview(content) }, 300)
}

async function runPreview(content: string) {
  if (!previewOpen.value) return
  try {
    const result = await compileScene(content)
    if (result.ok) {
      previewError.value = null
      previewEditor.setContent(result.output)
    } else {
      previewError.value = `${result.line}:${result.col}: ${result.error}`
      previewEditor.setContent('')
    }
  } catch (e) {
    previewError.value = `Compiler unavailable: ${(e as Error).message}`
    previewEditor.setContent('')
  }
}

async function togglePreview() {
  previewOpen.value = !previewOpen.value
  if (previewOpen.value) {
    await nextTick()
    previewEditor.create('')
    void runPreview(cm.getContent())
  } else {
    previewEditor.destroy()
  }
}

const mapName = computed(() => currentMap.value?.name ?? '')

watch([mapName, scriptEditorOpen], async ([name, open]) => {
  if (!name || !open) return

  const sceneContent = await store.loadSceneFile(name)
  if (sceneContent) {
    isDslMode.value = true
    sharedScriptMode.value = 'dsl'
    await nextTick()
    if (editorContainer.value) {
      cm.create(sceneContent, 'dsl')
      sharedDslBlocks.value = parseDsl(sceneContent)
      sharedActiveDslBlock.value = null
      sharedScriptFunctions.value = []
    }
    // Refresh the compiled-JS preview for the newly loaded scene.
    if (previewOpen.value) {
      await nextTick()
      previewEditor.create('')
      void runPreview(sceneContent)
    }
  } else {
    isDslMode.value = false
    sharedScriptMode.value = 'js'
    // The compiled-JS preview only applies to DSL files; tear it down.
    if (previewOpen.value) {
      previewOpen.value = false
      previewEditor.destroy()
    }
    const content = await store.loadScriptFile(name)
    await nextTick()
    if (editorContainer.value) {
      cm.create(content, 'js')
      sharedScriptFunctions.value = parseFunctions(content)
      sharedActiveFunction.value = null
      sharedDslBlocks.value = []
    }
  }

  if (scriptJumpTarget.value) {
    if (isDslMode.value) {
      const blocks = parseDsl(sceneContent || '')
      const target = blocks.find(b => b.name === scriptJumpTarget.value)
      if (target) cm.jumpToLine(target.line)
    } else {
      cm.jumpToFunction(scriptJumpTarget.value)
      sharedActiveFunction.value = scriptJumpTarget.value
    }
    store.clearJumpTarget()
  }
}, { immediate: true })

watch(scriptJumpTarget, (target) => {
  if (!target) return
  if (isDslMode.value) {
    const content = cm.getContent()
    const blocks = parseDsl(content)
    const match = blocks.find(b => b.name === target)
    if (match) cm.jumpToLine(match.line)
  } else {
    cm.jumpToFunction(target)
    sharedActiveFunction.value = target
  }
  store.clearJumpTarget()
})

watch(sharedActiveFunction, (name) => {
  if (name) cm.jumpToFunction(name)
})

watch(sharedActiveDslBlock, (block) => {
  if (block) cm.jumpToLine(block.line)
})

async function handleSave() {
  if (!currentMap.value) return
  const content = cm.getContent()
  if (isDslMode.value) {
    await store.saveSceneFile(currentMap.value.name, content)
    // WYSIWYG: compile the just-saved .scene and hot-inject it into the
    // running game (the wasm binary embeds the scenes it was built with, so
    // without this the playtest would keep showing the old script).
    await injectSavedScene(currentMap.value.name, content)
  } else {
    await store.saveScriptFile(currentMap.value.name, content)
  }
}

const injectStatus = ref('')
let injecting = false

/** Compile a `.scene` to JS and hot-inject it (script + config) into the game. */
async function injectSavedScene(mapName: string, sceneSource: string) {
  if (injecting) return
  injecting = true
  injectStatus.value = 'compiling…'
  try {
    const result = await compileScene(sceneSource)
    if (!result.ok) {
      injectStatus.value = `compile error ${result.line}:${result.col}`
      return
    }
    injectStatus.value = 'injecting…'
    const config = store.scriptConfigs[mapName]
    await injectSceneScript(mapName, result.output, config ? JSON.stringify(config) : null)
    injectStatus.value = '✓ preview updated'
  } catch (e) {
    injectStatus.value = `preview unavailable: ${(e as Error).message}`
  } finally {
    injecting = false
  }
}

/** Save (if dirty), then open the floating playtest pre-targeted at the
 *  current map — never teleports a running Play session. */
async function previewInGame() {
  if (!currentMap.value) return
  if (sceneDirty.value || scriptDirty.value) await handleSave()
  const name = currentMap.value.name
  playtestOverlay.launch({ kind: 'map', map: name })
  injectStatus.value = `▶ test mode → ${name}`
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 's') {
    e.preventDefault()
    if (scriptEditorOpen.value) {
      handleSave()
    }
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
  if (previewTimer) clearTimeout(previewTimer)
})
</script>

<template>
  <div class="script-editor-panel">
    <div class="panel-header">
      <div class="header-left">
        <span class="text-xs font-mono text-text-muted">{{ mapName }}/{{ isDslMode ? 'script.scene' : 'script.js' }}</span>
        <span v-if="isDslMode ? sceneDirty : scriptDirty" class="dirty-badge">Modified</span>
      </div>
      <div class="header-right">
        <button
          v-if="isDslMode"
          class="preview-btn"
          :class="{ active: previewOpen }"
          title="Toggle compiled-JS preview"
          @click="togglePreview"
        >
          {{ previewOpen ? 'Hide JS' : 'Show JS' }}
        </button>
        <button
          class="preview-btn"
          title="Save and open the floating playtest at this map"
          @click="previewInGame"
        >
          ▶ In Game
        </button>
        <button
          class="save-btn"
          :disabled="isDslMode ? !sceneDirty : !scriptDirty"
          :class="{ disabled: isDslMode ? !sceneDirty : !scriptDirty }"
          @click="handleSave"
        >
          Save Script
        </button>
        <span v-if="injectStatus" class="text-[10px] text-text-muted font-mono" :title="injectStatus">{{ injectStatus }}</span>
      </div>
    </div>

    <div class="editor-split" :class="{ 'has-preview': isDslMode && previewOpen }">
      <div class="editor-area" ref="editorContainer"></div>
      <template v-if="isDslMode && previewOpen">
        <div class="split-divider"></div>
        <div class="preview-pane">
          <div class="preview-header">
            <span class="text-xs font-mono text-text-muted">compiled JS (read-only)</span>
          </div>
          <div v-if="previewError" class="preview-error">{{ previewError }}</div>
          <div v-show="!previewError" class="preview-area" ref="previewContainer"></div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.script-editor-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--color-bg-panel);
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 10px;
  background: var(--color-bg-inset);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.dirty-badge {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 3px;
  background: rgba(241, 196, 15, 0.2);
  color: var(--color-warning);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.save-btn {
  padding: 2px 10px;
  font-size: 11px;
  font-weight: bold;
  border: none;
  border-radius: 3px;
  cursor: pointer;
  background: var(--color-accent);
  color: var(--color-bg);
  transition: opacity 0.15s;
}

.save-btn:hover:not(.disabled) {
  opacity: 0.85;
}

.save-btn.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.preview-btn {
  padding: 2px 10px;
  font-size: 11px;
  font-weight: bold;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 3px;
  cursor: pointer;
  background: transparent;
  color: var(--color-text-muted);
  transition: all 0.15s;
}

.preview-btn:hover {
  border-color: var(--color-accent);
  color: var(--color-text);
}

.preview-btn.active {
  background: var(--color-accent);
  border-color: var(--color-accent);
  color: var(--color-bg);
}

.editor-split {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: row;
}

.editor-area {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.editor-split.has-preview .editor-area {
  flex: 1 1 50%;
}

.split-divider {
  width: 1px;
  flex: 0 0 1px;
  background: rgba(255, 255, 255, 0.1);
}

.preview-pane {
  flex: 1 1 50%;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--color-bg-panel);
}

.preview-header {
  padding: 4px 10px;
  background: var(--color-bg-inset);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.preview-error {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 10px;
  font-family: ui-monospace, monospace;
  font-size: 12px;
  white-space: pre-wrap;
  color: var(--color-error, #f87171);
}

.preview-area {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.editor-area :deep(.cm-editor),
.preview-area :deep(.cm-editor) {
  height: 100%;
}

.editor-area :deep(.cm-scroller),
.preview-area :deep(.cm-scroller) {
  overflow: auto;
}
</style>
