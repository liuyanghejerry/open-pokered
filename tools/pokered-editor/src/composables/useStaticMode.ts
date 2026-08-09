import { ref } from 'vue'

// ───────────────────────────────────────────────────────────────────────────
// Shared static-mode flag. Set once at boot (App.vue initialize) by probing
// the /api backend; components read it to disable features that need the
// local dev backend (new map / new tileset creation, AI assistant, sprite
// generation) instead of letting them fail with raw errors on GitHub Pages.
// ───────────────────────────────────────────────────────────────────────────

export const staticMode = ref(false)

/** Read the shared flag: `const staticMode = useStaticMode()` */
export function useStaticMode() {
  return staticMode
}
