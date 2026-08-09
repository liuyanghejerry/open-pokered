// Stage the dotzuki-web WASM preview package into dist-electron/wasm-pkg so
// electron-builder can ship it as an extraResource (→ Resources/wasm-pkg),
// which the packaged app's /wasm route reads via DOTZUKI_WASM_ROOT.
//
// The pkg is built by `pnpm build:wasm` (wasm-pack) into crates/dotzuki-web/pkg.
// If it isn't there we still create the (near-empty) dest so extraResources
// has a valid source — packaging then succeeds, just without the ui-activity
// layout preview, exactly as an unbuilt dev checkout behaves today.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const jrpgSrc = path.resolve(root, '..', '..', 'crates', 'dotzuki-web', 'pkg')
// WYSIWYG game preview (pokered-runner-web). Copied into the same staging
// dir as dotzuki-web — the /wasm route falls back to it for pokered_runner_web.*
// files (see server/pokeredRoutes.ts).
const pokeredSrc = path.resolve(root, '..', '..', 'crates', 'pokered-runner-web', 'pkg')
const dest = path.join(root, 'dist-electron', 'wasm-pkg')

// Start from a clean dest each time.
fs.rmSync(dest, { recursive: true, force: true })
fs.mkdirSync(dest, { recursive: true })

let staged = 0
for (const [label, src, probe] of [
  ['WASM preview pkg', jrpgSrc, 'dotzuki_web.js'],
  ['pokered-runner-web pkg', pokeredSrc, 'pokered_runner_web.js'],
] as const) {
  if (fs.existsSync(path.join(src, probe))) {
    // Copy the files the preview actually loads (js loader + wasm binary); the
    // .d.ts/package.json come along harmlessly via a plain recursive copy.
    fs.cpSync(src, dest, { recursive: true, filter: (s) => path.basename(s) !== '.gitignore' })
    const wasm = fs.readdirSync(dest).find((f) => f.endsWith('_bg.wasm'))
    const mb = wasm ? (fs.statSync(path.join(dest, wasm)).size / 1e6).toFixed(1) : '?'
    console.log(`✓ staged ${label} → dist-electron/wasm-pkg (${mb} MB wasm)`)
    staged += 1
  } else {
    console.warn(`⚠ ${path.relative(process.cwd(), src)} not found — skipped (${label}).`)
  }
}

if (staged === 0) {
  fs.writeFileSync(
    path.join(dest, 'README.txt'),
    'WASM packages not built.\n' +
      'Run `pnpm build:wasm` (and `pnpm build:wasm-pokered`) before packaging.\n',
  )
}
