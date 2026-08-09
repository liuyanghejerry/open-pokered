<script setup lang="ts">
import { ref } from 'vue'
import { GAME_WIDTH, GAME_HEIGHT } from '../composables/useGameSession'

const props = withDefaults(
  defineProps<{
    /** CSS height for the stage (kept 160:144 aspect). */
    height?: string
  }>(),
  { height: 'min(70vh, 560px)' },
)

const emit = defineEmits<{ blur: [] }>()

/** The actual <canvas> element — consumed by useGameSession. */
const canvasEl = ref<HTMLCanvasElement | null>(null)
defineExpose({ canvasEl })
</script>

<template>
  <canvas
    ref="canvasEl"
    :width="GAME_WIDTH"
    :height="GAME_HEIGHT"
    tabindex="0"
    @blur="emit('blur')"
    class="max-w-full [image-rendering:pixelated] outline-none focus:ring-2 focus:ring-accent border border-[rgba(255,255,255,0.1)] rounded"
    :style="{ aspectRatio: `${GAME_WIDTH} / ${GAME_HEIGHT}`, height: props.height }"
  ></canvas>
</template>
