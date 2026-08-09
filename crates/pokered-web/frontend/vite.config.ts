import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'
import { fileURLToPath, URL } from 'node:url'

// https://vitejs.dev/config/
export default defineConfig({
  // For GitHub Pages deployment, set VITE_BASE_PATH=/<repo-name>/
  // For local dev or root-domain hosting, leave unset (defaults to '/')
  base: process.env.VITE_BASE_PATH || '/',

  plugins: [
    wasm(),
    topLevelAwait(),
    vue(),
  ],

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@wasm': fileURLToPath(new URL('./src/wasm', import.meta.url)),
    },
  },

  server: {
    port: 8080,
    // Required headers for SharedArrayBuffer / WASM threads support
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },

  preview: {
    port: 8080,
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },

  build: {
    target: 'es2020',
    outDir: 'dist',
    // Ensure WASM files are not inlined — they must be served as separate assets
    assetsInlineLimit: 0,
  },
})
