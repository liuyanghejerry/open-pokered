<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useMapStore } from '../stores/mapStore'
import {
  TILESET_FILES,
  MUSIC_LIST,
  tilesetCategory,
  TILESET_CATEGORY_LABEL,
  type TilesetCategory,
} from '../types/constants'

const props = defineProps<{
  open: boolean
  initialX?: number
  initialY?: number
}>()
const emit = defineEmits<{
  close: []
  created: [name: string]
}>()

const store = useMapStore()

const name = ref('')
const displayName = ref('')
const tileset = ref('Overworld')
const width = ref(10)
const height = ref(9)
const music = ref('PalletTown')
const borderBlock = ref(0)
const placeOnWorldMap = ref(true)
const townX = ref(0)
const townY = ref(0)
const submitting = ref(false)
const errorMsg = ref('')

// Built-in + user-created tilesets. The custom ones live in
// `tilesetExtras` (POST /api/tilesets) and carry their declared category.
const TILESET_NAMES = computed(() => {
  const builtin = Object.keys(TILESET_FILES)
  const custom = Object.keys(store.tilesetExtras)
  return Array.from(new Set([...builtin, ...custom])).sort()
})

function categoryOf(name: string): TilesetCategory {
  const extra = store.tilesetExtras[name]
  if (extra) return extra.category
  return tilesetCategory(name)
}

// Group tilesets by inferred category (室外 / 室内 / 洞穴) so the user can
// pick the type of map (indoor / outdoor / cave) consciously. The category
// is purely derived from the tileset — that's what the original game does.
const GROUPED_TILESETS = computed<{ category: TilesetCategory; names: string[] }[]>(() => {
  const groups: Record<TilesetCategory, string[]> = { outdoor: [], indoor: [], cave: [] }
  for (const t of TILESET_NAMES.value) {
    groups[categoryOf(t)].push(t)
  }
  return (['outdoor', 'cave', 'indoor'] as TilesetCategory[]).map((c) => ({
    category: c,
    names: groups[c],
  }))
})

const selectedCategory = computed<TilesetCategory>(() => categoryOf(tileset.value))
const selectedCategoryLabel = computed(() => TILESET_CATEGORY_LABEL[selectedCategory.value])

watch(() => props.open, (v) => {
  if (v) {
    name.value = ''
    displayName.value = ''
    tileset.value = 'Overworld'
    width.value = 10
    height.value = 9
    music.value = 'PalletTown'
    borderBlock.value = 0
    placeOnWorldMap.value = props.initialX != null && props.initialY != null
    townX.value = props.initialX ?? 0
    townY.value = props.initialY ?? 0
    submitting.value = false
    errorMsg.value = ''
  }
})

const nameValid = computed(() => /^[A-Za-z][A-Za-z0-9_]*$/.test(name.value.trim()))

async function submit() {
  if (!nameValid.value) {
    errorMsg.value = 'Name must start with a letter and contain only letters, digits and underscores.'
    return
  }
  errorMsg.value = ''
  submitting.value = true
  const result = await store.createMap({
    name: name.value.trim(),
    displayName: displayName.value.trim() || undefined,
    tileset: tileset.value,
    width: width.value,
    height: height.value,
    music: music.value,
    borderBlock: borderBlock.value,
    townMap: placeOnWorldMap.value
      ? { x: townX.value, y: townY.value }
      : undefined,
  })
  submitting.value = false
  if (result.ok) {
    emit('created', name.value.trim())
    emit('close')
  } else {
    errorMsg.value = result.error ?? 'Failed to create map'
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
    >
      <div class="bg-bg-panel border border-accent rounded-lg p-5 w-[420px] max-h-[90vh] overflow-y-auto">
        <h2 class="text-accent text-base font-bold mb-3">Create New Map</h2>

        <label class="block text-xs mb-1">Map Name (identifier)</label>
        <input
          v-model="name"
          type="text"
          placeholder="e.g. NewTown"
          class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs mb-2 font-mono"
        />

        <label class="block text-xs mb-1">Display Name (optional)</label>
        <input
          v-model="displayName"
          type="text"
          placeholder="e.g. NEW TOWN"
          class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs mb-2"
        />

        <div class="grid grid-cols-2 gap-2">
          <div>
            <label class="block text-xs mb-1">
              Tileset
              <span class="text-text-muted font-normal">({{ selectedCategoryLabel }})</span>
            </label>
            <select
              v-model="tileset"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
            >
              <optgroup
                v-for="group in GROUPED_TILESETS"
                :key="group.category"
                :label="TILESET_CATEGORY_LABEL[group.category]"
              >
                <option v-for="t in group.names" :key="t" :value="t">{{ t }}</option>
              </optgroup>
            </select>
            <p class="text-[10px] text-text-muted mt-0.5 leading-tight">
              室内/室外/洞穴 由 tileset 决定（原版游戏一致行为）
            </p>
          </div>
          <div>
            <label class="block text-xs mb-1">Music</label>
            <select
              v-model="music"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
            >
              <option v-for="m in MUSIC_LIST" :key="m" :value="m">{{ m }}</option>
            </select>
          </div>
          <div>
            <label class="block text-xs mb-1">Width (blocks)</label>
            <input
              v-model.number="width"
              type="number"
              min="1"
              max="255"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
            />
          </div>
          <div>
            <label class="block text-xs mb-1">Height (blocks)</label>
            <input
              v-model.number="height"
              type="number"
              min="1"
              max="255"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
            />
          </div>
          <div class="col-span-2">
            <label class="block text-xs mb-1">Border Block ID (initial fill)</label>
            <input
              v-model.number="borderBlock"
              type="number"
              min="0"
              max="255"
              class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
            />
          </div>
        </div>

        <div class="mt-3 p-2 rounded bg-bg-inset">
          <label class="flex items-center gap-1.5 cursor-pointer text-xs mb-2">
            <input v-model="placeOnWorldMap" type="checkbox" class="w-auto" />
            <b>Place on World Map</b>
          </label>
          <div v-if="placeOnWorldMap" class="grid grid-cols-2 gap-2">
            <div>
              <label class="block text-xs mb-1">X (0-15)</label>
              <input
                v-model.number="townX"
                type="number"
                min="0"
                max="15"
                class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              />
            </div>
            <div>
              <label class="block text-xs mb-1">Y (0-15)</label>
              <input
                v-model.number="townY"
                type="number"
                min="0"
                max="15"
                class="w-full p-1.5 rounded border border-accent bg-bg text-text text-xs"
              />
            </div>
          </div>
        </div>

        <div v-if="errorMsg" class="mt-2 text-danger text-[11px]">{{ errorMsg }}</div>

        <div class="flex justify-end gap-2 mt-4">
          <button
            class="px-3 py-1.5 bg-[#333] text-text border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#444]"
            @click="cancel"
          >
            Cancel
          </button>
          <button
            class="px-3 py-1.5 bg-[#27ae60] text-white border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#229954] disabled:opacity-50 disabled:cursor-not-allowed"
            :disabled="!nameValid || submitting"
            @click="submit"
          >
            {{ submitting ? 'Creating...' : 'Create' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
