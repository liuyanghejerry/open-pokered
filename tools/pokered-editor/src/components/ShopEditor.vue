<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import { useItemStore } from '../stores/itemStore'
import { dataFetch } from '../composables/dataAdapter'
import type { ItemFile } from '../types/item'

const store = useItemStore()
const {
  shopList,
  activeShopId,
  currentShop,
  shopDirty,
  shopLoading,
  shopError,
} = storeToRefs(store)

const shopSearch = ref('')
const addModalOpen = ref(false)
const addSearch = ref('')
const itemCache = ref<Record<string, ItemFile | null>>({})
const itemList = ref<string[]>([])

const filteredShops = computed(() => {
  const q = shopSearch.value.toLowerCase().trim()
  if (!q) return shopList.value
  return shopList.value.filter((id) => {
    const shop = itemCache.value[id] as unknown as { id: string; name: string } | null
    if (!shop) return id.toLowerCase().includes(q)
    return (
      id.toLowerCase().includes(q) ||
      (shop as any).name?.toLowerCase().includes(q)
    )
  })
})

const shopDisplayName = computed(() => {
  if (!currentShop.value) return ''
  return currentShop.value.name || currentShop.value.id
})

const availableItems = computed(() => {
  const q = addSearch.value.toLowerCase().trim()
  return itemList.value.filter((id) => {
    if (currentShop.value?.items.includes(id)) return false
    const info = itemCache.value[id]
    if (info?.key_item) return false
    if (!q) return true
    return (
      id.toLowerCase().includes(q) ||
      (info?.name ?? '').toLowerCase().includes(q)
    )
  })
})

function itemName(id: string): string {
  const info = itemCache.value[id]
  return info?.name ?? id
}

onMounted(async () => {
  await store.loadShopList()
  if (shopList.value.length > 0) {
    selectShop(shopList.value[0])
  }
  await loadItemList()
})

async function loadItemList() {
  try {
    const res = await dataFetch('/api/items')
    if (!res.ok) return
    const data = await res.json()
    itemList.value = Array.isArray(data) ? data : (data.items ?? data)
  } catch {
    // silently ignore
  }
}

async function fetchItemInfo(id: string): Promise<ItemFile | null> {
  if (id in itemCache.value) return itemCache.value[id]
  try {
    const res = await dataFetch(`/api/items/${encodeURIComponent(id)}`)
    if (!res.ok) {
      itemCache.value[id] = null
      return null
    }
    const data = await res.json() as ItemFile
    itemCache.value[id] = data
    return data
  } catch {
    itemCache.value[id] = null
    return null
  }
}

async function selectShop(id: string) {
  await store.loadShop(id)
  if (currentShop.value) {
    for (const itemId of currentShop.value.items) {
      fetchItemInfo(itemId)
    }
  }
}

async function openAddModal() {
  addSearch.value = ''
  addModalOpen.value = true
  for (const id of itemList.value) {
    if (!(id in itemCache.value)) {
      fetchItemInfo(id)
    }
  }
}

function addItem(itemId: string) {
  store.addShopItem(itemId)
  addSearch.value = ''
  addModalOpen.value = false
  fetchItemInfo(itemId)
}

function removeItem(index: number) {
  store.removeShopItem(index)
}

function moveItem(index: number, direction: 'up' | 'down') {
  store.moveShopItem(index, direction)
}
</script>

<template>
  <div class="flex h-full min-h-0">
    <!-- Left panel: Shop list -->
    <div class="w-64 flex-shrink-0 overflow-y-auto bg-bg-panel border-r border-[rgba(255,255,255,0.06)]">
      <div class="p-3">
        <h2 class="text-accent text-sm font-bold mb-3">SHOPS</h2>

        <input
          v-model="shopSearch"
          type="text"
          placeholder="Search shops..."
          class="w-full p-1.5 mb-2 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-[11px]"
        />

        <div class="text-[10px] text-text-muted mb-1">
          {{ filteredShops.length }} / {{ shopList.length }}
        </div>

        <nav class="space-y-0.5">
          <button
            v-for="id in filteredShops"
            :key="id"
            class="shop-nav-btn"
            :class="{ active: activeShopId === id }"
            @click="selectShop(id)"
          >
            <span class="shop-nav-label">{{ id }}</span>
          </button>
        </nav>

        <div
          v-if="filteredShops.length === 0"
          class="text-[11px] text-text-muted mt-2"
        >
          No matching shops.
        </div>
      </div>
    </div>

    <!-- Right panel: Shop inventory editor -->
    <div class="flex-1 flex flex-col min-h-0">
      <!-- Header -->
      <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
        <div class="flex items-baseline gap-3">
          <h2 class="text-accent text-sm font-bold">Shop Editor</h2>
          <span v-if="currentShop" class="text-text text-[12px] font-mono">
            {{ shopDisplayName }}
          </span>
        </div>
        <div class="flex gap-2">
          <button
            class="px-3 py-1 rounded text-[11px] font-bold cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="!shopDirty"
            @click="store.saveShop()"
          >
            💾 Save
          </button>
        </div>
      </div>

      <!-- Content -->
      <div class="flex-1 overflow-y-auto p-3 min-h-0">
        <!-- Error -->
        <div
          v-if="shopError"
          class="mb-3 p-2 rounded bg-danger/10 border border-danger text-danger text-[11px]"
        >
          {{ shopError }}
        </div>

        <!-- Empty state -->
        <div
          v-if="!activeShopId"
          class="text-text-muted text-xs"
        >
          Select a shop from the sidebar to edit.
        </div>

        <div
          v-else-if="shopLoading"
          class="text-text-muted text-xs"
        >
          Loading...
        </div>

        <!-- Inventory -->
        <div v-else-if="currentShop" class="space-y-4">
          <section class="bg-bg p-3 rounded border border-[rgba(255,255,255,0.06)]">
            <h3 class="text-accent text-[12px] font-bold mb-3">
              Inventory ({{ currentShop.items.length }} items)
            </h3>

            <!-- Item rows -->
            <div v-if="currentShop.items.length === 0" class="text-text-muted text-[11px] py-2">
              No items in this shop.
            </div>

            <div v-else class="space-y-1">
              <div
                v-for="(itemId, index) in currentShop.items"
                :key="index"
                class="flex items-center gap-2 p-2 rounded bg-bg-inset border border-[rgba(255,255,255,0.04)] hover:border-[rgba(255,255,255,0.08)] transition-colors"
              >
                <div class="flex flex-col gap-0.5 shrink-0">
                  <button
                    class="w-5 h-4 flex items-center justify-center rounded text-[9px] cursor-pointer bg-bg text-text-muted border border-[rgba(255,255,255,0.08)] hover:text-accent hover:border-accent disabled:opacity-20 disabled:cursor-not-allowed"
                    title="Move up"
                    :disabled="index === 0"
                    @click="moveItem(index, 'up')"
                  >
                    ▲
                  </button>
                  <button
                    class="w-5 h-4 flex items-center justify-center rounded text-[9px] cursor-pointer bg-bg text-text-muted border border-[rgba(255,255,255,0.08)] hover:text-accent hover:border-accent disabled:opacity-20 disabled:cursor-not-allowed"
                    title="Move down"
                    :disabled="index === currentShop!.items.length - 1"
                    @click="moveItem(index, 'down')"
                  >
                    ▼
                  </button>
                </div>

                <span class="text-text-muted text-[9px] font-mono w-5 text-right shrink-0">
                  {{ index + 1 }}
                </span>

                <span class="flex-1 text-text text-[12px]">
                  {{ itemName(itemId) }}
                </span>

                <span class="text-text-muted text-[10px] font-mono shrink-0">
                  {{ itemId }}
                </span>

                <button
                  class="w-6 h-6 flex items-center justify-center rounded text-[12px] font-bold cursor-pointer bg-transparent text-text-muted border border-[rgba(255,255,255,0.08)] hover:text-danger hover:border-danger transition-colors"
                  title="Remove item"
                  @click="removeItem(index)"
                >
                  ×
                </button>
              </div>
            </div>

            <!-- Add item button -->
            <div class="mt-3">
              <button
                class="w-full px-3 py-2 rounded text-[11px] font-bold cursor-pointer bg-bg-inset text-accent border border-dashed border-[rgba(255,255,255,0.1)] hover:border-accent hover:bg-accent/5 transition-colors"
                @click="openAddModal()"
              >
                + Add Item
              </button>
            </div>
          </section>
        </div>
      </div>
    </div>

    <!-- Add Item Modal -->
    <Teleport to="body">
      <div
        v-if="addModalOpen"
        class="fixed inset-0 z-50 flex items-center justify-center"
        style="background: rgba(0, 0, 0, 0.6)"
        @click.self="addModalOpen = false"
      >
        <div class="bg-bg-panel rounded-lg border border-[rgba(255,255,255,0.1)] shadow-2xl w-[420px] max-h-[70vh] flex flex-col">
          <!-- Modal Header -->
          <div class="flex items-center justify-between p-3 border-b border-[rgba(255,255,255,0.06)] shrink-0">
            <h3 class="text-accent text-sm font-bold">Add Item</h3>
            <button
              class="w-6 h-6 flex items-center justify-center rounded text-text-muted cursor-pointer bg-transparent border border-[rgba(255,255,255,0.08)] hover:text-text hover:border-[rgba(255,255,255,0.2)] transition-colors"
              @click="addModalOpen = false"
            >
              ×
            </button>
          </div>

          <!-- Search -->
          <div class="p-3 shrink-0">
            <input
              v-model="addSearch"
              type="text"
              placeholder="Search items..."
              class="w-full p-2 rounded border border-[rgba(255,255,255,0.1)] bg-bg text-text text-xs"
              autofocus
            />
            <div class="text-[10px] text-text-muted mt-1">
              {{ availableItems.length }} items available
            </div>
          </div>

          <!-- Item List -->
          <div class="flex-1 overflow-y-auto px-3 pb-3 min-h-0">
            <div v-if="availableItems.length === 0" class="text-[11px] text-text-muted py-4 text-center">
              No matching items.
            </div>
            <button
              v-for="itemId in availableItems"
              :key="itemId"
              class="w-full flex items-center gap-2 px-2 py-1.5 rounded text-left cursor-pointer bg-transparent border-none text-text text-[12px] hover:bg-accent/10 hover:text-accent transition-colors"
              @click="addItem(itemId)"
            >
              <span class="flex-1">{{ itemName(itemId) }}</span>
              <span class="text-text-muted text-[10px] font-mono">{{ itemId }}</span>
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.shop-nav-btn {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px;
  font-size: 11px;
  font-weight: 500;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  border-radius: 4px;
  text-align: left;
  transition: background 0.15s, color 0.15s;
}

.shop-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.shop-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.shop-nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
