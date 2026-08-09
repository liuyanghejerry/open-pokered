import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ItemFile, CategoryDef, EffectRegistry, ShopData } from '../types/item'
import { EFFECT_REGISTRY, defaultEffectParams } from '../types/item'
import { dataFetch } from '../composables/dataAdapter'
import { injectItem } from '../composables/usePokeredRunner'

export const useItemStore = defineStore('item', () => {
  const itemList = ref<string[]>([])
  const activeItemId = ref<string | null>(null)
  const currentItem = ref<ItemFile | null>(null)
  const categories = ref<CategoryDef[]>([])
  const effectRegistry = ref<EffectRegistry>(EFFECT_REGISTRY)
  const shops = ref<ShopData[]>([])
  const shopList = ref<string[]>([])
  const activeShopId = ref<string | null>(null)
  const currentShop = ref<ShopData | null>(null)
  const shopDirty = ref(false)
  const shopLoading = ref(false)
  const shopError = ref<string | null>(null)
  const selectedCategory = ref<string | null>(null)
  const searchQuery = ref('')
  const loading = ref(false)
  const dirty = ref(false)
  const error = ref<string | null>(null)
  const itemsByCategory = ref<Record<string, string[]>>({})

  const items = computed(() => itemList.value)

  const filteredItems = computed(() => {
    let list = itemList.value
    if (searchQuery.value) {
      const q = searchQuery.value.toLowerCase()
      list = list.filter(id => id.toLowerCase().includes(q))
    }
    if (selectedCategory.value) {
      const catItems = itemsByCategory.value[selectedCategory.value]
      if (catItems) {
        const catSet = new Set(catItems)
        list = list.filter(id => catSet.has(id))
      }
    }
    return list
  })

  async function loadItemList() {
    try {
      const res = await dataFetch('/api/items')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      itemList.value = Array.isArray(data) ? data : (data.items ?? data)
    } catch (e) {
      error.value = `Failed to load item list: ${(e as Error).message}`
    }
  }

  async function loadItem(id: string) {
    if (dirty.value && !confirm('Discard unsaved item changes?')) return
    loading.value = true
    error.value = null
    try {
      const res = await dataFetch(`/api/items/${encodeURIComponent(id)}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      currentItem.value = await res.json()
      activeItemId.value = id
      dirty.value = false
    } catch (e) {
      error.value = `Failed to load ${id}: ${(e as Error).message}`
      currentItem.value = null
    } finally {
      loading.value = false
    }
  }

  async function saveItem() {
    if (!currentItem.value || !activeItemId.value) return
    try {
      const res = await dataFetch(`/api/items/${encodeURIComponent(activeItemId.value)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(currentItem.value),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      dirty.value = false
      // WYSIWYG: push the saved item into the running game.
      try {
        await injectItem(activeItemId.value, JSON.stringify(currentItem.value))
      } catch {
        /* preview injection is best-effort */
      }
    } catch (e) {
      error.value = `Failed to save: ${(e as Error).message}`
    }
  }

  /**
   * Create a new item: the server writes `data/items/<name>.json` AND appends
   * the name to `item_list.json` (the ItemId enum-order source), so the next
   * `cargo build` picks it up. Returns false and sets `error` on failure.
   */
  async function createItem(name: string): Promise<boolean> {
    try {
      const res = await dataFetch('/api/items', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      })
      if (!res.ok) {
        const msg = await res.json().catch(() => null)
        error.value = `Failed to create ${name}: ${msg?.error ?? `HTTP ${res.status}`}`
        return false
      }
      dirty.value = false // the confirm in loadItem would block the fresh load otherwise
      await loadItemList()
      await loadItem(name)
      return true
    } catch (e) {
      error.value = `Failed to create ${name}: ${(e as Error).message}`
      return false
    }
  }

  function updateField<K extends keyof ItemFile>(field: K, value: ItemFile[K]) {
    if (!currentItem.value) return
    currentItem.value[field] = value
    dirty.value = true
  }

  function updateEffectType(type: string) {
    if (!currentItem.value) return
    const params = defaultEffectParams(type, 'active')
    currentItem.value.effect = { type, params }
    dirty.value = true
  }

  function updateEffectParam(name: string, value: any) {
    if (!currentItem.value?.effect) return
    currentItem.value.effect = {
      ...currentItem.value.effect,
      params: { ...currentItem.value.effect.params, [name]: value },
    }
    dirty.value = true
  }

  function setHeldTrigger(trigger: string) {
    if (!currentItem.value) return
    if (trigger === 'None') {
      currentItem.value.held_effect = null
    } else {
      currentItem.value.held_effect = {
        trigger,
        type: 'None',
        params: {},
      }
    }
    dirty.value = true
  }

  function updateHeldEffectType(type: string) {
    if (!currentItem.value?.held_effect) return
    const params = defaultEffectParams(type, 'held')
    currentItem.value.held_effect = { ...currentItem.value.held_effect, type, params }
    dirty.value = true
  }

  function updateHeldEffectParam(name: string, value: any) {
    if (!currentItem.value?.held_effect) return
    currentItem.value.held_effect = {
      ...currentItem.value.held_effect,
      params: { ...currentItem.value.held_effect.params, [name]: value },
    }
    dirty.value = true
  }

  function updateBerry(field: keyof NonNullable<ItemFile['berry']>, value: number | string) {
    if (!currentItem.value?.berry) return
    currentItem.value.berry = {
      ...currentItem.value.berry,
      [field]: field.startsWith('flavor_') || field.startsWith('natural_gift') || field === 'growth_time' || field === 'min_yield' || field === 'max_yield' || field === 'smoothness'
        ? Number(value) || 0
        : value,
    }
    dirty.value = true
  }

  async function loadCategories() {
    try {
      const byCat: Record<string, string[]> = {}
      for (const name of itemList.value) {
        try {
          const r = await dataFetch(`/api/items/${encodeURIComponent(name)}`)
          if (r.ok) {
            const item: ItemFile = await r.json()
            if (item.category) {
              if (!byCat[item.category]) byCat[item.category] = []
              byCat[item.category].push(name)
            }
          }
        } catch { /* skip */ }
      }
      itemsByCategory.value = byCat
      const defs: CategoryDef[] = []
      const catColors: Record<string, string> = {
        Medicine: '#e74c3c',
        'Poké Balls': '#f39c12',
        Battle: '#8e44ad',
        Items: '#2ecc71',
        Evolution: '#e67e22',
        Key: '#1abc9c',
      }
      for (const cat of Object.keys(byCat).sort()) {
        defs.push({ id: cat, label: cat, icon_tile: 0, color: catColors[cat] ?? '#888' })
      }
      categories.value = defs
    } catch (e) {
      error.value = `Failed to load categories: ${(e as Error).message}`
    }
  }

  function setCategory(cat: string | null) {
    selectedCategory.value = cat
  }

  // ── Shop methods ──────────────────────────────────────────────────────

  async function loadShopList() {
    shopError.value = null
    try {
      const res = await dataFetch('/api/shops')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      shopList.value = await res.json()
    } catch (e) {
      shopError.value = `Failed to load shop list: ${(e as Error).message}`
    }
  }

  async function loadShop(id: string) {
    if (shopDirty.value && !confirm('Discard unsaved shop changes?')) return
    shopLoading.value = true
    shopError.value = null
    try {
      const res = await dataFetch(`/api/shops/${encodeURIComponent(id)}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      currentShop.value = await res.json()
      activeShopId.value = id
      shopDirty.value = false
    } catch (e) {
      shopError.value = `Failed to load ${id}: ${(e as Error).message}`
      currentShop.value = null
    } finally {
      shopLoading.value = false
    }
  }

  async function saveShop() {
    if (!currentShop.value || !activeShopId.value) return
    try {
      const res = await dataFetch(`/api/shops/${encodeURIComponent(activeShopId.value)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(currentShop.value),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      shopDirty.value = false
    } catch (e) {
      shopError.value = `Failed to save shop: ${(e as Error).message}`
    }
  }

  function addShopItem(itemId: string) {
    if (!currentShop.value) return
    if (currentShop.value.items.includes(itemId)) return
    currentShop.value = {
      ...currentShop.value,
      items: [...currentShop.value.items, itemId],
    }
    shopDirty.value = true
  }

  function removeShopItem(index: number) {
    if (!currentShop.value) return
    const items = [...currentShop.value.items]
    items.splice(index, 1)
    currentShop.value = { ...currentShop.value, items }
    shopDirty.value = true
  }

  function moveShopItem(index: number, direction: 'up' | 'down') {
    if (!currentShop.value) return
    const items = [...currentShop.value.items]
    const newIndex = direction === 'up' ? index - 1 : index + 1
    if (newIndex < 0 || newIndex >= items.length) return
    const temp = items[index]
    items[index] = items[newIndex]
    items[newIndex] = temp
    currentShop.value = { ...currentShop.value, items }
    shopDirty.value = true
  }

  return {
    itemList,
    items,
    activeItemId,
    currentItem,
    categories,
    effectRegistry,
    shops,
    shopList,
    activeShopId,
    currentShop,
    shopDirty,
    shopLoading,
    shopError,
    selectedCategory,
    searchQuery,
    loading,
    dirty,
    error,
    itemsByCategory,
    filteredItems,
    loadItemList,
    loadItem,
    createItem,
    saveItem,
    updateField,
    updateEffectType,
    updateEffectParam,
    setHeldTrigger,
    updateHeldEffectType,
    updateHeldEffectParam,
    updateBerry,
    loadCategories,
    setCategory,
    loadShopList,
    loadShop,
    saveShop,
    addShopItem,
    removeShopItem,
    moveShopItem,
  }
})
