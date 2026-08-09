<script setup lang="ts">
import { useStaticMode } from '../composables/useStaticMode'

type Activity = 'map' | 'script' | 'save' | 'trainer' | 'pokemon' | 'move' | 'layout' | 'pixel' | 'playtest'

withDefaults(defineProps<{
  active: Activity
  assistantOpen?: boolean
  playtestOpen?: boolean
}>(), {
  assistantOpen: false,
  playtestOpen: false,
})

const emit = defineEmits<{
  select: [value: Activity]
  toggleAssistant: []
}>()

const staticMode = useStaticMode()

const items: { id: Activity; icon: string; label: string }[] = [
  { id: 'map', icon: '🗺', label: 'Map Editor' },
  { id: 'script', icon: '{}', label: 'Script Editor' },
  { id: 'save', icon: '💾', label: 'Save Editor' },
  { id: 'trainer', icon: '🎓', label: 'Trainer Editor' },
  { id: 'pokemon', icon: '🐾', label: 'Pokemon Editor' },
  { id: 'move', icon: '⚔', label: 'Move Editor' },
  { id: 'layout', icon: '🎨', label: 'Layout Editor' },
  { id: 'pixel', icon: '🖼', label: 'Pixel Editor' },
  { id: 'playtest', icon: '🎮', label: 'Playtest' },
]
</script>

<template>
  <div class="activity-bar">
    <button
      v-for="item in items"
      :key="item.id"
      class="activity-btn"
      :class="{ active: active === item.id || (item.id === 'playtest' && playtestOpen) }"
      :title="item.id === 'playtest' ? (playtestOpen ? 'Close the floating playtest' : 'Open the floating playtest') : item.label"
      @click="emit('select', item.id)"
    >
      <span class="activity-icon">{{ item.icon }}</span>
    </button>

    <!-- AI Assistant dock toggle (pinned to the bottom). Works in static mode
         too: the chat runs browser-direct against the configured provider
         (e.g. DeepSeek), so it's never disabled — only key/provider setup is
         needed. Sprite generation still requires the local backend. -->
    <button
      class="activity-btn assistant-btn"
      :class="{ active: assistantOpen }"
      :title="staticMode
        ? 'AI Assistant (browser-direct — set a provider & API key in the assistant settings)'
        : 'AI Assistant'"
      @click="emit('toggleAssistant')"
    >
      <span class="activity-icon">✨</span>
    </button>
  </div>
</template>

<style scoped>
.activity-bar {
  width: 48px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  background: var(--color-bg-inset);
  border-right: 1px solid rgba(255, 255, 255, 0.06);
  padding-top: 4px;
}

.activity-btn {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  cursor: pointer;
  position: relative;
  color: var(--color-text-muted);
  border-left: 2px solid transparent;
  transition: color 0.15s, background 0.15s, border-color 0.15s;
}

.activity-btn:hover {
  color: var(--color-text);
  background: rgba(255, 255, 255, 0.04);
}

.activity-btn.active {
  color: var(--color-accent);
  border-left-color: var(--color-accent);
  background: rgba(78, 204, 163, 0.06);
}

.activity-icon {
  font-family: monospace;
  font-size: 14px;
  line-height: 1;
}

.assistant-btn {
  margin-top: auto;
  margin-bottom: 4px;
}

.activity-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}
</style>
