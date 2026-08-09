<script setup lang="ts">
// ───────────────────────────────────────────────────────────────────────────
// Image-generation provider editor — SEPARATE from the text providers. kind:
// 'openai' (OpenAI-compatible images API) | 'gemini' (Google generateContent,
// which supports reference images). Keys live in browser localStorage (shared
// keyspace with text providers by id), never on disk. The "Test" action renders
// one tiny image to verify the provider + key.
// ───────────────────────────────────────────────────────────────────────────
import { onMounted, reactive, ref, computed } from 'vue'
import { useAiImageProviders } from '../../composables/useAiImageProviders'
import { getStoredKey, setStoredKey } from '../../composables/useAiStream'
import type { ImageProviderProfile } from '../../types/ai'

const ai = useAiImageProviders()
const { imageProviders } = ai

interface Draft extends ImageProviderProfile { apiKey: string }
function blankDraft(): Draft {
  return { id: '', kind: 'gemini', baseURL: 'https://generativelanguage.googleapis.com', model: 'gemini-2.5-flash-image', proxyUrl: '', apiKey: '' }
}
const draft = reactive<Draft>(blankDraft())
const editingIndex = ref<number | null>(null)
const savedMsg = ref('')

interface TestResult { ok: boolean; msg: string; preview?: string }
const testing = ref(false)
const testResult = ref<TestResult | null>(null)
const rowTest = reactive<{ index: number | null; testing: boolean; result: TestResult | null }>({ index: null, testing: false, result: null })

onMounted(() => ai.loadImageProviders())

function keyStored(id: string): boolean { return !!getStoredKey(id) }
const editingHasKey = computed(() => editingIndex.value !== null && keyStored(draft.id.trim()))

function edit(i: number) {
  editingIndex.value = i
  Object.assign(draft, blankDraft(), JSON.parse(JSON.stringify(imageProviders.value[i])))
  draft.apiKey = ''
  testResult.value = null
}
function reset() {
  editingIndex.value = null
  Object.assign(draft, blankDraft())
  testResult.value = null
}
function toProfile(): ImageProviderProfile {
  const clean: ImageProviderProfile = { id: draft.id.trim(), kind: draft.kind, baseURL: draft.baseURL.trim(), model: draft.model.trim() }
  if (draft.proxyUrl?.trim()) clean.proxyUrl = draft.proxyUrl.trim()
  return clean
}
async function commit() {
  if (!draft.id.trim()) return
  const clean = toProfile()
  const next = imageProviders.value.slice()
  if (editingIndex.value !== null) next[editingIndex.value] = clean
  else {
    const existing = next.findIndex((p) => p.id === clean.id)
    if (existing >= 0) next[existing] = clean
    else next.push(clean)
  }
  await ai.saveImageProviders(next)
  if (draft.apiKey.trim()) setStoredKey(clean.id, draft.apiKey.trim())
  reset()
  flash('Saved')
}
async function removeAt(i: number) {
  const id = imageProviders.value[i].id
  const next = imageProviders.value.slice()
  next.splice(i, 1)
  await ai.saveImageProviders(next)
  localStorage.removeItem('jrpg-ai-key-' + id)
  if (editingIndex.value === i) reset()
}
function clearKey(id: string) {
  localStorage.removeItem('jrpg-ai-key-' + id)
  flash('Key cleared')
}
function flash(m: string) {
  savedMsg.value = m
  setTimeout(() => { if (savedMsg.value === m) savedMsg.value = '' }, 1500)
}

async function callTest(profile: ImageProviderProfile, apiKey: string): Promise<TestResult> {
  try {
    const resp = await fetch('/api/ai/test-image-provider', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ profile, apiKey }),
    })
    const data = await resp.json()
    if (!resp.ok) throw new Error(data.error || resp.statusText)
    return data.ok
      ? { ok: true, msg: '', preview: data.base64 ? `data:image/png;base64,${data.base64}` : undefined }
      : { ok: false, msg: data.error || '' }
  } catch (e) {
    return { ok: false, msg: e instanceof Error ? e.message : String(e) }
  }
}

/** Test the draft using the typed key (or a saved one when blank). */
async function runTest() {
  testResult.value = null
  if (!draft.id.trim() || !draft.model.trim()) {
    testResult.value = { ok: false, msg: 'Set a name and an image model before testing.' }
    return
  }
  const key = draft.apiKey.trim() || getStoredKey(draft.id.trim()) || ''
  if (!key) {
    testResult.value = { ok: false, msg: 'Enter an API key first (type one above, or save one for this profile).' }
    return
  }
  testing.value = true
  testResult.value = await callTest(toProfile(), key)
  testing.value = false
}

/** Test a saved profile from its row, using its stored key. */
async function runRowTest(i: number) {
  const profile = imageProviders.value[i]
  const key = getStoredKey(profile.id) || ''
  rowTest.index = i
  rowTest.result = null
  if (!key) {
    rowTest.result = { ok: false, msg: 'Enter an API key first (type one above, or save one for this profile).' }
    return
  }
  rowTest.testing = true
  rowTest.result = await callTest(profile, key)
  rowTest.testing = false
}

function applyPreset(kind: 'openai' | 'geminiFlash' | 'geminiPro') {
  if (kind === 'openai') Object.assign(draft, { id: draft.id || 'openai-img', kind: 'openai', baseURL: 'https://api.openai.com/v1', model: 'gpt-image-1' })
  if (kind === 'geminiFlash') Object.assign(draft, { id: draft.id || 'gemini-img', kind: 'gemini', baseURL: 'https://generativelanguage.googleapis.com', model: 'gemini-2.5-flash-image' })
  if (kind === 'geminiPro') Object.assign(draft, { id: draft.id || 'gemini-pro', kind: 'gemini', baseURL: 'https://generativelanguage.googleapis.com', model: 'gemini-3-pro-image' })
}
</script>

<template>
  <div class="p-5 max-w-2xl">
    <h2 class="text-base font-bold text-purple-400 mb-1">Image generation providers</h2>
    <p class="text-[11px] text-gray-400 mb-4 leading-snug">Separate from the text providers. Gemini is recommended — it accepts reference images, so a base sprite locks identity across a sheet. Behind a firewall, set GEMINI_PROXY / HTTPS_PROXY when launching the dev server.</p>

    <!-- Existing profiles -->
    <div class="space-y-2 mb-6">
      <div v-for="(p, i) in imageProviders" :key="p.id" class="bg-gray-800 border border-gray-700 rounded px-3 py-2">
        <div class="flex items-center gap-3">
          <div class="flex-1 min-w-0">
            <div class="text-sm text-gray-100 font-medium">{{ p.id }}
              <span class="text-[10px] text-gray-500 ml-1">{{ p.kind }}</span>
            </div>
            <div class="text-[11px] text-gray-500 truncate">{{ p.model }} · {{ p.baseURL }}</div>
          </div>
          <span class="text-[10px] px-1.5 py-0.5 rounded" :class="keyStored(p.id) ? 'bg-green-900/40 text-green-400' : 'bg-gray-700 text-gray-400'">
            {{ keyStored(p.id) ? 'key set' : 'no key' }}
          </span>
          <button @click="runRowTest(i)" :disabled="rowTest.testing" class="text-[11px] text-gray-400 hover:text-green-400 disabled:opacity-40">
            {{ rowTest.index === i && rowTest.testing ? 'Testing…' : 'Test' }}
          </button>
          <button v-if="keyStored(p.id)" @click="clearKey(p.id)" class="text-[11px] text-gray-400 hover:text-amber-400">clear key</button>
          <button @click="edit(i)" class="text-[11px] text-gray-400 hover:text-purple-400">edit</button>
          <button @click="removeAt(i)" class="text-[11px] text-gray-400 hover:text-red-400">delete</button>
        </div>
        <div v-if="rowTest.index === i && rowTest.result" class="mt-2 flex items-center gap-2 text-[11px] rounded px-2 py-1"
          :class="rowTest.result.ok ? 'bg-green-900/20 text-green-300' : 'bg-red-900/20 text-red-300'">
          <img v-if="rowTest.result.preview" :src="rowTest.result.preview" class="w-8 h-8 rounded border border-gray-700" style="image-rendering: pixelated;" alt="" />
          <span class="font-semibold">{{ rowTest.result.ok ? 'Success' : 'Failed' }}</span>
          <span v-if="rowTest.result.msg" class="break-words">· {{ rowTest.result.msg }}</span>
        </div>
      </div>
      <p v-if="!imageProviders.length" class="text-xs text-gray-600">No image providers yet.</p>
    </div>

    <!-- Editor -->
    <div class="bg-gray-800 border border-gray-700 rounded p-4 space-y-3">
      <div class="flex items-center justify-between">
        <h3 class="text-xs font-semibold uppercase tracking-wide text-gray-400">
          {{ editingIndex !== null ? 'Edit image provider' : 'New image provider' }}
        </h3>
        <div class="flex gap-1">
          <button @click="applyPreset('geminiFlash')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">Gemini Flash</button>
          <button @click="applyPreset('geminiPro')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">Gemini Pro</button>
          <button @click="applyPreset('openai')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">OpenAI</button>
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <label class="text-[11px] text-gray-400">Name
          <input v-model="draft.id" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <label class="text-[11px] text-gray-400">Protocol
          <select v-model="draft.kind" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100">
            <option value="gemini">Gemini (generateContent)</option>
            <option value="openai">OpenAI-compatible images</option>
          </select>
        </label>
        <label class="text-[11px] text-gray-400 col-span-2">Base URL
          <input v-model="draft.baseURL" placeholder="https://generativelanguage.googleapis.com" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <label class="text-[11px] text-gray-400 col-span-2">Proxy (optional)
          <input v-model="draft.proxyUrl" placeholder="http://127.0.0.1:9085" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
          <span class="text-[10px] text-gray-500 block mt-0.5">Route requests through an HTTP/HTTPS proxy — needed if your network blocks the provider (e.g. Gemini). Leave blank to use the GEMINI_PROXY / HTTPS_PROXY env var, or no proxy.</span>
        </label>
        <label class="text-[11px] text-gray-400">Image model
          <input v-model="draft.model" placeholder="gemini-2.5-flash-image" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <label class="text-[11px] text-gray-400">API key
          <input v-model="draft.apiKey" type="password" autocomplete="off" placeholder="stored in browser, sent per-request" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
          <span v-if="editingHasKey" class="text-[10px] text-gray-500">leave blank to keep the saved key</span>
        </label>
      </div>

      <!-- Test generation -->
      <div class="border-t border-gray-700/70 pt-3 space-y-2">
        <div class="flex items-center gap-2">
          <button @click="runTest" :disabled="testing || !draft.id.trim()" class="px-3 py-1 text-xs rounded bg-green-700 text-white hover:bg-green-600 disabled:opacity-40">
            {{ testing ? 'Testing…' : 'Test generation' }}
          </button>
          <span class="text-[10px] text-gray-500">Renders one tiny image to verify the provider + key.</span>
        </div>
        <div v-if="testResult" class="flex items-center gap-2 text-[11px] rounded px-2 py-1.5"
          :class="testResult.ok ? 'bg-green-900/20 text-green-300' : 'bg-red-900/20 text-red-300'">
          <img v-if="testResult.preview" :src="testResult.preview" class="w-10 h-10 rounded border border-gray-700" style="image-rendering: pixelated;" alt="" />
          <span class="font-semibold">{{ testResult.ok ? 'Success' : 'Failed' }}</span>
          <span v-if="testResult.msg" class="break-words">· {{ testResult.msg }}</span>
        </div>
      </div>

      <div class="flex items-center gap-2 border-t border-gray-700/70 pt-3">
        <button @click="commit" :disabled="!draft.id.trim()" class="px-3 py-1 text-xs rounded bg-purple-600 text-white hover:bg-purple-500 disabled:opacity-40">
          {{ editingIndex !== null ? 'Update' : 'Add' }}
        </button>
        <button v-if="editingIndex !== null" @click="reset" class="px-3 py-1 text-xs rounded text-gray-400 hover:text-gray-200">cancel</button>
        <span v-if="savedMsg" class="text-[11px] text-green-400">{{ savedMsg }}</span>
      </div>
    </div>
  </div>
</template>
