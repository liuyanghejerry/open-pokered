import { ref, type Ref } from 'vue'
import { fetchWithProgress } from '../utils/fetchWithProgress'

interface WasmModule {
  /** wasm-bindgen init — pass the fetched .wasm bytes to control download progress. */
  default(input?: BufferSource | WebAssembly.Module): Promise<void>
  render_layout(
    menu_name: string,
    layout_json: string,
    mock_state_id: number,
    lang: number,
    overrides_json: string,
  ): Uint8Array
  compile_scene(source: string): string
  compile_scene_config(source: string): string
  compile_screen_source(source: string): string
}

let wasmModule: WasmModule | null = null
let initPromise: Promise<WasmModule> | null = null

/**
 * Load + init the jrpg-web layout-preview WASM module (cached singleton;
 * failures are retryable). The `.wasm` is fetched ourselves with byte-level
 * progress (mirroring the pokered runner) so the boot loading screen can
 * cover it; wasm-bindgen's `default()` accepts the raw bytes.
 */
export function loadLayoutPreviewModule(
  onProgress?: (loaded: number, total: number) => void,
): Promise<WasmModule> {
  if (wasmModule) return Promise.resolve(wasmModule)
  if (initPromise) return initPromise
  initPromise = (async () => {
    try {
      // Vite import-analysis pre-resolves string-literal `/wasm/*` paths even
      // with @vite-ignore. Building the URL at runtime defeats the analyzer
      // so the request reaches the `serve-wasm` middleware in vite.config.ts.
      // BASE_URL keeps the path correct under sub-path Pages hosting
      // (e.g. /pokered/editor/wasm/…).
      const base = import.meta.env.BASE_URL
      const wasmJsUrl = new URL(`${base}wasm/jrpg_web.js`, window.location.origin).href
      const mod = (await import(/* @vite-ignore */ wasmJsUrl)) as unknown as WasmModule
      const wasmBytes = await fetchWithProgress(
        new URL(`${base}wasm/jrpg_web_bg.wasm`, window.location.origin).href,
        onProgress,
      )
      await mod.default(wasmBytes)
      wasmModule = mod
      return mod
    } catch (e) {
      // Allow a retry (e.g. after building the pkg) instead of caching the failure.
      initPromise = null
      throw new Error(`Failed to load the layout-preview WASM: ${(e as Error).message}`)
    }
  })()
  return initPromise
}

/** Result of compiling a `.scene` DSL source via the WASM bridge. */
export type SceneCompileResult =
  | { ok: true; output: string }
  | { ok: false; error: string; raw: string; line: number; col: number }

interface WasmCompileOk { ok: true; js?: string; config?: string }
interface WasmCompileErr { ok: false; error: string; raw?: string; line?: number; col?: number }

export interface WasmPreview {
  ready: Ref<boolean>
  error: Ref<string | null>
  render: (menuName: string, layoutJson: string, mockStateId: number, lang: number, overrides?: Record<string, string>) => Promise<Uint8Array>
  compileScene: (source: string) => Promise<SceneCompileResult>
  compileSceneConfig: (source: string) => Promise<SceneCompileResult>
  compileScreen: (source: string) => Promise<SceneCompileResult>
}

export function useWasmPreview(): WasmPreview {
  const ready = ref(false)
  const error = ref<string | null>(null)

  async function ensureInit(): Promise<void> {
    try {
      await loadLayoutPreviewModule()
      ready.value = true
    } catch (e) {
      error.value = `Failed to load wasm: ${(e as Error).message}`
      throw e
    }
  }

  async function render(menuName: string, layoutJson: string, mockStateId: number, lang: number, overrides?: Record<string, string>): Promise<Uint8Array> {
    await ensureInit()
    const overridesJson = overrides ? JSON.stringify(overrides) : ''
    return wasmModule!.render_layout(menuName, layoutJson, mockStateId, lang, overridesJson)
  }

  // The Rust bridge returns a JSON string (success: { ok, js|config },
  // failure: { ok:false, error, raw, line, col }). Normalize both shapes into
  // a single discriminated union for callers.
  function normalizeCompile(json: string, field: 'js' | 'config'): SceneCompileResult {
    let parsed: WasmCompileOk | WasmCompileErr
    try {
      parsed = JSON.parse(json)
    } catch (e) {
      return { ok: false, error: `bad compiler response: ${(e as Error).message}`, raw: json, line: 1, col: 1 }
    }
    if (parsed.ok) {
      return { ok: true, output: (parsed[field] ?? '') as string }
    }
    return {
      ok: false,
      error: parsed.error ?? 'unknown compile error',
      raw: parsed.raw ?? parsed.error ?? '',
      line: parsed.line ?? 1,
      col: parsed.col ?? 1,
    }
  }

  async function compileScene(source: string): Promise<SceneCompileResult> {
    await ensureInit()
    return normalizeCompile(wasmModule!.compile_scene(source), 'js')
  }

  async function compileSceneConfig(source: string): Promise<SceneCompileResult> {
    await ensureInit()
    return normalizeCompile(wasmModule!.compile_scene_config(source), 'config')
  }

  async function compileScreen(source: string): Promise<SceneCompileResult> {
    await ensureInit()
    return normalizeCompile(wasmModule!.compile_screen_source(source), 'js')
  }

  return { ready, error, render, compileScene, compileSceneConfig, compileScreen }
}
