<script setup lang="ts">
// ───────────────────────────────────────────────────────────────────────────
// AI sprite generation dialog for the Pixel activity. Sends the prompt to
// POST /api/sprites/generate-single (provider image → matte → pixelize →
// resample server-side), previews the result, and loads it into the current
// canvas on confirm — saving stays on the usual Ctrl+S / PUT /gfx/** path.
// ───────────────────────────────────────────────────────────────────────────
import { ref, computed, watch } from 'vue'
import { usePixelStore } from '../stores/pixelStore'
import { useAiImageProviders } from '../composables/useAiImageProviders'
import { getStoredKey } from '../composables/useAiStream'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: [] }>()

const store = usePixelStore()
const { imageProviders, loadImageProviders } = useAiImageProviders()

const providerId = ref('')
const prompt = ref('')
const generating = ref(false)
const errorMsg = ref('')
/** base64 PNG of the last successful generation (preview + load source). */
const resultBase64 = ref('')

const selectedProvider = computed(() => imageProviders.value.find((p) => p.id === providerId.value))
const hasKey = computed(() => !!(providerId.value && getStoredKey(providerId.value)))
const canGenerate = computed(
  () => !!selectedProvider.value?.model && hasKey.value && !!prompt.value.trim() && !generating.value,
)
const previewUrl = computed(() => (resultBase64.value ? `data:image/png;base64,${resultBase64.value}` : ''))
const targetLabel = computed(() => {
  const a = store.activeAsset
  return a ? `${a.displayName} — ${store.canvasWidth}×${store.canvasHeight}px` : ''
})

function defaultPrompt(): string {
  const a = store.activeAsset
  if (!a) return ''
  switch (a.category) {
    case 'pokemon-front':
      return `A Pokémon-style creature named ${a.displayName}, front view, full body.`
    case 'pokemon-back':
      return `A Pokémon-style creature named ${a.displayName}, seen from behind, full body.`
    case 'trainer':
      return `A Pokémon trainer character named ${a.displayName}, front-facing, full body.`
    case 'npc':
      return `A small overworld NPC character sprite named ${a.displayName}, front-facing, full body.`
    default:
      return `A Game Boy era video-game sprite of ${a.displayName}.`
  }
}

watch(() => props.open, async (v) => {
  if (!v) return
  prompt.value = defaultPrompt()
  resultBase64.value = ''
  errorMsg.value = ''
  generating.value = false
  await loadImageProviders()
  if (!providerId.value && imageProviders.value.length) providerId.value = imageProviders.value[0].id
})

async function generate() {
  const profile = selectedProvider.value
  const apiKey = providerId.value ? getStoredKey(providerId.value) : null
  if (!profile || !apiKey || !prompt.value.trim()) return
  generating.value = true
  errorMsg.value = ''
  try {
    const resp = await fetch('/api/sprites/generate-single', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        profile, apiKey,
        prompt: prompt.value.trim(),
        width: store.canvasWidth,
        height: store.canvasHeight,
      }),
    })
    const json = await resp.json().catch(() => ({}))
    if (!resp.ok) throw new Error(json.error || `HTTP ${resp.status}`)
    resultBase64.value = json.base64
  } catch (e) {
    errorMsg.value = (e as Error).message
  } finally {
    generating.value = false
  }
}

async function loadIntoCanvas() {
  if (!resultBase64.value) return
  try {
    const blob = await (await fetch(`data:image/png;base64,${resultBase64.value}`)).blob()
    const img = await createImageBitmap(blob)
    const canvas = new OffscreenCanvas(img.width, img.height)
    const ctx = canvas.getContext('2d')!
    ctx.drawImage(img, 0, 0)
    store.loadGeneratedImage(ctx.getImageData(0, 0, img.width, img.height))
    emit('close')
  } catch (e) {
    errorMsg.value = `Failed to load image: ${(e as Error).message}`
  }
}

function cancel() {
  emit('close')
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 bg-black/60 z-[100] flex items-center justify-center"
      @click.self="cancel"
      @keydown.stop
    >
      <div class="bg-bg-panel border border-accent rounded-lg p-5 w-[440px] max-h-[90vh] overflow-y-auto">
        <h2 class="text-accent text-base font-bold mb-1">✨ AI Generate Sprite</h2>
        <p class="text-[10px] text-text-muted mb-3">{{ targetLabel }}</p>

        <!-- No image provider configured -->
        <div v-if="!imageProviders.length" class="p-2 rounded bg-bg-inset text-[11px] text-amber-400/90 leading-snug">
          No image provider configured. Open the <b>Assistant panel ⚙ Settings → Image Providers</b> tab to add one (Gemini or OpenAI-compatible), then retry.
        </div>

        <template v-else>
          <label class="block text-xs mb-1">Image Provider</label>
          <select
            v-model="providerId"
            class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs mb-1"
          >
            <option v-for="p in imageProviders" :key="p.id" :value="p.id">{{ p.id }} ({{ p.kind }}: {{ p.model }})</option>
          </select>
          <p v-if="providerId && !hasKey" class="text-[10px] text-amber-400/90 mb-2">
            No API key stored for this provider — set it in Assistant ⚙ Settings → Image Providers.
          </p>
          <div v-else class="mb-2" />

          <label class="block text-xs mb-1">Prompt</label>
          <textarea
            v-model="prompt"
            rows="4"
            placeholder="Describe the sprite…"
            class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs mb-2 resize-y"
            @keydown.esc.stop="cancel"
          />

          <!-- Preview -->
          <div class="flex items-start gap-3 mb-2">
            <div
              class="w-28 h-28 shrink-0 rounded border border-[rgba(255,255,255,0.1)] bg-bg-inset flex items-center justify-center overflow-hidden"
            >
              <img
                v-if="previewUrl"
                :src="previewUrl"
                alt="Generated sprite preview"
                class="max-w-full max-h-full"
                style="image-rendering: pixelated"
              />
              <span v-else-if="generating" class="text-accent text-[11px] animate-pulse px-2 text-center">Generating…</span>
              <span v-else class="text-text-muted text-[10px] px-2 text-center">Preview</span>
            </div>
            <p class="text-[10px] text-text-muted leading-snug">
              The result is background-keyed, pixel-snapped and scaled to the current canvas.
              Loading it replaces the canvas (undoable) — polish it with the pixel tools,
              then save as usual (Ctrl+S).
            </p>
          </div>
        </template>

        <div v-if="errorMsg" class="mt-2 text-danger text-[11px] break-words">{{ errorMsg }}</div>

        <div class="flex justify-end gap-2 mt-4">
          <button
            class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444]"
            @click="cancel"
          >Close</button>
          <button
            v-if="imageProviders.length"
            class="px-3 py-1.5 bg-[#555] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#666] disabled:opacity-50 disabled:cursor-not-allowed"
            :disabled="!resultBase64"
            @click="loadIntoCanvas"
          >Load into Canvas</button>
          <button
            v-if="imageProviders.length"
            class="px-3 py-1.5 bg-[#27ae60] text-white border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#229954] disabled:opacity-50 disabled:cursor-not-allowed"
            :disabled="!canGenerate"
            @click="generate"
          >{{ generating ? 'Generating…' : resultBase64 ? 'Regenerate' : 'Generate' }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
