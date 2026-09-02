import { describe, expect, it } from 'vitest'
import { transformSync } from 'esbuild'
import {
  publishedSaveKey,
  renderPlayerHtml,
  renderWebDirPlayerHtml,
} from './playerTemplate'
import { slugifyTitle } from './publish'

// Minimal structurally-valid wasm module (magic + version header only) —
// enough to prove the embedded base64 survives the template round-trip.
const WASM_MAGIC = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00])

function render(overrides: Partial<Parameters<typeof renderPlayerHtml>[0]> = {}) {
  return renderPlayerHtml({
    title: 'My Hack',
    runnerGlueJs: 'export class PokeredRunner {}\nexport default function __wbg_init() {}',
    wasmBase64: btoa(String.fromCharCode(...WASM_MAGIC)),
    editsJson: '[]',
    ...overrides,
  })
}

function extract(html: string, pattern: RegExp): string {
  const m = pattern.exec(html)
  expect(m, `template section ${pattern} not found`).not.toBeNull()
  return m![1]
}

describe('renderPlayerHtml', () => {
  it('embeds title, save key, edits and wasm payload', () => {
    const html = render({ title: 'My <Hack>', editsJson: '[{"path":"maps/A/map.json","content":"{}"}]' })
    expect(html).toContain('<title>My &lt;Hack&gt;</title>')
    // `<` is JSON-escaped inside the payload string too (\u003c) — JSON.parse
    // in the player restores it, so the save key is still "pokered-save:My <Hack>".
    expect(html).toContain('"pokered-save:My \\u003cHack>"')
    expect(html).toContain('\\u003c')
    expect(html).toContain('{"path":"maps/A/map.json","content":"{}"}')
    expect(html).toContain(btoa(String.fromCharCode(...WASM_MAGIC)))
  })

  it('never lets an edits payload terminate the script early', () => {
    const evil = JSON.stringify([
      { path: 'maps/A/map.json', content: '</script><script>alert(1)</script>' },
    ])
    const html = render({ editsJson: evil })
    // The payload's `<` became \u003c (valid JSON escape, inert in HTML)…
    expect(html).toContain('\\u003c/script>\\u003cscript>alert(1)')
    // …and the document has exactly the three closers the template emits.
    expect(html.match(/<\/script>/gi)).toHaveLength(3)
  })

  it('escapes a </script> sequence inside the inlined wasm-bindgen glue', () => {
    const html = render({ runnerGlueJs: 'const s = "</script>";\nexport class PokeredRunner {}' })
    expect(html).toContain('<\\/script')
    expect(html.match(/<\/script>/gi)).toHaveLength(3)
  })

  it('round-trips the embedded wasm bytes', () => {
    const html = render()
    const b64 = extract(html, /<script type="application\/base64" id="embedded-wasm">([\s\S]*?)<\/script>/)
    const bin = atob(b64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    expect(WebAssembly.validate(bytes)).toBe(true)
  })

  it('emits a syntactically valid module script (glue + player runtime)', () => {
    const html = render()
    const module = extract(html, /<script type="module">([\s\S]*?)<\/script>/)
    expect(module).toContain('class PokeredRunner') // from the glue stub
    expect(module).toContain('new PokeredRunner') // from the player runtime
    // esbuild throws on syntax errors; transformSync failing fails the test.
    transformSync(module, { loader: 'js', format: 'esm' })
  })
})

describe('renderWebDirPlayerHtml', () => {
  it('loads glue, wasm and data.json from sibling files', () => {
    const html = renderWebDirPlayerHtml({ title: 'My Hack' })
    expect(html).toContain('<title>My Hack</title>')
    expect(html).toContain("import('./wasm/pokered_runner_web.js')")
    expect(html).toContain("fetch('./wasm/pokered_runner_web_bg.wasm')")
    expect(html).toContain("fetch('./data.json')")
    expect(html).toContain('"pokered-save:My Hack"')
    // No embedded payloads in the web-dir flavor.
    expect(html).not.toContain('embedded-wasm')
    expect(html).not.toContain('embedded-edits')
  })

  it('emits a syntactically valid module script', () => {
    const html = renderWebDirPlayerHtml({ title: 'My Hack' })
    const module = extract(html, /<script type="module">([\s\S]*?)<\/script>/)
    expect(module).toContain('new mod.PokeredRunner')
    transformSync(module, { loader: 'js', format: 'esm' })
  })
})

describe('slugifyTitle', () => {
  it('strips accents and punctuation', () => {
    expect(slugifyTitle('Pokémon Red')).toBe('pokemon-red')
    expect(slugifyTitle('  My Hack!! ')).toBe('my-hack')
  })

  it('falls back when nothing survives', () => {
    expect(slugifyTitle('!!!')).toBe('pokemon-red')
  })
})

describe('publishedSaveKey', () => {
  it('namespaces by title like dotzuki export does', () => {
    expect(publishedSaveKey('My Hack')).toBe('pokered-save:My Hack')
  })
})
