import { ref } from 'vue'
import type { ScriptFunction, DslBlock } from './useCodeMirror'

export const sharedScriptFunctions = ref<ScriptFunction[]>([])
export const sharedActiveFunction = ref<string | null>(null)
export const sharedDslBlocks = ref<DslBlock[]>([])
export const sharedActiveDslBlock = ref<DslBlock | null>(null)
export const sharedScriptMode = ref<'js' | 'dsl'>('js')
