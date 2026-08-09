<script setup lang="ts">
import { ref } from 'vue'
import { useMapStore } from '../stores/mapStore'
import { storeToRefs } from 'pinia'

const store = useMapStore()
const { currentMap } = storeToRefs(store)

const showAddCoordEvent = ref(false)
const newCoordX = ref(0)
const newCoordY = ref(0)
const newCoordTrigger = ref('')

function addCoordEvent() {
  if (!newCoordTrigger.value.trim()) return
  store.addCoordEvent(newCoordX.value, newCoordY.value, newCoordTrigger.value.trim())
  showAddCoordEvent.value = false
  newCoordX.value = 0
  newCoordY.value = 0
  newCoordTrigger.value = ''
}

function toHex(n: number, pad = 2): string {
  return '0x' + n.toString(16).padStart(pad, '0')
}

function selectWarp(index: number) {
  const warp = currentMap.value?.warps?.[index]
  if (warp) store.selectEntity({ type: 'warp', data: warp, index })
}

function selectSign(index: number) {
  const sign = currentMap.value?.signs?.[index]
  if (sign) store.selectEntity({ type: 'sign', data: sign, index })
}

function selectNpc(index: number) {
  const npc = currentMap.value?.npcs?.[index]
  if (npc) store.selectEntity({ type: 'npc', data: npc, index })
}

function selectCoordEvent(index: number) {
  const ce = store.currentScriptConfig?.coordEvents?.[index]
  if (ce) {
    store.selectEntity({
      type: 'coordEvent',
      data: { x: ce.position[0], y: ce.position[1], trigger: ce.trigger },
      index,
    })
  }
}

function isSelected(type: string, index: number): boolean {
  return store.selectedEntity?.type === type && store.selectedEntity?.index === index
}
</script>

<template>
  <div class="bg-bg-inset p-2.5 rounded-md font-mono text-[11px]">
    <h3 class="text-accent text-[13px] font-bold mb-2 font-sans">Map Info</h3>
    <template v-if="currentMap">
      <p class="my-0.5"><b>{{ currentMap.name }}</b></p>
      <p class="my-0.5">ID: <code class="text-accent">{{ toHex(currentMap.id) }}</code></p>
      <p class="my-0.5">Size: {{ currentMap.header.width }}x{{ currentMap.header.height }} blocks</p>
      <p class="my-0.5">Tiles: {{ currentMap.header.width * 4 }}x{{ currentMap.header.height * 4 }}</p>
      <p class="my-0.5">Tileset: {{ currentMap.header.tileset }}</p>

      <template v-if="currentMap.warps && currentMap.warps.length > 0">
        <p class="my-0.5"><b>Warps ({{ currentMap.warps.length }}):</b></p>
        <p
          v-for="(warp, i) in currentMap.warps"
          :key="i"
          class="my-0.5 ml-2.5 cursor-pointer hover:text-accent transition-colors"
          :class="isSelected('warp', i) ? 'text-accent font-bold' : ''"
          @click="selectWarp(i)"
        >
          Warp {{ i }}: ({{ warp.x }}, {{ warp.y }})
          <template v-if="warp.destMap"> → {{ warp.destMap }}</template>
        </p>
      </template>

      <template v-if="currentMap.signs && currentMap.signs.length > 0">
        <p class="my-0.5"><b>Signs ({{ currentMap.signs.length }}):</b></p>
        <p
          v-for="(sign, i) in currentMap.signs"
          :key="'sign-' + i"
          class="my-0.5 ml-2.5 cursor-pointer hover:text-[#f1c40f] transition-colors"
          :class="isSelected('sign', i) ? 'text-[#f1c40f] font-bold' : ''"
          @click="selectSign(i)"
        >
          Sign {{ i }}: ({{ sign.x }}, {{ sign.y }}) text#{{ sign.textId }}
          <template v-if="sign.talk"> → <span class="text-accent cursor-pointer hover:underline" @click.stop="store.jumpToFunction(sign.talk!)">{{ sign.talk }}</span></template>
        </p>
      </template>

      <template v-if="store.currentScriptConfig?.coordEvents?.length || store.currentScriptConfig">
        <div class="flex items-center justify-between my-0.5">
          <b>Coord Events ({{ store.currentScriptConfig?.coordEvents?.length ?? 0 }}):</b>
          <button
            v-if="store.currentScriptConfig && !showAddCoordEvent"
            class="text-[10px] px-1.5 py-0.5 bg-accent text-bg border-none rounded cursor-pointer hover:opacity-80"
            @click="showAddCoordEvent = true"
          >+ Add</button>
        </div>
        <div v-if="showAddCoordEvent" class="ml-2.5 my-1 p-2 bg-bg rounded border border-accent">
          <div class="flex items-center gap-2 mb-1">
            <label class="text-[10px] text-text-muted">X:</label>
            <input v-model.number="newCoordX" type="number" min="0" class="w-12 p-0.5 rounded border border-accent bg-bg-inset text-text text-[10px] font-mono" />
            <label class="text-[10px] text-text-muted">Y:</label>
            <input v-model.number="newCoordY" type="number" min="0" class="w-12 p-0.5 rounded border border-accent bg-bg-inset text-text text-[10px] font-mono" />
          </div>
          <div class="flex items-center gap-2 mb-1">
            <label class="text-[10px] text-text-muted">Trigger:</label>
            <input v-model="newCoordTrigger" type="text" placeholder="functionName" class="flex-1 p-0.5 rounded border border-accent bg-bg-inset text-text text-[10px] font-mono" @keyup.enter="addCoordEvent" />
          </div>
          <div class="flex gap-1">
            <button class="text-[10px] px-2 py-0.5 bg-accent text-bg border-none rounded cursor-pointer hover:opacity-80" @click="addCoordEvent">Add</button>
            <button class="text-[10px] px-2 py-0.5 bg-transparent text-text-muted border border-text-muted rounded cursor-pointer hover:text-text" @click="showAddCoordEvent = false">Cancel</button>
          </div>
        </div>
        <p v-for="(ce, i) in store.currentScriptConfig?.coordEvents" :key="'ce-' + i"
           class="my-0.5 ml-2.5 cursor-pointer hover:text-[#e67e22] transition-colors"
           :class="isSelected('coordEvent', i) ? 'text-[#e67e22] font-bold' : ''"
           @click="selectCoordEvent(i)">
          ({{ ce.position[0] }}, {{ ce.position[1] }}) → <span class="cursor-pointer hover:underline" @click.stop="store.jumpToFunction(ce.trigger)">{{ ce.trigger }}</span>
        </p>
      </template>

      <template v-if="currentMap">
        <div class="flex items-center justify-between my-0.5">
          <b>NPCs ({{ currentMap.npcs?.length ?? 0 }}):</b>
          <button
            class="text-[10px] px-1.5 py-0.5 bg-accent text-bg border-none rounded cursor-pointer hover:opacity-80"
            @click="store.addNpc()"
          >+ Add</button>
        </div>
        <p
          v-for="(npc, i) in currentMap.npcs"
          :key="'npc-' + i"
          class="my-0.5 ml-2.5 cursor-pointer hover:opacity-80 transition-colors"
          :class="isSelected('npc', i) ? 'font-bold' : ''"
          @click="selectNpc(i)"
        >
          <span :class="npc.isTrainer ? 'text-danger' : npc.itemId != null ? 'text-accent' : 'text-text'">
            {{ npc.spriteName ?? `sprite#${npc.spriteId}` }}
          </span>
          ({{ npc.x }}, {{ npc.y }})
          <template v-if="npc.isTrainer"> {{ npc.trainerClass }}#{{ npc.trainerSet }}</template>
          <template v-if="npc.itemId != null"> item={{ toHex(npc.itemId) }}</template>
          <template v-if="npc.talk"> → <span class="text-accent cursor-pointer hover:underline" @click.stop="store.jumpToFunction(npc.talk!)">{{ npc.talk }}</span></template>
        </p>
      </template>
    </template>
    <template v-else>
      <p>Loading map data...</p>
    </template>
  </div>
</template>
