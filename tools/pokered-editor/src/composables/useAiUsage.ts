// ───────────────────────────────────────────────────────────────────────────
// useAiUsage — a tiny session token meter. Every AI action emits a `usage` event
// (input/output tokens); the chat surfaces it via message metadata. This store
// accumulates them so the UI can show a running cost readout. Module singleton.
// ───────────────────────────────────────────────────────────────────────────
import { ref, computed } from 'vue'

const inputTokens = ref(0)
const outputTokens = ref(0)
const calls = ref(0)

/** Accept the AI SDK usage shape (inputTokens/outputTokens or prompt/completion). */
function record(u: any): void {
  if (!u) return
  const inp = Number(u.inputTokens ?? u.promptTokens ?? u.input_tokens ?? 0)
  const out = Number(u.outputTokens ?? u.completionTokens ?? u.output_tokens ?? 0)
  if (!inp && !out) return
  inputTokens.value += inp
  outputTokens.value += out
  calls.value += 1
}

function reset(): void { inputTokens.value = 0; outputTokens.value = 0; calls.value = 0 }

/** Compact "1.2k" formatting. */
function fmt(n: number): string {
  return n >= 1000 ? (n / 1000).toFixed(n >= 10000 ? 0 : 1) + 'k' : String(n)
}

export function useAiUsage() {
  const total = computed(() => inputTokens.value + outputTokens.value)
  const label = computed(() => `${fmt(inputTokens.value)}↑ ${fmt(outputTokens.value)}↓`)
  return { inputTokens, outputTokens, calls, total, label, record, reset }
}
