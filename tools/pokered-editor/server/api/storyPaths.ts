// @ts-nocheck -- extracted from vite.config.ts; loose dev-server types preserved
import path from 'path'
import fs from 'fs'
import os from 'os'
import { loadConfig, resolveDataPath, resolveGfxPath, getProjectRoot } from './projectConfig'
import { DEFAULT_SPRITE_CATEGORIES } from './util'

export function storyActivityConfig(): any {
  const cfg = loadConfig()
  const act = cfg.activities.find(a => a.type === 'story')
  if (!act) throw new Error('No story activity configured')
  return act.config
}
export function storiesRoot(): string {
  return resolveDataPath(storyActivityConfig().storiesDir)
}
/**
 * AI provider profile files live in the project root; with NO project open
 * (loadConfig throws) they fall back to a global file under the user's home,
 * so the assistant is configurable from the welcome screen too. Read and write
 * both go through these helpers — never build the path inline.
 */
function profileFile(projectFileName: string, globalFileName: string): string {
  try {
    loadConfig()
    return path.join(getProjectRoot(), projectFileName)
  } catch {
    return path.join(os.homedir(), '.jrpg-editor', globalFileName)
  }
}
export function providersFile(): string {
  return profileFile('.jrpg-editor.providers.json', 'providers.json')
}
export function imageProvidersFile(): string {
  return profileFile('.jrpg-editor.image-providers.json', 'image-providers.json')
}
export function editorSettingsFile(): string {
  return path.join(getProjectRoot(), '.jrpg-editor.settings.json')
}
/** Kebab-ASCII slug for a record id; falls back to the sanitized id (e.g. a
 *  non-ASCII display name) so the filename is always safe and deterministic. */
export function storySlug(id: string): string {
  const s = String(id).toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')
  return s || path.basename(String(id)) || 'record'
}
/**
 * Map a record id to its on-disk file. Story files are named by a kebab slug
 * while the record `id` is a display name (the cross-reference key used by
 * scenes/quests/relationships), so we resolve by MATCHING an existing record's
 * `id` — not by assuming filename === id. New records get `${slug(id)}.json`.
 * This makes save/delete idempotent (no forked duplicate file per edit).
 */
export function resolveStoryFile(dir: string, id: string): string {
  if (fs.existsSync(dir)) {
    for (const f of fs.readdirSync(dir)) {
      if (!f.endsWith('.json')) continue
      try {
        if (JSON.parse(fs.readFileSync(path.join(dir, f), 'utf-8'))?.id === id) return path.join(dir, f)
      } catch { /* skip unreadable */ }
    }
  }
  return path.join(dir, `${storySlug(id)}.json`)
}
export function readStoryRecord(kind: string, id: string): any | null {
  const file = resolveStoryFile(path.join(storiesRoot(), kind), id)
  if (!fs.existsSync(file)) return null
  try { return JSON.parse(fs.readFileSync(file, 'utf-8')) } catch { return null }
}
// ── Sprite Studio helpers ──
export function spriteCategories(): any[] {
  let sc: any = {}
  try { sc = storyActivityConfig() } catch { sc = {} }
  const cats = sc?.sprite?.categories
  return (Array.isArray(cats) && cats.length) ? cats : DEFAULT_SPRITE_CATEGORIES
}
export function spriteCategory(id: string): any | null {
  return spriteCategories().find((c: any) => c.id === id) ?? null
}
/** Resolve a gfx-relative path, refusing anything that escapes gfxRoot. */
export function resolveGfxSafe(rel: string): string {
  const base = resolveGfxPath('')
  const abs = path.resolve(base, String(rel).replace(/^\/+/, ''))
  if (abs !== base && !abs.startsWith(base + path.sep)) throw new Error('access denied')
  return abs
}
export function scanFlags(sc: any): string[] {
  const flags = new Set<string>()
  const scan = sc.flagSource?.scan
  if (scan !== null) {
    const dirRel = scan?.dir ?? sc.scenesDir ?? 'maps'
    const fns: string[] = scan?.fns ?? ['getFlag', 'setFlag']
    const recursive = scan?.recursive !== false
    const scanRoot = resolveDataPath(dirRel)
    const re = new RegExp(`(?:${fns.join('|')})\\s*\\(\\s*["']([A-Za-z_][A-Za-z0-9_]*)["']`, 'g')
    const walk = (d: string) => {
      if (!fs.existsSync(d)) return
      for (const entry of fs.readdirSync(d, { withFileTypes: true })) {
        const full = path.join(d, entry.name)
        if (entry.isDirectory()) { if (recursive) walk(full); continue }
        if (!/\.(js|ts|scene|json)$/.test(entry.name)) continue
        const text = fs.readFileSync(full, 'utf-8')
        let m: RegExpExecArray | null
        while ((m = re.exec(text)) !== null) flags.add(m[1])
      }
    }
    walk(scanRoot)
  }
  const tableId = sc.flagSource?.table
  if (tableId) {
    const cfg = loadConfig()
    const table = cfg.activities
      .flatMap((a: any) => a.type === 'data' ? a.config.tables : [])
      .find((t: any) => t.id === tableId)
    if (table) {
      const tdir = resolveDataPath(table.dir)
      const idField = table.idField ?? 'id'
      if (fs.existsSync(tdir)) {
        for (const f of fs.readdirSync(tdir).filter((x: string) => x.endsWith('.json'))) {
          try {
            const rec = JSON.parse(fs.readFileSync(path.join(tdir, f), 'utf-8'))
            if (rec[idField]) flags.add(String(rec[idField]))
          } catch { /* ignore */ }
        }
      }
    }
  }
  return [...flags].sort()
}