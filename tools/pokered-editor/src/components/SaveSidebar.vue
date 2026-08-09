<script setup lang="ts">
export type SaveSection = 'info' | 'party' | 'flags' | 'items'

defineProps<{
  activeSection: SaveSection
}>()

const emit = defineEmits<{
  select: [value: SaveSection]
}>()

const sections: { id: SaveSection; icon: string; label: string }[] = [
  { id: 'info', icon: 'ℹ', label: 'Player Info' },
  { id: 'party', icon: '⚔', label: 'Party' },
  { id: 'flags', icon: '🏳', label: 'Flags' },
  { id: 'items', icon: '🎒', label: 'Items' },
]
</script>

<template>
  <div class="p-4 overflow-y-auto">
    <h2 class="text-accent text-base font-bold mb-4">Save Editor</h2>

    <nav class="space-y-0.5">
      <button
        v-for="sec in sections"
        :key="sec.id"
        class="save-nav-btn"
        :class="{ active: activeSection === sec.id }"
        @click="emit('select', sec.id)"
      >
        <span class="save-nav-icon">{{ sec.icon }}</span>
        <span class="save-nav-label">{{ sec.label }}</span>
      </button>
    </nav>
  </div>
</template>

<style scoped>
.save-nav-btn {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  font-size: 12px;
  font-weight: 500;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  border-radius: 4px;
  text-align: left;
  transition: background 0.15s, color 0.15s;
}

.save-nav-btn:hover {
  background: rgba(255, 255, 255, 0.04);
  color: var(--color-text);
}

.save-nav-btn.active {
  background: rgba(78, 204, 163, 0.08);
  color: var(--color-accent);
}

.save-nav-icon {
  font-size: 14px;
  width: 20px;
  text-align: center;
  flex-shrink: 0;
}

.save-nav-label {
  white-space: nowrap;
}
</style>
