<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useAiProviders } from '../../composables/useAiProviders'
import { getStoredKey, setStoredKey } from '../../composables/useAiStream'
import { buildBrowserModel } from '../../composables/assistantStatic'
import { staticMode } from '../../composables/useStaticMode'
import type { ProviderProfile } from '../../types/ai'

const ai = useAiProviders()
const { providers } = ai

interface Draft extends ProviderProfile {
  apiKey: string
}

function blankDraft(): Draft {
  return { id: '', kind: 'openai', baseURL: '', model: '', embeddingModel: '', proxyUrl: '', apiKey: '' }
}

const draft = reactive<Draft>(blankDraft())
const editingIndex = ref<number | null>(null)
const savedMsg = ref('')

const testPrompt = ref('Reply with a single word: OK.')
const testing = ref(false)
const testResult = ref<{ ok: boolean; msg: string } | null>(null)
const rowTest = reactive<{ index: number | null; testing: boolean; result: { ok: boolean; msg: string } | null }>({
  index: null, testing: false, result: null,
})

function keyStored(id: string): boolean {
  return !!getStoredKey(id)
}

/** Whether the profile being edited already has a key saved under its id. */
const editingHasKey = computed(() => editingIndex.value !== null && keyStored(draft.id.trim()))

function edit(i: number) {
  editingIndex.value = i
  Object.assign(draft, blankDraft(), JSON.parse(JSON.stringify(providers.value[i])))
  draft.apiKey = ''
  testResult.value = null
}

function reset() {
  editingIndex.value = null
  Object.assign(draft, blankDraft())
  testResult.value = null
}

function toProfile(): ProviderProfile {
  const clean: ProviderProfile = { id: draft.id.trim(), kind: draft.kind, baseURL: draft.baseURL.trim(), model: draft.model.trim() }
  if (draft.embeddingModel?.trim()) clean.embeddingModel = draft.embeddingModel.trim()
  if (draft.proxyUrl?.trim()) clean.proxyUrl = draft.proxyUrl.trim()
  return clean
}

async function commit() {
  if (!draft.id.trim()) return
  const clean = toProfile()
  const next = providers.value.slice()
  if (editingIndex.value !== null) next[editingIndex.value] = clean
  else {
    const existing = next.findIndex(p => p.id === clean.id)
    if (existing >= 0) next[existing] = clean
    else next.push(clean)
  }
  await ai.saveProviders(next)
  // Persist a freshly-typed key under the (possibly new) id.
  if (draft.apiKey.trim()) setStoredKey(clean.id, draft.apiKey.trim())
  reset()
  flash('Saved')
}

async function removeAt(i: number) {
  const id = providers.value[i].id
  const next = providers.value.slice()
  next.splice(i, 1)
  await ai.saveProviders(next)
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

async function callTest(profile: ProviderProfile, apiKey: string): Promise<{ ok: boolean; msg: string }> {
  try {
    // Static hosting has no /api/ai/test-provider — test browser-direct.
    if (staticMode.value) {
      const { generateText } = await import('ai')
      const model = await buildBrowserModel(profile, apiKey)
      const { text } = await generateText({
        model,
        prompt: testPrompt.value.trim() || 'Reply with a single word: OK.',
      })
      return { ok: true, msg: (text || '').trim() }
    }
    const resp = await fetch('/api/ai/test-provider', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ profile, apiKey, prompt: testPrompt.value }),
    })
    const data = await resp.json()
    if (!resp.ok) throw new Error(data.error || resp.statusText)
    return data.ok ? { ok: true, msg: data.text || '' } : { ok: false, msg: data.error || '' }
  } catch (e) {
    return { ok: false, msg: e instanceof Error ? e.message : String(e) }
  }
}

/** Test the draft in the editor, using the typed key (or a saved one when blank). */
async function runTest() {
  testResult.value = null
  if (!draft.id.trim() || !draft.model.trim()) {
    testResult.value = { ok: false, msg: 'Set a name and a text model before testing.' }
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
  const profile = providers.value[i]
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

function applyPreset(kind: 'anthropic' | 'openai' | 'deepseek' | 'ollama') {
  if (kind === 'anthropic') Object.assign(draft, { id: draft.id || 'claude', kind: 'anthropic', baseURL: 'https://api.anthropic.com', model: 'claude-opus-4-8' })
  if (kind === 'openai') Object.assign(draft, { id: draft.id || 'openai', kind: 'openai', baseURL: 'https://api.openai.com/v1', model: 'gpt-4o' })
  if (kind === 'deepseek') Object.assign(draft, { id: draft.id || 'deepseek', kind: 'openai', baseURL: 'https://api.deepseek.com', model: 'deepseek-chat' })
  if (kind === 'ollama') Object.assign(draft, { id: draft.id || 'local', kind: 'openai', baseURL: 'http://localhost:11434/v1', model: 'qwen2.5-coder' })
}
</script>

<template>
  <div class="p-5 max-w-2xl">
    <h2 class="text-base font-bold text-blue-400 mb-1">AI Providers</h2>
    <p class="text-[11px] text-gray-400 mb-4 leading-snug">Vendor-agnostic — any Anthropic-shape or OpenAI-shape endpoint works via a custom base URL. API keys are stored only in your browser (localStorage), never written to the project.</p>

    <!-- Existing profiles -->
    <div class="space-y-2 mb-6">
      <div
        v-for="(p, i) in providers"
        :key="p.id"
        class="bg-gray-800 border border-gray-700 rounded px-3 py-2"
      >
        <div class="flex items-center gap-3">
          <div class="flex-1 min-w-0">
            <div class="text-sm text-gray-100 font-medium">{{ p.id }}
              <span class="text-[10px] text-gray-500 ml-1">{{ p.kind }}</span>
            </div>
            <div class="text-[11px] text-gray-500 truncate">{{ p.model }} · {{ p.baseURL }}</div>
          </div>
          <span
            class="text-[10px] px-1.5 py-0.5 rounded"
            :class="keyStored(p.id) ? 'bg-green-900/40 text-green-400' : 'bg-gray-700 text-gray-400'"
          >{{ keyStored(p.id) ? 'key ✓' : 'no key' }}</span>
          <button @click="runRowTest(i)" :disabled="rowTest.testing" class="text-[11px] text-gray-400 hover:text-green-400 disabled:opacity-40">
            {{ rowTest.index === i && rowTest.testing ? 'Testing…' : 'Test' }}
          </button>
          <button v-if="keyStored(p.id)" @click="clearKey(p.id)" class="text-[11px] text-gray-400 hover:text-amber-400">clear key</button>
          <button @click="edit(i)" class="text-[11px] text-gray-400 hover:text-blue-400">edit</button>
          <button @click="removeAt(i)" class="text-[11px] text-gray-400 hover:text-red-400">delete</button>
        </div>
        <!-- Row test result -->
        <div
          v-if="rowTest.index === i && rowTest.result"
          class="mt-2 text-[11px] rounded px-2 py-1 whitespace-pre-wrap break-words"
          :class="rowTest.result.ok ? 'bg-green-900/20 text-green-300' : 'bg-red-900/20 text-red-300'"
        >
          <span class="font-semibold">{{ rowTest.result.ok ? 'Success' : 'Failed' }}</span>
          <span v-if="rowTest.result.msg"> · {{ rowTest.result.msg }}</span>
        </div>
      </div>
      <p v-if="!providers.length" class="text-xs text-gray-600">No providers yet. Add one below.</p>
    </div>

    <!-- Editor -->
    <div class="bg-gray-800 border border-gray-700 rounded p-4 space-y-3">
      <div class="flex items-center justify-between">
        <h3 class="text-xs font-semibold uppercase tracking-wide text-gray-400">
          {{ editingIndex !== null ? 'Edit profile' : 'New profile' }}
        </h3>
        <div class="flex gap-1">
          <button @click="applyPreset('anthropic')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">Claude</button>
          <button @click="applyPreset('openai')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">OpenAI</button>
          <button @click="applyPreset('deepseek')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">DeepSeek</button>
          <button @click="applyPreset('ollama')" class="text-[10px] px-1.5 py-0.5 rounded bg-gray-700 hover:bg-gray-600">Ollama</button>
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <label class="text-[11px] text-gray-400">Name / id
          <input v-model="draft.id" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <label class="text-[11px] text-gray-400">Protocol
          <select v-model="draft.kind" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100">
            <option value="anthropic">anthropic (Messages)</option>
            <option value="openai">openai (Chat Completions)</option>
          </select>
        </label>
        <label class="text-[11px] text-gray-400 col-span-2">Base URL
          <input v-model="draft.baseURL" placeholder="https://api.deepseek.com" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <label class="text-[11px] text-gray-400 col-span-2">Proxy (optional)
          <input v-model="draft.proxyUrl" placeholder="http://127.0.0.1:9085" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
          <span class="text-[10px] text-gray-500 block mt-0.5">Route requests through an HTTP/HTTPS proxy — needed if your network blocks the provider. Leave blank to use the HTTPS_PROXY env var, or no proxy.</span>
        </label>
        <label class="text-[11px] text-gray-400 col-span-2">Model (text)
          <input v-model="draft.model" placeholder="deepseek-chat" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <label v-if="draft.kind === 'openai'" class="text-[11px] text-gray-400 col-span-2">Embedding model (optional)
          <input v-model="draft.embeddingModel" placeholder="text-embedding-3-small" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
          <span class="text-[10px] text-gray-500 block mt-0.5">Enables retrieval/RAG — the chat pulls the most relevant project records into context.</span>
        </label>
        <label class="text-[11px] text-gray-400 col-span-2">API key
          <input
            v-model="draft.apiKey"
            type="password"
            autocomplete="off"
            placeholder="sk-...  (stored only in your browser)"
            class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100"
          />
          <span v-if="editingHasKey" class="text-[10px] text-gray-500">A key is already saved — leave blank to keep it.</span>
        </label>
      </div>

      <!-- Test connection -->
      <div class="border-t border-gray-700/70 pt-3 space-y-2">
        <label class="text-[11px] text-gray-400 block">Test prompt
          <input v-model="testPrompt" class="mt-1 w-full bg-gray-900 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100" />
        </label>
        <div class="flex items-center gap-2">
          <button
            @click="runTest"
            :disabled="testing || !draft.id.trim()"
            class="px-3 py-1 text-xs rounded bg-green-700 text-white hover:bg-green-600 disabled:opacity-40"
          >
            {{ testing ? 'Testing…' : 'Run test' }}
          </button>
        </div>
        <div
          v-if="testResult"
          class="text-[11px] rounded px-2 py-1.5 whitespace-pre-wrap break-words"
          :class="testResult.ok ? 'bg-green-900/20 text-green-300' : 'bg-red-900/20 text-red-300'"
        >
          <span class="font-semibold">{{ testResult.ok ? 'Success' : 'Failed' }}</span>
          <span v-if="testResult.msg"> · {{ testResult.msg }}</span>
        </div>
      </div>

      <div class="flex items-center gap-2 border-t border-gray-700/70 pt-3">
        <button @click="commit" :disabled="!draft.id.trim()" class="px-3 py-1 text-xs rounded bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-40">
          {{ editingIndex !== null ? 'Update' : 'Add' }}
        </button>
        <button v-if="editingIndex !== null" @click="reset" class="px-3 py-1 text-xs rounded text-gray-400 hover:text-gray-200">Cancel</button>
        <span v-if="savedMsg" class="text-[11px] text-green-400">{{ savedMsg }}</span>
      </div>
    </div>
  </div>
</template>
