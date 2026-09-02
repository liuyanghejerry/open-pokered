// ───────────────────────────────────────────────────────────────────────────
// Published-game player templates — two flavors over one shared runtime.
//
// 1. renderPlayerHtml (single file, static hosting): ONE self-contained HTML
//    embedding the wasm-pack web glue, the runner binary (base64) and the
//    edit set — no server, no other files. Download-and-play.
// 2. renderWebDirPlayerHtml (web dir, backend hosting): a small index.html
//    that loads `./wasm/pokered_runner_web.js`, `./wasm/…_bg.wasm` and
//    `./data.json` from sibling files — the standard multi-file layout of
//    `dotzuki export --web`, written to disk by POST /api/publish and served
//    under /published/.
//
// The player runtime mirrors the in-editor playtest contract
// (composables/usePokeredRunner.ts + useGameSession.ts): input bitmask,
// fixed-step rAF loop, localStorage save with a per-title save key, delta
// replay. Page structure follows the engine's `dotzuki export --web` player
// (dotzuki-cli templates/web-player.html): status line, pixelated canvas,
// auto-boot on load. Unlike dotzuki's runner, pokered's runner-web drives its
// own Web Audio output internally — the context stays suspended until the
// first user gesture (autoplay policy), which the page resumes on.
// ───────────────────────────────────────────────────────────────────────────

export interface PlayerTemplateInput {
  /** Game title shown in the tab and the page header. */
  title: string
  /** Text of wasm-pack's web glue (pokered_runner_web.js), inlined verbatim. */
  runnerGlueJs: string
  /** Base64 of pokered_runner_web_bg.wasm (no data: URL prefix). */
  wasmBase64: string
  /** Edit-set JSON (exportDeltasJson() output): [{path, content}, …]. */
  editsJson: string
}

export interface WebDirPlayerTemplateInput {
  /** Game title shown in the tab and the page header. */
  title: string
}

/** Save key namespace mirrors dotzuki's `dotzuki-save:<title>` convention. */
export function publishedSaveKey(title: string): string {
  return `pokered-save:${title}`
}

/** Escape a string for safe interpolation into HTML text/attributes. */
function htmlEscape(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
}

/** Escape inline JS so the HTML parser can't end the <script> early.
 *  `<\/script` inside a JS string literal is still `</script`. */
function jsForScriptTag(s: string): string {
  return s.replace(/<\/script/gi, '<\\/script')
}

/** Escape an embedded JSON payload: `\u003c` is a valid JSON escape for `<`,
 *  so the payload stays JSON.parse-able but can never form `</script>`. */
function jsonForScriptTag(s: string): string {
  return s.replace(/</g, '\\u003c')
}

// Boot sequence up to "the runner is constructed and the edits are known" —
// the only part that differs between the two flavors. `__POKERED_BOOT__` must
// set `runner` and `gameEdits`; the shared runtime takes it from there.
const BOOT_SINGLE_FILE = String.raw`
  try {
    say('Decoding game engine…')
    const wasmB64 = document.getElementById('embedded-wasm').textContent.replace(/\s+/g, '')
    await __wbg_init(decodeBase64(wasmB64))
    say('Starting game…')
    runner = new PokeredRunner(localStorage.getItem(SAVE_KEY))
    gameEdits = JSON.parse(document.getElementById('embedded-edits').textContent)
  } catch (e) {
    say('Failed to start: ' + ((e && e.message) || String(e)))
  }
`

const BOOT_WEB_DIR = String.raw`
  try {
    say('Loading game engine…')
    const mod = await import('./wasm/pokered_runner_web.js')
    await mod.default(await (await fetch('./wasm/pokered_runner_web_bg.wasm')).arrayBuffer())
    say('Starting game…')
    runner = new mod.PokeredRunner(localStorage.getItem(SAVE_KEY))
    gameEdits = await (await fetch('./data.json')).json()
  } catch (e) {
    say('Failed to start: ' + ((e && e.message) || String(e)))
  }
`

// The flavor-agnostic player runtime. In the single-file flavor it is appended
// after the wasm-bindgen glue inside one <script type="module"> — the glue's
// module-scope bindings (`__wbg_init`, `PokeredRunner`) are directly visible.
// String.raw keeps the regex backslashes intact.
const PLAYER_RUNTIME_JS = String.raw`
const SAVE_KEY = __POKERED_SAVE_KEY__
const WIDTH = 160
const HEIGHT = 144
const STEP_MS = 1000 / 59.7275 // GB frame rate; the runner advances 1 frame per tick

const statusEl = document.getElementById('status')
const canvas = document.getElementById('screen')
const ctx = canvas.getContext('2d')

const KEY_BITS = { ArrowUp: 64, ArrowDown: 128, ArrowLeft: 32, ArrowRight: 16,
  Enter: 8, Backspace: 4, KeyW: 64, KeyS: 128, KeyA: 32, KeyD: 16,
  KeyZ: 1, KeyX: 2, Space: 8, ShiftRight: 4 }
let input = 0
let runner = null
let gameEdits = []
let muted = false
let rafId = 0
let lastTime = 0
let acc = 0

const say = (t) => { statusEl.textContent = t }

addEventListener('keydown', (e) => {
  // Modifier combos stay browser/editor shortcuts, never game input.
  if (e.ctrlKey || e.metaKey || e.altKey) return
  const b = KEY_BITS[e.key] ?? KEY_BITS[e.code]
  if (!b || !runner) return
  e.preventDefault()
  input |= b
})
addEventListener('keyup', (e) => {
  const b = KEY_BITS[e.key] ?? KEY_BITS[e.code]
  if (!b) return
  if (runner) e.preventDefault()
  input &= ~b
})

// The runner owns its Web Audio context; re-queue music on user gestures in
// case the context starts suspended.
const resumeAudio = () => { if (runner) runner.resume_audio() }
addEventListener('pointerdown', resumeAudio, { capture: true })
addEventListener('keydown', resumeAudio, { capture: true })

function decodeBase64(b64) {
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return bytes
}

// Replay the edit set into the live runner. Delta paths and fallback
// semantics are the editor's IndexedDB delta store; binary (data:-URL) deltas
// are skipped — gfx is compiled into the wasm binary and can't be overridden
// at runtime (same limitation as the in-editor playtest). map.blk entries are
// number-array JSON, the same shape /api/maps/<map>/map.blk serves.
function applyEdits() {
  const scenes = {}
  const configs = {}
  for (const d of gameEdits) {
    if (!d || typeof d.content !== 'string' || d.content.startsWith('data:')) continue
    const p = d.path || ''
    if (p.startsWith('maps/')) {
      const rest = p.slice('maps/'.length)
      const slash = rest.indexOf('/')
      if (slash < 0) continue
      const name = rest.slice(0, slash)
      const file = rest.slice(slash + 1)
      if (file === 'map.json') runner.set_map_data(name, d.content)
      else if (file === 'map.blk') runner.set_map_blk(name, d.content)
      else if (file === 'script.scene') scenes[name] = d.content
      else if (file === 'script_config.json') configs[name] = d.content
    } else if (p.startsWith('trainers/')) {
      runner.set_trainer(p.slice('trainers/'.length, -'.json'.length), d.content)
    } else if (p.startsWith('moves/')) {
      runner.set_move(p.slice('moves/'.length, -'.json'.length), d.content)
    } else if (p.startsWith('items/')) {
      runner.set_item(p.slice('items/'.length, -'.json'.length), d.content)
    } else if (p.startsWith('pokemon/')) {
      runner.set_base_stats(p.slice('pokemon/'.length, -'.json'.length), d.content)
    }
  }
  if (Object.keys(scenes).length > 0 || Object.keys(configs).length > 0) {
    runner.reload_scripts(JSON.stringify(scenes), JSON.stringify(configs))
  }
}

function persistSave() {
  if (!runner) return
  const s = runner.export_save()
  if (s) { try { localStorage.setItem(SAVE_KEY, s) } catch (e) { /* storage blocked */ } }
}

function frame(now) {
  rafId = requestAnimationFrame(frame)
  if (!runner) return
  if (!lastTime) lastTime = now
  acc += Math.min(now - lastTime, 250) // cap long tab-switch gaps
  lastTime = now
  let bytes = null
  while (acc >= STEP_MS) {
    bytes = runner.tick(input)
    acc -= STEP_MS
  }
  if (bytes) {
    ctx.putImageData(new ImageData(new Uint8ClampedArray(bytes), WIDTH, HEIGHT), 0, 0)
  }
}

function startGame() {
  runner.resume_audio()
  canvas.hidden = false
  statusEl.hidden = true
  document.getElementById('toolbar').hidden = false
  setInterval(persistSave, 2000)
  addEventListener('pagehide', persistSave)
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') persistSave()
  })
  rafId = requestAnimationFrame(frame)
}

async function boot() {
__POKERED_BOOT__
  if (runner) {
    applyEdits()
    startGame()
  }
}

// Auto-boot like the engine's own web player; the Web Audio context stays
// suspended until the first user gesture (browser autoplay policy) — the
// pointerdown/keydown listeners above resume it.
boot()

document.getElementById('btn-mute').addEventListener('click', (e) => {
  if (!runner) return
  muted = !muted
  runner.set_muted(muted)
  e.target.textContent = muted ? 'Unmute' : 'Mute'
})

// Data overrides live in wasm global state and survive a reset; the re-push
// is idempotent and also restores the script injections a reset wipes.
document.getElementById('btn-reset').addEventListener('click', () => {
  if (!runner) return
  runner.reset(localStorage.getItem(SAVE_KEY) ?? null)
  try { applyEdits() } catch (e) { /* keep the reboot going */ }
  runner.resume_audio()
})
`

/** Build the player runtime for a flavor by filling the two placeholders and
 *  escaping for inline use in a <script type="module">. */
function buildPlayerJs(flavorBoot: string, title: string): string {
  const saveKey = jsonForScriptTag(JSON.stringify(publishedSaveKey(title)))
  return jsForScriptTag(
    PLAYER_RUNTIME_JS.replace('__POKERED_SAVE_KEY__', saveKey).replace('__POKERED_BOOT__', flavorBoot.trimEnd()),
  )
}

/** Shared page chrome (head/CSS/body scaffolding around the game widgets). */
function pageShell(title: string, body: string): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title}</title>
<style>
  html,body{margin:0;min-height:100%;background:#14161a;color:#e8e8e8;
            font-family:system-ui,-apple-system,sans-serif}
  body{display:flex;flex-direction:column;align-items:center;justify-content:center;
       gap:14px;padding:24px;box-sizing:border-box}
  h1{font-size:18px;font-weight:600;margin:0}
  #status{color:#9aa0a6;font-size:14px}
  #hint{color:#5f6368;font-size:12px;text-align:center}
  canvas{image-rendering:pixelated;width:min(480px,96vw);background:#000;
         box-shadow:0 8px 40px rgba(0,0,0,.6);border-radius:4px}
  #toolbar{display:flex;gap:8px}
  #toolbar button{background:#1f232b;color:#e8e8e8;border:1px solid #333a46;
                  border-radius:4px;padding:4px 12px;font-size:12px;cursor:pointer}
  #toolbar button:hover{border-color:#5f6368}
</style>
</head>
<body>
<h1>${title}</h1>
<div id="status">Loading…</div>
<canvas id="screen" width="160" height="144" hidden></canvas>
<div id="toolbar" hidden>
  <button id="btn-mute">Mute</button>
  <button id="btn-reset">Reset</button>
</div>
<div id="hint">Arrows / WASD move · Z = A · X = B · Enter = Start · Backspace = Select · progress auto-saves · click once to enable sound</div>
${body}
</body>
</html>
`
}

/** Single-file flavor: everything embedded, works offline from file://. */
export function renderPlayerHtml(input: PlayerTemplateInput): string {
  const title = htmlEscape(input.title)
  const playerJs = buildPlayerJs(BOOT_SINGLE_FILE, input.title)
  return pageShell(
    title,
    `<script type="application/base64" id="embedded-wasm">${input.wasmBase64}</script>
<script type="application/json" id="embedded-edits">${jsonForScriptTag(input.editsJson)}</script>
<script type="module">
${jsForScriptTag(input.runnerGlueJs)}
${playerJs}
</script>
`,
  )
}

/** Web-dir flavor: loads glue/wasm/edits from sibling files (served over
 *  HTTP by the publishing backend — module import + fetch need http(s)). */
export function renderWebDirPlayerHtml(input: WebDirPlayerTemplateInput): string {
  const title = htmlEscape(input.title)
  const playerJs = buildPlayerJs(BOOT_WEB_DIR, input.title)
  return pageShell(
    title,
    `<script type="module">
${playerJs}
</script>
`,
  )
}
