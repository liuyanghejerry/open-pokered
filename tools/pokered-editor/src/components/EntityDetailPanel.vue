<script setup lang="ts">
import { useMapStore } from '../stores/mapStore'
import { useTrainerStore } from '../stores/trainerStore'
import { injectTrainer } from '../composables/usePokeredRunner'
import { usePlaytestOverlay } from '../composables/usePlaytestOverlay'
import { storeToRefs } from 'pinia'
import { useRouter } from 'vue-router'

const store = useMapStore()
const trainerStore = useTrainerStore()
const playtestOverlay = usePlaytestOverlay()
const { selectedEntity } = storeToRefs(store)
const router = useRouter()

function toHex(n: number, pad = 2): string {
  return '0x' + n.toString(16).padStart(pad, '0')
}

function editTrainerTeam(trainerClass: string) {
  router.push(`/trainer/${trainerClass}`)
}

/**
 * Open the floating playtest in a trainer battle vs the selected NPC's class
 * & set (the in-game opponent this NPC actually fights). If the trainer
 * editor has that class loaded, its current (possibly unsaved) parties are
 * injected first.
 */
async function testTrainerBattle() {
  const sel = selectedEntity.value
  if (!sel || sel.type !== 'npc') return
  const t = sel.data
  if (!t.isTrainer || !t.trainerClass) return
  if (trainerStore.activeClass === t.trainerClass && trainerStore.data) {
    try {
      await injectTrainer(t.trainerClass, JSON.stringify(trainerStore.data))
    } catch {
      /* preview injection is best-effort */
    }
  }
  playtestOverlay.launch({
    kind: 'trainerBattle',
    class: t.trainerClass,
    partyIndex: Math.max(0, (t.trainerSet ?? 1) - 1),
  })
}
</script>

<template>
  <div v-if="selectedEntity" class="bg-bg-panel/95 p-2.5 rounded border border-accent max-w-[320px] max-h-[55vh] overflow-y-auto">
    <div class="flex items-center justify-between mb-2">
      <h3 class="text-accent text-[13px] font-bold">
        {{ selectedEntity.type === 'sign' ? 'Sign Detail' : selectedEntity.type === 'npc' ? 'NPC Detail' : selectedEntity.type === 'coordEvent' ? 'Coord Event Detail' : 'Warp Detail' }}
      </h3>
      <button
        class="text-[10px] text-text-muted hover:text-text cursor-pointer bg-transparent border-none"
        @click="store.selectEntity(null)"
      >
        ✕ Close
      </button>
    </div>

    <template v-if="selectedEntity.type === 'sign'">
  <div v-if="selectedEntity.data.talk" class="mt-1">
    <span class="text-text-muted">Script: </span>
    <span class="text-accent cursor-pointer hover:underline" @click="store.jumpToFunction(selectedEntity!.data.talk!)">{{ selectedEntity.data.talk }}</span>
  </div>
  <label class="block text-[10px] text-text-muted mt-2">Script Function:</label>
  <input
    type="text"
    :value="selectedEntity.data.talk || ''"
    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono mt-0.5"
    placeholder="e.g. signOakLab"
    @change="store.updateSignTalk(selectedEntity!.index, ($event.target as HTMLInputElement).value)"
  />
      <div class="font-mono text-[11px] space-y-1">
        <p>Position: ({{ selectedEntity.data.x }}, {{ selectedEntity.data.y }})</p>
        <p>Text ID: {{ selectedEntity.data.textId }}</p>
      </div>
      <div
        v-if="selectedEntity.data.textId != null"
        class="mt-2"
      >
        <p class="text-[10px] text-text-muted italic">Text data in map.json text section</p>
      </div>
    </template>

    <template v-if="selectedEntity.type === 'npc'">
  <div v-if="selectedEntity.data.talk" class="mt-1">
    <span class="text-text-muted">Script: </span>
    <span class="text-accent cursor-pointer hover:underline" @click="store.jumpToFunction(selectedEntity!.data.talk!)">{{ selectedEntity.data.talk }}</span>
  </div>
  <label class="block text-[10px] text-text-muted mt-2">Script Function:</label>
  <input
    type="text"
    :value="selectedEntity.data.talk || ''"
    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono mt-0.5"
    placeholder="e.g. talkOak"
    @change="store.updateNpcTalk(selectedEntity!.index, ($event.target as HTMLInputElement).value)"
  />
  <label class="block text-[10px] text-text-muted mt-2">Toggle ID:</label>
  <input
    type="text"
    :value="selectedEntity.data.toggleId || ''"
    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono mt-0.5"
    placeholder="e.g. OAKS_LAB_OBJ_2"
    @change="store.updateNpcToggleId(selectedEntity!.index, ($event.target as HTMLInputElement).value)"
  />
  <div class="flex items-center mt-2">
    <input
      type="checkbox"
      :checked="selectedEntity.data.defaultHidden || false"
      @change="store.updateNpcDefaultHidden(selectedEntity!.index, ($event.target as HTMLInputElement).checked)"
    />
    <label class="text-[11px] text-text-muted ml-1">Hidden by default</label>
  </div>
  <div class="font-mono text-[11px] space-y-1">
        <p>
          <span
            :class="selectedEntity.data.isTrainer ? 'text-danger' : selectedEntity.data.itemId != null ? 'text-accent' : 'text-[#9b59b6]'"
            class="font-bold"
          >
            {{ selectedEntity.data.spriteName }}
          </span>
        </p>
        <p>Position: ({{ selectedEntity.data.x }}, {{ selectedEntity.data.y }})</p>
        <p>Movement: {{ selectedEntity.data.movement }} / {{ selectedEntity.data.facing }}</p>
        <p v-if="selectedEntity.data.range > 0">Range: {{ selectedEntity.data.range }}</p>
        <p v-if="selectedEntity.data.isTrainer">
          Trainer: {{ selectedEntity.data.trainerClass }} #{{ selectedEntity.data.trainerSet }}
        </p>
        <div class="flex gap-1.5 mt-1">
          <button
            v-if="selectedEntity.data.isTrainer && selectedEntity.data.trainerClass"
            class="px-2 py-0.5 rounded text-[11px] cursor-pointer bg-bg-inset text-text border border-[rgba(255,255,255,0.1)] hover:border-accent"
            title="Fight this trainer's party in the playtest"
            @click="testTrainerBattle"
          >
            ⚔ 试玩对战
          </button>
          <button
            v-if="selectedEntity.data.isTrainer && selectedEntity.data.trainerClass"
            class="px-2 py-0.5 rounded text-[11px] cursor-pointer bg-accent text-bg border-none hover:bg-accent-hover"
            @click="editTrainerTeam(selectedEntity.data.trainerClass)"
          >
            🎓 Edit Trainer Team
          </button>
        </div>
        <p v-if="selectedEntity.data.itemId != null">
          Item: {{ toHex(selectedEntity.data.itemId) }}
        </p>
      </div>
      <div
        v-if="selectedEntity.data.textId != null"
        class="mt-2"
      >
        <p class="text-[10px] text-text-muted italic">Text data in map.json text section</p>
      </div>
      <button
        class="mt-2 px-2 py-0.5 rounded text-[11px] cursor-pointer bg-transparent text-danger border border-danger/40 hover:bg-danger/10"
        @click="store.removeNpc(selectedEntity!.index)"
      >
        🗑 Delete NPC
      </button>
    </template>

    <template v-if="selectedEntity.type === 'coordEvent'">
  <div v-if="selectedEntity.data.trigger" class="mt-1">
    <span class="text-text-muted">Script: </span>
    <span class="text-accent cursor-pointer hover:underline" @click="store.jumpToFunction(selectedEntity!.data.trigger)">{{ selectedEntity.data.trigger }}</span>
  </div>
  <label class="block text-[10px] text-text-muted mt-2">Trigger Function:</label>
  <input
    type="text"
    :value="selectedEntity.data.trigger"
    class="w-full p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono mt-0.5"
    placeholder="e.g. coordExitRow"
    @change="store.updateCoordEvent(selectedEntity!.index, { trigger: ($event.target as HTMLInputElement).value })"
  />
  <div class="font-mono text-[11px] space-y-1 mt-2">
    <div class="flex items-center gap-2">
      <span class="text-text-muted">X:</span>
      <input
        type="number"
        :value="selectedEntity.data.x"
        class="w-16 p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
        min="0"
        @change="store.updateCoordEvent(selectedEntity!.index, { x: parseInt(($event.target as HTMLInputElement).value) })"
      />
      <span class="text-text-muted">Y:</span>
      <input
        type="number"
        :value="selectedEntity.data.y"
        class="w-16 p-1 rounded border border-accent bg-bg text-text text-[11px] font-mono"
        min="0"
        @change="store.updateCoordEvent(selectedEntity!.index, { y: parseInt(($event.target as HTMLInputElement).value) })"
      />
    </div>
  </div>
  <button
    class="mt-3 px-3 py-1.5 bg-danger text-white border-none rounded cursor-pointer text-[11px] font-bold hover:opacity-80 w-full"
    @click="store.removeCoordEvent(selectedEntity!.index)"
  >
    Delete Coord Event
  </button>
</template>

<template v-if="selectedEntity.type === 'warp'">
      <div class="font-mono text-[11px] space-y-1">
        <p>Position: ({{ selectedEntity.data.x }}, {{ selectedEntity.data.y }})</p>
        <p v-if="selectedEntity.data.destMap">
          Destination: {{ selectedEntity.data.destMap }}
        </p>
        <p v-if="selectedEntity.data.destWarpId != null">
          Dest Warp ID: {{ selectedEntity.data.destWarpId }}
        </p>
      </div>
      <button
        v-if="selectedEntity.data.destMap"
        class="mt-2 px-3 py-1.5 bg-[#3498db] text-white border-none rounded cursor-pointer text-[11px] font-bold hover:bg-[#2980b9] w-full"
        @click="store.navigateToMap(selectedEntity!.type === 'warp' ? selectedEntity!.data.destMap! : '')"
      >
        Go to {{ selectedEntity.data.destMap }} →
      </button>
    </template>
  </div>
</template>
