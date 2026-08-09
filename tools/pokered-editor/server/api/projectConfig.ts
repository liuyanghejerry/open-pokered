import path from 'path'
import fs from 'fs'
import { fileURLToPath } from 'url'
import { setProjectRoot } from '../context/projectContext'

// pokered-editor adaptation: default to the workspace root (where the pokered
// project's .dotzuki-editor.json lives) instead of process.cwd(). Under Vite 8
// (rolldown) and vitest, `import.meta.url` is each module's real file URL — for
// this file that is pokered-editor/server/api — so the workspace root is four
// levels up. JRPG_PROJECT_ROOT still overrides. (import.meta.url rather than
// `__dirname` because the esbuild-bundled Electron api-server is pure ESM,
// where a bare `__dirname` is undefined.)
const moduleDir = path.dirname(fileURLToPath(import.meta.url))
let projectRoot = process.env.JRPG_PROJECT_ROOT || path.resolve(moduleDir, '../../../..')

export interface ProjectConfig {
  name: string
  dataRoot: string
  gfxRoot?: string
  activities: ActivityDef[]
}

export interface ActivityDef {
  id: string
  type: 'map' | 'script' | 'data' | 'assets' | 'story' | 'ui' | 'audio'
  config: Record<string, unknown>
  enabled?: boolean
}

export interface TableDef {
  id: string
  dir: string
}

let cachedConfig: ProjectConfig | null = null

export function getProjectRoot(): string {
  return projectRoot
}

export function setProjectRootDir(dir: string): void {
  projectRoot = dir
  cachedConfig = null
  setProjectRoot(dir)
}

export function resetConfigCache(): void {
  cachedConfig = null
}

export function configFile() { return path.join(projectRoot, '.dotzuki-editor.json') }

export function loadConfig(): ProjectConfig {
  if (cachedConfig) return cachedConfig
    if (!fs.existsSync(configFile())) {
    throw new Error(`No .dotzuki-editor.json found in ${projectRoot}. Run 'dotzuki-editor init' first.`)
  }
  cachedConfig = JSON.parse(fs.readFileSync(configFile(), 'utf-8'))
  return cachedConfig!
}

export function resolveDataPath(relative: string): string {
  const cfg = loadConfig()
  return path.resolve(projectRoot, cfg.dataRoot, relative)
}

export function resolveGfxPath(relative: string): string {
  const cfg = loadConfig()
  return path.resolve(projectRoot, cfg.gfxRoot ?? 'gfx', relative)
}
