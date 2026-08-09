// ───────────────────────────────────────────────────────────────────────────
// Project create/scaffold tests — drive the real route handlers through a
// minimal mock of the connect `server.middlewares.use` surface, with the
// project root pinned to a fresh temp dir per test (setProjectRootDir, not
// DOTZUKI_PROJECT_ROOT, which projectConfig reads only once at module load).
// ───────────────────────────────────────────────────────────────────────────
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import fs from 'fs'
import os from 'os'
import path from 'path'
import { Readable } from 'stream'
import { registerProject } from './project'
import { getProjectRoot, setProjectRootDir } from '../projectConfig'

type Handler = (req: any, res: any) => unknown

function makeServer() {
  const routes = new Map<string, Handler>()
  return {
    routes,
    middlewares: { use(route: string, fn: Handler) { routes.set(route, fn) } },
  }
}

function mockReq(method: string, body?: unknown, url = '/') {
  const req = new Readable({ read() {} }) as any
  req.method = method
  req.url = url
  req.headers = { host: 'localhost' }
  if (body !== undefined) req.push(JSON.stringify(body))
  req.push(null)
  return req
}

function mockRes() {
  const res: any = {
    status: 0,
    body: '',
    writeHead(status: number) { res.status = status },
    end(chunk?: string) { res.body = chunk ?? '' },
    json() { return JSON.parse(res.body) },
  }
  return res
}

async function call(routes: Map<string, Handler>, route: string, req: any) {
  const handler = routes.get(route)!
  const res = mockRes()
  await handler(req, res)
  return res
}

let ROOT = ''

beforeEach(() => {
  ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'jrpg-create-'))
  setProjectRootDir(ROOT)
})

afterEach(() => {
  try { fs.rmSync(ROOT, { recursive: true, force: true }) } catch { /* ignore */ }
})

async function createProject(body: unknown) {
  const server = makeServer()
  registerProject(server)
  return call(server.routes, '/api/project/create', mockReq('POST', body))
}

function readConfig(target: string) {
  return JSON.parse(fs.readFileSync(path.join(target, '.dotzuki-editor.json'), 'utf-8'))
}

describe('POST /api/project/create', () => {
  it.each(['empty', 'wuxia', 'jrpg'])('scaffolds the %s template by id', async (id) => {
    const res = await createProject({ name: 'My Game', template: id, dir: `game-${id}` })
    expect(res.status).toBe(200)
    const target = path.join(ROOT, `game-${id}`)
    // Creating switches the editor's project root to the new project.
    expect(getProjectRoot()).toBe(target)

    const cfg = readConfig(target)
    expect(cfg.name).toBe('My Game')
    // Scripts use the .scene DSL; the tiles activity backs the map editor.
    const script = cfg.activities.find((a: any) => a.type === 'script')
    expect(script.config.extension).toBe('.scene')
    const tiles = cfg.activities.find((a: any) => a.type === 'tiles')
    expect(tiles.config.tilesDir).toBe('tiles')

    // Starter content, and NO Rust scaffolding.
    expect(fs.existsSync(path.join(target, 'assets', 'scenes', 'main.scene'))).toBe(true)
    expect(fs.existsSync(path.join(target, 'README.md'))).toBe(true)
    expect(fs.existsSync(path.join(target, 'Cargo.toml'))).toBe(false)
    expect(fs.existsSync(path.join(target, 'src', 'main.rs'))).toBe(false)
    expect(fs.existsSync(path.join(target, 'data', 'maps'))).toBe(true)
    expect(fs.existsSync(path.join(target, 'gfx'))).toBe(true)
  })

  it('wuxia template seeds its data tables', async () => {
    const res = await createProject({ name: 'Wuxia', template: 'wuxia', dir: 'wuxia-game' })
    expect(res.status).toBe(200)
    const target = path.join(ROOT, 'wuxia-game')
    const cfg = readConfig(target)
    const data = cfg.activities.find((a: any) => a.type === 'data')
    expect(data.config.tables.map((t: any) => t.id)).toEqual(['characters', 'skills', 'items', 'status'])
    expect(fs.existsSync(path.join(target, 'data', 'characters'))).toBe(true)
  })

  it('rejects a template NAME (the old wizard payload) with 400', async () => {
    const res = await createProject({ name: 'My Game', template: 'Empty Project', dir: 'game-x' })
    expect(res.status).toBe(400)
    expect(res.json().error).toContain('Unknown template')
  })

  it('rejects an invalid directory name with 400', async () => {
    const res = await createProject({ name: 'My Game', template: 'empty', dir: 'Bad Dir!' })
    expect(res.status).toBe(400)
    expect(res.json().error).toContain('Invalid directory name')
  })

  it('rejects a non-empty target directory with 409', async () => {
    const target = path.join(ROOT, 'occupied')
    fs.mkdirSync(target, { recursive: true })
    fs.writeFileSync(path.join(target, 'keep.txt'), 'hi')
    const res = await createProject({ name: 'My Game', template: 'empty', dir: 'occupied' })
    expect(res.status).toBe(409)
    // The existing directory is left untouched.
    expect(fs.existsSync(path.join(target, '.dotzuki-editor.json'))).toBe(false)
    expect(fs.existsSync(path.join(target, 'keep.txt'))).toBe(true)
  })

  it('scaffolds into an existing EMPTY directory', async () => {
    fs.mkdirSync(path.join(ROOT, 'empty-dir'), { recursive: true })
    const res = await createProject({ name: 'My Game', template: 'empty', dir: 'empty-dir' })
    expect(res.status).toBe(200)
    expect(fs.existsSync(path.join(ROOT, 'empty-dir', '.dotzuki-editor.json'))).toBe(true)
  })

  it('accepts an absolute directory path (Electron folder picker)', async () => {
    const target = path.join(ROOT, 'abs-game')
    const res = await createProject({ name: 'Abs Game', template: 'empty', dir: target })
    expect(res.status).toBe(200)
    expect(fs.existsSync(path.join(target, '.dotzuki-editor.json'))).toBe(true)
    expect(getProjectRoot()).toBe(target)
  })

  it('derives the directory from the game name when dir is omitted', async () => {
    const res = await createProject({ name: 'My Cool Game', template: 'empty' })
    expect(res.status).toBe(200)
    expect(fs.existsSync(path.join(ROOT, 'my-cool-game', '.dotzuki-editor.json'))).toBe(true)
  })
})

describe('GET /api/project/templates', () => {
  it('localizes name/description via ?lang= and falls back to English', async () => {
    const server = makeServer()
    registerProject(server)

    const zh = await call(server.routes, '/api/project/templates', mockReq('GET', undefined, '/?lang=zh'))
    const zhList = zh.json()
    expect(zhList.map((t: any) => t.id)).toEqual(['empty', 'wuxia', 'jrpg'])
    expect(zhList[0].name).toBe('空白项目')

    const en = await call(server.routes, '/api/project/templates', mockReq('GET'))
    expect(en.json()[0].name).toBe('Empty Project')

    const fr = await call(server.routes, '/api/project/templates', mockReq('GET', undefined, '/?lang=fr'))
    expect(fr.json()[0].name).toBe('Empty Project')
  })
})

describe('GET /api/project/root', () => {
  it('always reports the current project root', async () => {
    const server = makeServer()
    registerProject(server)
    const res = await call(server.routes, '/api/project/root', mockReq('GET'))
    expect(res.status).toBe(200)
    expect(res.json().projectRoot).toBe(ROOT)
  })
})
