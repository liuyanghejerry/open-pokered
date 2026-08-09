import { defineConfig } from 'vitest/config'
import path from 'path'

// Isolated test config (vitest prefers this over vite.config.ts, so the dev
// server middleware plugins are never loaded during tests). The ported
// jrpg-editor server suite (actions, sprite-sheet pipeline, …) is pure Node.
export default defineConfig({
  resolve: {
    // Mirror vite.config.ts so client modules under test resolve '@' imports.
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  test: {
    include: ['server/**/*.test.ts', 'src/**/*.test.ts'],
    environment: 'node',
  },
})
