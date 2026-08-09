<script setup lang="ts">
import { ref } from 'vue'

const props = defineProps<{ providerId: string }>()
const emit = defineEmits<{ submit: [key: string, remember: boolean]; cancel: [] }>()

const key = ref('')
const remember = ref(true)

function submit() {
  if (!key.value.trim()) return
  emit('submit', key.value.trim(), remember.value)
}
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" @click.self="emit('cancel')">
    <div class="w-96 bg-gray-800 border border-gray-700 rounded-lg shadow-xl p-5">
      <h3 class="text-sm font-bold text-blue-400 mb-1">API key for "{{ props.providerId }}"</h3>
      <p class="text-[11px] text-gray-400 mb-3 leading-snug">
        Entered keys are used only for this request and (if remembered) stored in your browser's localStorage. They are never written to the project or sent anywhere but your chosen vendor.
      </p>
      <input
        v-model="key"
        type="password"
        placeholder="sk-..."
        autofocus
        @keydown.enter="submit"
        class="w-full bg-gray-900 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100 focus:border-blue-500 focus:outline-none"
      />
      <label class="flex items-center gap-2 mt-3 text-xs text-gray-300">
        <input v-model="remember" type="checkbox" class="accent-blue-500" />
        Remember this key in my browser
      </label>
      <div class="flex justify-end gap-2 mt-4">
        <button @click="emit('cancel')" class="px-3 py-1 text-xs rounded text-gray-400 hover:text-gray-200">
          Cancel
        </button>
        <button
          @click="submit"
          :disabled="!key.trim()"
          class="px-3 py-1 text-xs rounded bg-blue-600 text-white hover:bg-blue-500 disabled:opacity-40"
        >
          Use key
        </button>
      </div>
    </div>
  </div>
</template>
