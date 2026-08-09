<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useMapStore } from '../stores/mapStore'
import {
  TILESET_FILES,
  tilesetCategory,
  TILESET_CATEGORY_LABEL,
  type TilesetCategory,
} from '../types/constants'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ close: []; created: [name: string] }>()

const store = useMapStore()

const name = ref('')
const displayName = ref('')
const base = ref('Overworld')
const category = ref<TilesetCategory>('outdoor')
const submitting = ref(false)
const errorMsg = ref('')

const BUILTIN_TILESETS = computed(() => Object.keys(TILESET_FILES).sort())
const GROUPED = computed<{ category: TilesetCategory; names: string[] }[]>(() => {
  const groups: Record<TilesetCategory, string[]> = { outdoor: [], indoor: [], cave: [] }
  for (const t of BUILTIN_TILESETS.value) groups[tilesetCategory(t)].push(t)
  return (['outdoor', 'cave', 'indoor'] as TilesetCategory[]).map((c) => ({
    category: c,
    names: groups[c],
  }))
})

watch(() => props.open, (v) => {
  if (v) {
    name.value = ''
    displayName.value = ''
    base.value = 'Overworld'
    category.value = 'outdoor'
    submitting.value = false
    errorMsg.value = ''
  }
})

// When base changes, default the new tileset's category to the base's category
// (the user can override).
watch(base, (b) => {
  category.value = tilesetCategory(b)
})

const nameValid = computed(() => /^[A-Za-z][A-Za-z0-9_]*$/.test(name.value.trim()))

async function submit() {
  if (!nameValid.value) {
    errorMsg.value = 'Name must start with a letter and contain only letters, digits and underscores.'
    return
  }
  errorMsg.value = ''
  submitting.value = true
  const result = await store.createTileset({
    name: name.value.trim(),
    base: base.value,
    category: category.value,
    displayName: displayName.value.trim() || undefined,
  })
  submitting.value = false
  if (!result.ok) {
    errorMsg.value = result.error ?? 'Unknown error'
    return
  }
  emit('created', name.value.trim())
  emit('close')
}
</script>

<template>
  <div
    v-if="open"
    class="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
    @click.self="emit('close')"
  >
    <div class="bg-bg-panel border border-accent rounded-lg p-4 w-[460px] max-w-[95%]">
      <h2 class="text-accent font-bold text-base mb-3">+ New Tileset</h2>

      <p class="text-[11px] text-text-muted mb-3 leading-snug">
        Creates a new tileset by cloning a base tileset's
        <code class="text-accent">.bst</code> blockset and
        <code class="text-accent">.png</code> tile graphics. Edit the new
        tileset's blocks via the Block Palette (double-click a block).
        <br />
        <b class="text-accent">Runtime:</b> custom tilesets are picked up by
        the Rust game runtime at compile time via
        <code class="text-accent">tileset_extras.json</code>. The new
        tileset's <em>blocks</em> and <em>tile collisions</em> apply at
        runtime; metadata that lives outside the blockset (palettes, counter
        tiles, grass tile, animation, door/warp/spinner behaviour) is
        inherited from the chosen <em>base</em> tileset.
      </p>

      <div class="space-y-2.5 text-xs">
        <div>
          <label class="block mb-1 font-bold">Name (PascalCase identifier)</label>
          <input
            v-model="name"
            type="text"
            placeholder="MyTileset"
            class="w-full p-1.5 rounded border border-accent bg-bg text-text"
            :class="!nameValid && name.length ? 'border-warning' : ''"
          />
        </div>

        <div>
          <label class="block mb-1 font-bold">Display Name (optional)</label>
          <input
            v-model="displayName"
            type="text"
            placeholder="My Tileset"
            class="w-full p-1.5 rounded border border-accent bg-bg text-text"
          />
        </div>

        <div>
          <label class="block mb-1 font-bold">
            Base Tileset (clone from) — currently:
            <span class="text-accent">{{ TILESET_CATEGORY_LABEL[tilesetCategory(base)] }}</span>
          </label>
          <select
            v-model="base"
            class="w-full p-1.5 rounded border border-accent bg-bg text-text"
          >
            <optgroup
              v-for="grp in GROUPED"
              :key="grp.category"
              :label="`── ${TILESET_CATEGORY_LABEL[grp.category]} ──`"
            >
              <option v-for="t in grp.names" :key="t" :value="t">{{ t }}</option>
            </optgroup>
          </select>
        </div>

        <div>
          <label class="block mb-1 font-bold">Category for this new tileset</label>
          <select
            v-model="category"
            class="w-full p-1.5 rounded border border-accent bg-bg text-text"
          >
            <option value="outdoor">{{ TILESET_CATEGORY_LABEL.outdoor }}</option>
            <option value="cave">{{ TILESET_CATEGORY_LABEL.cave }}</option>
            <option value="indoor">{{ TILESET_CATEGORY_LABEL.indoor }}</option>
          </select>
        </div>
      </div>

      <div v-if="errorMsg" class="mt-2 text-warning text-[11px]">{{ errorMsg }}</div>

      <div class="mt-4 flex justify-end gap-2">
        <button
          class="px-3 py-1.5 text-xs bg-bg-inset rounded hover:bg-[#444] text-text"
          :disabled="submitting"
          @click="emit('close')"
        >Cancel</button>
        <button
          class="px-3 py-1.5 text-xs bg-accent text-bg-panel rounded font-bold hover:opacity-85"
          :disabled="submitting || !nameValid"
          :class="(submitting || !nameValid) ? 'opacity-50 cursor-not-allowed' : ''"
          @click="submit"
        >{{ submitting ? 'Creating...' : 'Create' }}</button>
      </div>
    </div>
  </div>
</template>
