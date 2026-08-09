<script setup lang="ts">
import { computed, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useItemStore } from '../stores/itemStore'
import { EFFECT_REGISTRY } from '../types/item'

const store = useItemStore()
const { currentItem, activeItemId, dirty, error, loading, categories } = storeToRefs(store)

const heldOpen = ref(false)
const berryOpen = ref(false)

const heldTriggers = computed(() =>
  EFFECT_REGISTRY.held_triggers.filter(t => t !== 'None')
)

const isBerry = computed(() => currentItem.value?.category === 'Berries')

const sellPrice = computed(() => {
  if (!currentItem.value) return 0
  return Math.floor(currentItem.value.price / 2)
})

const selectedActiveEffect = computed(() => {
  if (!currentItem.value?.effect) return null
  return EFFECT_REGISTRY.active_effects.find(
    e => e.type === currentItem.value!.effect!.type
  ) ?? null
})

const selectedHeldEffect = computed(() => {
  if (!currentItem.value?.held_effect) return null
  return EFFECT_REGISTRY.held_effects.find(
    e => e.type === currentItem.value!.held_effect!.type
  ) ?? null
})

function setPrice(raw: string) {
  const v = parseInt(raw, 10)
  if (Number.isFinite(v)) {
    store.updateField('price', Math.max(0, Math.min(999999, v)))
  }
}

function setTags(raw: string) {
  const tags = raw.split(',').map(t => t.trim()).filter(Boolean)
  store.updateField('tags', tags)
}

function tagsString(): string {
  if (!currentItem.value) return ''
  return currentItem.value.tags.join(', ')
}

const FLAVOR_FIELDS = [
  { key: 'flavor_spicy' as const, label: 'Spicy', emoji: '🌶' },
  { key: 'flavor_dry' as const, label: 'Dry', emoji: '💨' },
  { key: 'flavor_sweet' as const, label: 'Sweet', emoji: '🍬' },
  { key: 'flavor_bitter' as const, label: 'Bitter', emoji: '💊' },
  { key: 'flavor_sour' as const, label: 'Sour', emoji: '🍋' },
]

const ALL_POKEMON_TYPES = [
  'Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock',
  'Bug', 'Ghost', 'Fire', 'Water', 'Grass', 'Electric',
  'Psychic', 'Ice', 'Dragon',
]
</script>

<template>
  <div class="flex-1 flex flex-col min-h-0">
    <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
      <div class="flex items-baseline gap-3">
        <h2 class="text-accent text-sm font-bold">Item Editor</h2>
        <span v-if="currentItem" class="text-text text-[12px] font-mono">{{ currentItem.id }}</span>
        <span v-if="currentItem" class="text-text-muted text-[11px]">
          {{ currentItem.category }}
        </span>
      </div>
      <div class="flex gap-2">
        <button
          class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="!dirty"
          @click="store.saveItem()"
        >
          💾 Save
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-3 min-h-0">
      <div v-if="error" class="mb-3 p-2 rounded bg-danger/10 border border-danger text-danger text-[11px]">
        {{ error }}
      </div>

      <div v-if="!activeItemId" class="text-text-muted text-xs">
        Select an item from the sidebar to edit.
      </div>

      <div v-else-if="loading" class="text-text-muted text-xs">Loading...</div>

      <div v-else-if="currentItem" class="grid grid-cols-2 gap-4">
        <!-- Left Column: Basic Info -->
        <div class="space-y-4">
          <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
            <h3 class="text-accent text-[12px] font-bold mb-2">Basic Info</h3>

            <div class="mb-2">
              <label class="block text-[10px] text-text-muted mb-0.5">Name</label>
              <input
                type="text"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono uppercase"
                maxlength="20"
                :value="currentItem.name"
                @input="store.updateField('name', ($event.target as HTMLInputElement).value.toUpperCase())"
              />
            </div>

            <div class="grid grid-cols-2 gap-2 mb-2">
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Price (₽)</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="0"
                  max="999999"
                  :value="currentItem.price"
                  @change="setPrice(($event.target as HTMLInputElement).value)"
                />
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Sell Price</label>
                <div class="p-1 rounded bg-bg-inset text-text-muted text-[11px] font-mono">
                  ₽{{ sellPrice.toLocaleString() }}
                </div>
              </div>
            </div>

            <div class="mb-2">
              <label class="block text-[10px] text-text-muted mb-0.5">Category</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="currentItem.category"
                @change="store.updateField('category', ($event.target as HTMLSelectElement).value)"
              >
                <option v-for="cat in categories" :key="cat.id" :value="cat.id">{{ cat.label }}</option>
              </select>
            </div>

            <div class="flex items-center gap-4 mb-2">
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input
                  type="checkbox"
                  class="accent-accent"
                  :checked="currentItem.key_item"
                  @change="store.updateField('key_item', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-[11px]" :class="currentItem.key_item ? 'text-accent' : 'text-text-muted'">
                  🔑 Key Item
                </span>
              </label>
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input
                  type="checkbox"
                  class="accent-accent"
                  :checked="currentItem.sellable"
                  @change="store.updateField('sellable', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-[11px]" :class="currentItem.sellable ? 'text-text' : 'text-text-muted'">
                  💰 Sellable
                </span>
              </label>
            </div>

            <div class="mb-2">
              <label class="block text-[10px] text-text-muted mb-0.5">
                Tags (comma-separated)
              </label>
              <input
                type="text"
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
                :value="tagsString()"
                @input="setTags(($event.target as HTMLInputElement).value)"
              />
            </div>

            <div>
              <label class="block text-[10px] text-text-muted mb-0.5">Description</label>
              <textarea
                class="w-full p-1.5 rounded border border-accent bg-bg text-text text-[11px] leading-snug"
                rows="3"
                :value="currentItem.description"
                @input="store.updateField('description', ($event.target as HTMLTextAreaElement).value)"
              />
            </div>
          </section>
        </div>

        <!-- Right Column: Effects -->
        <div class="space-y-4">
          <!-- Active Effect -->
          <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
            <h3 class="text-accent text-[12px] font-bold mb-2">Active Effect</h3>
            <div class="mb-2">
              <label class="block text-[10px] text-text-muted mb-0.5">Effect Type</label>
              <select
                class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                :value="currentItem.effect?.type ?? 'None'"
                @change="store.updateEffectType(($event.target as HTMLSelectElement).value)"
              >
                <option
                  v-for="ef in EFFECT_REGISTRY.active_effects"
                  :key="ef.type"
                  :value="ef.type"
                >{{ ef.type }}</option>
              </select>
            </div>

            <div v-if="selectedActiveEffect && selectedActiveEffect.params.length > 0" class="space-y-2">
              <p class="text-text-muted text-[10px] mb-2">
                {{ selectedActiveEffect.params.length > 0 ? 'Parameters:' : 'No parameters for this effect.' }}
              </p>
              <div
                v-for="param in selectedActiveEffect.params"
                :key="param.name"
              >
                <label class="block text-[10px] text-text-muted mb-0.5 capitalize">
                  {{ param.name }}
                </label>
                <div v-if="param.type === 'number'">
                  <input
                    type="number"
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    :min="param.min ?? 0"
                    :max="param.max ?? 999"
                    :value="currentItem.effect?.params?.[param.name] ?? param.default ?? 0"
                    @input="store.updateEffectParam(
                      param.name,
                      Math.max(param.min ?? 0, Math.min(param.max ?? 999, parseInt(($event.target as HTMLInputElement).value, 10) || 0))
                    )"
                  />
                </div>
                <div v-else-if="param.type === 'select' && param.options">
                  <select
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    :value="String(currentItem.effect?.params?.[param.name] ?? param.default ?? '')"
                    @change="store.updateEffectParam(param.name, ($event.target as HTMLSelectElement).value)"
                  >
                    <option
                      v-for="opt in param.options"
                      :key="opt"
                      :value="opt"
                    >{{ opt }}</option>
                  </select>
                </div>
                <div v-else-if="(param.type === 'stat' || param.type === 'pokemonType') && param.options">
                  <select
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    :value="String(currentItem.effect?.params?.[param.name] ?? param.default ?? '')"
                    @change="store.updateEffectParam(param.name, ($event.target as HTMLSelectElement).value)"
                  >
                    <option
                      v-for="opt in param.options"
                      :key="opt"
                      :value="opt"
                    >{{ opt.charAt(0).toUpperCase() + opt.slice(1) }}</option>
                  </select>
                </div>
                <div v-else>
                  <input
                    type="text"
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    :value="String(currentItem.effect?.params?.[param.name] ?? '')"
                    @input="store.updateEffectParam(param.name, ($event.target as HTMLInputElement).value)"
                  />
                </div>
              </div>
            </div>
            <p v-if="selectedActiveEffect && selectedActiveEffect.params.length === 0" class="text-text-muted text-[10px]">
              No parameters for this effect.
            </p>
          </section>

          <!-- Held Effect -->
          <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
            <button
              class="w-full flex items-center justify-between text-accent text-[12px] font-bold cursor-pointer bg-transparent border-none p-0"
              @click="heldOpen = !heldOpen"
            >
              <span>Held Effect</span>
              <span class="text-[10px] text-text-muted">{{ heldOpen ? '▾' : '▸' }}</span>
            </button>

            <div v-show="heldOpen" class="mt-2 space-y-2">
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Trigger</label>
                <select
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  :value="currentItem.held_effect?.trigger ?? 'None'"
                  @change="store.setHeldTrigger(($event.target as HTMLSelectElement).value)"
                >
                  <option value="None">None</option>
                  <option
                    v-for="trigger in heldTriggers"
                    :key="trigger"
                    :value="trigger"
                  >{{ trigger }}</option>
                </select>
              </div>

              <template v-if="currentItem.held_effect && currentItem.held_effect.trigger !== 'None'">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Effect Type</label>
                  <select
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    :value="currentItem.held_effect.type"
                    @change="store.updateHeldEffectType(($event.target as HTMLSelectElement).value)"
                  >
                    <option
                      v-for="ef in EFFECT_REGISTRY.held_effects"
                      :key="ef.type"
                      :value="ef.type"
                    >{{ ef.type }}</option>
                  </select>
                </div>

                <div
                  v-if="selectedHeldEffect && selectedHeldEffect.params.length > 0"
                  class="space-y-2"
                >
                  <div
                    v-for="param in selectedHeldEffect.params"
                    :key="param.name"
                  >
                    <label class="block text-[10px] text-text-muted mb-0.5 capitalize">
                      {{ param.name }}
                    </label>
                    <div v-if="param.type === 'number'">
                      <input
                        type="number"
                        class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                        :min="param.min ?? 0"
                        :max="param.max ?? 999"
                        :value="currentItem.held_effect!.params?.[param.name] ?? param.default ?? 0"
                        @input="store.updateHeldEffectParam(
                          param.name,
                          Math.max(param.min ?? 0, Math.min(param.max ?? 999, parseInt(($event.target as HTMLInputElement).value, 10) || 0))
                        )"
                      />
                    </div>
                    <div v-else-if="(param.type === 'select' || param.type === 'stat' || param.type === 'pokemonType') && param.options">
                      <select
                        class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                        :value="String(currentItem.held_effect!.params?.[param.name] ?? param.default ?? '')"
                        @change="store.updateHeldEffectParam(param.name, ($event.target as HTMLSelectElement).value)"
                      >
                        <option
                          v-for="opt in param.options"
                          :key="opt"
                          :value="opt"
                        >{{ opt.charAt(0).toUpperCase() + opt.slice(1) }}</option>
                      </select>
                    </div>
                    <div v-else>
                      <input
                        type="text"
                        class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                        :value="String(currentItem.held_effect!.params?.[param.name] ?? '')"
                        @input="store.updateHeldEffectParam(param.name, ($event.target as HTMLInputElement).value)"
                      />
                    </div>
                  </div>
                </div>
              </template>
            </div>
          </section>

          <!-- Use Rules -->
          <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
            <h3 class="text-accent text-[12px] font-bold mb-2">Use Rules</h3>
            <div class="space-y-2">
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input
                  type="checkbox"
                  class="accent-accent"
                  :checked="currentItem.use_in_battle"
                  @change="store.updateField('use_in_battle', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-[11px]" :class="currentItem.use_in_battle ? 'text-text' : 'text-text-muted'">
                  ⚔ Use in Battle
                </span>
              </label>
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input
                  type="checkbox"
                  class="accent-accent"
                  :checked="currentItem.use_outside_battle"
                  @change="store.updateField('use_outside_battle', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-[11px]" :class="currentItem.use_outside_battle ? 'text-text' : 'text-text-muted'">
                  🌍 Use Outside Battle
                </span>
              </label>
              <label class="flex items-center gap-1.5 cursor-pointer">
                <input
                  type="checkbox"
                  class="accent-accent"
                  :checked="currentItem.consume"
                  @change="store.updateField('consume', ($event.target as HTMLInputElement).checked)"
                />
                <span class="text-[11px]" :class="currentItem.consume ? 'text-text' : 'text-text-muted'">
                  🗑 Consume on Use
                </span>
              </label>
            </div>
          </section>

          <!-- Berry Section -->
          <section
            v-if="isBerry"
            class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]"
          >
            <button
              class="w-full flex items-center justify-between text-accent text-[12px] font-bold cursor-pointer bg-transparent border-none p-0"
              @click="berryOpen = !berryOpen"
            >
              <span>🍓 Berry Data</span>
              <span class="text-[10px] text-text-muted">{{ berryOpen ? '▾' : '▸' }}</span>
            </button>

            <div v-show="berryOpen" class="mt-2 space-y-3">
              <div class="grid grid-cols-2 gap-2">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Growth Time (hours)</label>
                  <input
                    type="number"
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    min="1"
                    max="99"
                    :value="currentItem.berry?.growth_time ?? 4"
                    @input="store.updateBerry('growth_time', parseInt(($event.target as HTMLInputElement).value, 10) || 4)"
                  />
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Min Yield</label>
                  <input
                    type="number"
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    min="1"
                    max="10"
                    :value="currentItem.berry?.min_yield ?? 2"
                    @input="store.updateBerry('min_yield', parseInt(($event.target as HTMLInputElement).value, 10) || 2)"
                  />
                </div>
              </div>
              <div>
                <label class="block text-[10px] text-text-muted mb-0.5">Max Yield</label>
                <input
                  type="number"
                  class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                  min="1"
                  max="99"
                  :value="currentItem.berry?.max_yield ?? 5"
                  @input="store.updateBerry('max_yield', parseInt(($event.target as HTMLInputElement).value, 10) || 5)"
                />
              </div>

              <div>
                <h4 class="text-[11px] text-text mb-2 font-bold">Flavors (0–20)</h4>
                <div class="space-y-2">
                  <div v-for="flavor in FLAVOR_FIELDS" :key="flavor.key">
                    <label class="flex items-center gap-2">
                      <span class="text-[10px] text-text-muted w-14">{{ flavor.emoji }} {{ flavor.label }}</span>
                      <input
                        type="range"
                        class="flex-1 accent-accent"
                        min="0"
                        max="20"
                        :value="currentItem.berry?.[flavor.key] ?? 0"
                        @input="store.updateBerry(flavor.key, parseInt(($event.target as HTMLInputElement).value, 10) || 0)"
                      />
                      <span class="text-[10px] text-text-muted w-5 text-right font-mono">
                        {{ currentItem.berry?.[flavor.key] ?? 0 }}
                      </span>
                    </label>
                  </div>
                </div>
              </div>

              <div class="grid grid-cols-2 gap-2">
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Natural Gift Power</label>
                  <input
                    type="number"
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    min="0"
                    max="100"
                    :value="currentItem.berry?.natural_gift_power ?? 60"
                    @input="store.updateBerry('natural_gift_power', parseInt(($event.target as HTMLInputElement).value, 10) || 0)"
                  />
                </div>
                <div>
                  <label class="block text-[10px] text-text-muted mb-0.5">Natural Gift Type</label>
                  <select
                    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px]"
                    :value="currentItem.berry?.natural_gift_type ?? 'Normal'"
                    @change="store.updateBerry('natural_gift_type', ($event.target as HTMLSelectElement).value)"
                  >
                    <option
                      v-for="t in ALL_POKEMON_TYPES"
                      :key="t"
                      :value="t"
                    >{{ t }}</option>
                  </select>
                </div>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  </div>
</template>
