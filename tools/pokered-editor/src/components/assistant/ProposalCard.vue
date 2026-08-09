<script setup lang="ts">
import { computed, ref } from 'vue'
import { diffHunks, type DiffOp, type AssistantProposal } from '../../composables/useProposals'

const props = defineProps<{ proposal: AssistantProposal }>()
const emit = defineEmits<{ apply: []; applySubset: [accepted: number[]]; forceApply: []; discard: [] ; revert: [] }>()

const expanded = ref(false)
const stats = computed(() => ({
  add: props.proposal.diff.filter(o => o.type === 'add').length,
  del: props.proposal.diff.filter(o => o.type === 'del').length,
}))

// project-scaffold proposals carry a structured payload, not a file diff —
// render a readable summary (name / dir / template / activities) instead.
const scaffold = computed(() => {
  if (props.proposal.target?.kind !== 'project-scaffold') return null
  try { return JSON.parse(props.proposal.after) } catch { return null }
})
const scaffoldActivities = computed(() =>
  (Array.isArray(scaffold.value?.activities) ? scaffold.value.activities : [])
    .map((a: any) => a?.label ?? a?.id)
    .filter(Boolean)
    .join(', '),
)

// ── hunk-grouped render rows + per-hunk selection ─────────────────────────────
interface Row { kind: 'ctx' | 'hunk'; ops: DiffOp[]; hunk: number }
const hunkCount = computed(() => diffHunks(props.proposal.diff).length)
const rows = computed<Row[]>(() => {
  const out: Row[] = []
  let hunk = -1
  let cur: Row | null = null
  for (const op of props.proposal.diff) {
    if (op.type === 'ctx') { cur = null; out.push({ kind: 'ctx', ops: [op], hunk: -1 }) }
    else {
      if (!cur) { hunk++; cur = { kind: 'hunk', ops: [], hunk }; out.push(cur) }
      cur.ops.push(op)
    }
  }
  return out
})
const previewRows = computed(() => (expanded.value ? rows.value : rows.value.slice(0, 12)))
const truncated = computed(() => !expanded.value && rows.value.length > 12)

// Selecting hunks is only meaningful for a pending multi-hunk edit.
const selectable = computed(() => props.proposal.status === 'pending' && hunkCount.value > 1)
const accepted = ref<Set<number>>(new Set())
// default: all hunks selected
function resetAccepted() { accepted.value = new Set(Array.from({ length: hunkCount.value }, (_, i) => i)) }
resetAccepted()
function toggle(h: number) {
  const s = new Set(accepted.value)
  s.has(h) ? s.delete(h) : s.add(h)
  accepted.value = s
}
const partial = computed(() => selectable.value && accepted.value.size < hunkCount.value)

function onApply() {
  if (partial.value) emit('applySubset', [...accepted.value])
  else emit('apply')
}
</script>

<template>
  <div class="border border-gray-700 rounded bg-gray-800/60">
    <div class="flex items-start gap-2 px-2.5 py-2">
      <span
        class="mt-0.5 text-[9px] uppercase tracking-wide px-1 rounded shrink-0"
        :class="proposal.target.kind === 'story' ? 'bg-indigo-900 text-indigo-300'
          : proposal.target.kind === 'data' ? 'bg-emerald-900 text-emerald-300'
          : proposal.target.kind === 'scene' ? 'bg-amber-900 text-amber-300'
          : proposal.target.kind === 'project-config' ? 'bg-purple-900 text-purple-300'
          : proposal.target.kind === 'project-scaffold' ? 'bg-blue-900 text-blue-300'
          : proposal.target.kind === 'map-create' ? 'bg-teal-900 text-teal-300'
          : 'bg-sky-900 text-sky-300'"
      >{{ proposal.target.kind }}</span>
      <div class="min-w-0 flex-1">
        <div class="text-xs text-gray-100 font-medium truncate">{{ proposal.title }}</div>
        <div class="text-[10px] text-gray-500 truncate">{{ proposal.target.path }}</div>
      </div>
      <span class="text-[10px] shrink-0"
        :class="proposal.status === 'applied' ? 'text-emerald-400'
          : proposal.status === 'reverted' ? 'text-gray-500'
          : proposal.status === 'failed' ? 'text-red-400'
          : proposal.status === 'conflict' ? 'text-amber-400' : 'text-gray-500'">
        {{ proposal.status === 'applied' ? 'Applied'
          : proposal.status === 'reverted' ? 'Reverted'
          : proposal.status === 'failed' ? 'Apply failed'
          : proposal.status === 'conflict' ? 'Changed' : '' }}
        <template v-if="proposal.status === 'pending'">
          <span class="text-emerald-500">+{{ stats.add }}</span>
          <span v-if="stats.del" class="text-red-500 ml-1">-{{ stats.del }}</span>
        </template>
      </span>
    </div>

    <p v-if="proposal.rationale" class="px-2.5 pb-1.5 text-[10px] text-gray-400 leading-snug">{{ proposal.rationale }}</p>

    <div v-if="selectable" class="px-2.5 pb-1 text-[10px] text-gray-500">Untick a hunk to leave it out; Apply writes only the ticked ones.</div>

    <!-- structured summary for a project-scaffold draft (no file diff) -->
    <div v-if="scaffold" class="mx-2.5 mb-2 rounded bg-gray-900 px-2.5 py-2 space-y-1 text-[11px]">
      <div class="flex gap-2"><span class="text-gray-500 w-16 shrink-0">Name</span><span class="text-gray-100">{{ scaffold.name }}</span></div>
      <div class="flex gap-2"><span class="text-gray-500 w-16 shrink-0">Directory</span><span class="text-gray-300 font-mono text-[10px] break-all">{{ scaffold.dir }}</span></div>
      <div class="flex gap-2"><span class="text-gray-500 w-16 shrink-0">Template</span><span class="text-gray-300">{{ scaffold.templateId }}</span></div>
      <div v-if="scaffoldActivities" class="flex gap-2"><span class="text-gray-500 w-16 shrink-0">Creates</span><span class="text-gray-400">{{ scaffoldActivities }}</span></div>
    </div>

    <div v-else class="mx-2.5 mb-2 max-h-56 overflow-auto rounded bg-gray-900 text-[10px] leading-[1.35] font-mono">
      <template v-for="(row, ri) in previewRows" :key="ri">
        <div v-if="row.kind === 'ctx'" class="px-2 whitespace-pre-wrap break-all text-gray-500">{{ '  ' + row.ops[0].text }}</div>
        <div v-else class="flex items-start" :class="selectable && !accepted.has(row.hunk) ? 'opacity-40' : ''">
          <input v-if="selectable" type="checkbox" :checked="accepted.has(row.hunk)" @change="toggle(row.hunk)"
            class="mt-1 ml-1 mr-0.5 shrink-0 accent-emerald-500" title="Include this change" />
          <div class="min-w-0 flex-1">
            <div v-for="(op, oi) in row.ops" :key="oi" class="px-2 whitespace-pre-wrap break-all"
              :class="op.type === 'add' ? 'bg-emerald-950/60 text-emerald-300' : 'bg-red-950/60 text-red-300'"
            >{{ (op.type === 'add' ? '+ ' : '- ') + op.text }}</div>
          </div>
        </div>
      </template>
    </div>

    <button v-if="truncated && !scaffold" @click="expanded = true" class="mx-2.5 mb-2 text-[10px] text-blue-400 hover:text-blue-300">
      ⌄ {{ rows.length - 12 }} more
    </button>

    <!-- stale-proposal guard: the file drifted since this diff was built -->
    <p v-if="proposal.status === 'conflict'" class="px-2.5 pb-1.5 text-[10px] text-amber-400/90 leading-snug">
      This file changed since the assistant proposed this edit. Applying will overwrite those changes.
    </p>

    <div class="flex items-center justify-end gap-1.5 px-2.5 pb-2">
      <span v-if="proposal.error" class="mr-auto text-[10px] text-red-400 truncate">{{ proposal.error }}</span>
      <template v-if="proposal.status === 'pending' || proposal.status === 'failed'">
        <button @click="emit('discard')" class="px-2 py-0.5 text-[11px] rounded text-gray-400 hover:text-gray-200">Discard</button>
        <button @click="onApply" :disabled="selectable && accepted.size === 0"
          class="px-2.5 py-0.5 text-[11px] rounded bg-emerald-700 text-white hover:bg-emerald-600 disabled:opacity-40">
          {{ partial ? 'Apply (' + accepted.size + '/' + hunkCount + ')' : 'Apply' }}
        </button>
      </template>
      <template v-else-if="proposal.status === 'conflict'">
        <button @click="emit('discard')" class="px-2 py-0.5 text-[11px] rounded text-gray-400 hover:text-gray-200">Discard</button>
        <button @click="emit('forceApply')" class="px-2.5 py-0.5 text-[11px] rounded bg-amber-700 text-white hover:bg-amber-600">Apply anyway</button>
      </template>
      <button v-else-if="proposal.status === 'applied'" @click="emit('revert')" class="px-2 py-0.5 text-[11px] rounded text-amber-400 hover:text-amber-300">Revert</button>
    </div>
  </div>
</template>
