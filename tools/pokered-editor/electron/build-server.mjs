// Bundle the Electron production API server (electron/api-server.ts + all the
// server/** route modules it imports) into a single ESM file the packaged main
// process can import. node_modules are kept EXTERNAL — they're resolved at
// runtime from the app's node_modules, which sidesteps bundling ESM-only deps
// (ai, @ai-sdk/*) and keeps the bundle tiny. Types are stripped, not checked.
import { build } from 'esbuild'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

await build({
  entryPoints: [path.join(root, 'electron/api-server.ts')],
  outfile: path.join(root, 'dist-electron/api-server.mjs'),
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node20',
  packages: 'external',
  sourcemap: true,
  logLevel: 'info',
})

console.log('✓ built dist-electron/api-server.mjs')
