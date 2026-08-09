// ──────────────────────────────────────────────────────────────────────────
// Electron main process for the Pokered Editor.
//
//   dev  (ELECTRON_DEV=1): the Vite dev server already serves the app + full
//        /api surface. We just point a window at it (ELECTRON_RENDERER_URL).
//   prod (packaged / preview): there is no Vite, so we start the bundled
//        api-server (dist-electron/api-server.mjs) — which serves dist/ AND the
//        same /api routes on one http origin — and load that URL.
//
// The renderer stays a locked-down web view (contextIsolation on, no node): it
// only ever talks to the local HTTP server, exactly like the browser build.
//
// Unlike dotzuki-editor this is a SINGLE-PROJECT editor: there is no create-a-
// project wizard. File → Open Repo Folder… switches the API (and every data
// route) to another pokered checkout via POST /api/project/open; a folder
// without .dotzuki-editor.json is simply rejected, never scaffolded.
// ──────────────────────────────────────────────────────────────────────────
const { app, BrowserWindow, ipcMain, dialog, Menu, shell } = require('electron')
const path = require('node:path')
const { pathToFileURL } = require('node:url')

// ELECTRON_PROD=1 forces the bundled-server path even when running unpackaged
// (used by `npm run electron:preview` to exercise the production flow).
const isDev =
  process.env.ELECTRON_PROD !== '1' &&
  (process.env.ELECTRON_DEV === '1' || !app.isPackaged)
const EDITOR_ROOT = path.resolve(__dirname, '..')

// The WASM layout-preview pkg lives in the repo (crates/dotzuki-web/pkg) in dev and
// preview, but a packaged app has no repo — it ships the pkg as an extraResource
// (Resources/wasm-pkg). Point the /wasm route there. Unpackaged runs leave this
// unset so the route falls back to the in-repo path.
if (app.isPackaged && !process.env.JRPG_WASM_ROOT) {
  process.env.JRPG_WASM_ROOT = path.join(process.resourcesPath, 'wasm-pkg')
}

/** @type {import('http').Server extends any ? any : never} */
let apiServer = null // { url, port, close } from the prod api-server
/** @type {string} base origin the renderer + main talk to for /api */
let apiBase = ''
/** @type {BrowserWindow | null} */
let win = null

async function startProdServer() {
  const serverPath = path.join(EDITOR_ROOT, 'dist-electron', 'api-server.mjs')
  const { startApiServer } = await import(pathToFileURL(serverPath).href)
  // pokered-editor edits one repo: default to the workspace root two levels
  // above this package (where .dotzuki-editor.json lives); JRPG_PROJECT_ROOT
  // overrides, and File → Open Repo Folder… re-roots at runtime.
  const projectRoot = process.env.JRPG_PROJECT_ROOT || path.resolve(EDITOR_ROOT, '..', '..')
  apiServer = await startApiServer({
    projectRoot,
    staticDir: path.join(EDITOR_ROOT, 'dist'),
    host: '127.0.0.1',
    // Ephemeral by default; JRPG_PORT pins it (handy for debugging/automation).
    port: Number(process.env.JRPG_PORT) || 0,
  })
  return apiServer.url
}

function createWindow() {
  win = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 900,
    minHeight: 600,
    backgroundColor: '#111827', // matches the app's dark shell
    title: 'Pokered Editor',
    autoHideMenuBar: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  })

  // Open target=_blank / external links in the system browser, not a new window.
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:/.test(url)) shell.openExternal(url)
    return { action: 'deny' }
  })

  win.loadURL(apiBase)
  if (isDev) win.webContents.openDevTools({ mode: 'detach' })
  win.on('closed', () => { win = null })
}

/** Native folder picker → switch the API's project root → reload the renderer. */
async function openProjectDialog() {
  const target = win ?? BrowserWindow.getFocusedWindow()
  const result = await dialog.showOpenDialog(target ?? undefined, {
    title: 'Open pokered repo',
    message: 'Choose the pokered workspace root (the folder containing .dotzuki-editor.json)',
    properties: ['openDirectory'],
  })
  if (result.canceled || !result.filePaths[0]) return { ok: false }
  const dir = result.filePaths[0]
  try {
    // Reuse the existing /api/project/open route so dev (Vite) and prod
    // (bundled) servers switch roots through one code path. No manifest →
    // just report the error (this editor never scaffolds a new project).
    const resp = await fetch(`${apiBase}/api/project/open`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: dir }),
    })
    const data = await resp.json().catch(() => ({}))
    if (!resp.ok) {
      await dialog.showMessageBox(target ?? undefined, {
        type: 'error',
        title: 'Could not open repo',
        message: data.error || `No .dotzuki-editor.json found in ${dir}`,
      })
      return { ok: false, error: data.error }
    }
    win?.webContents.reload()
    return { ok: true, path: dir }
  } catch (e) {
    return { ok: false, error: String(e) }
  }
}

function buildMenu() {
  const isMac = process.platform === 'darwin'
  const template = [
    ...(isMac ? [{ role: 'appMenu' }] : []),
    {
      label: 'File',
      submenu: [
        { label: 'Open Repo Folder…', accelerator: 'CmdOrCtrl+O', click: openProjectDialog },
        { type: 'separator' },
        { label: 'Reload', accelerator: 'CmdOrCtrl+R', click: () => win?.webContents.reload() },
        isMac ? { role: 'close' } : { role: 'quit' },
      ],
    },
    { role: 'editMenu' },
    {
      label: 'View',
      submenu: [
        { role: 'resetZoom' }, { role: 'zoomIn' }, { role: 'zoomOut' },
        { type: 'separator' }, { role: 'togglefullscreen' },
        { role: 'toggleDevTools' },
      ],
    },
    { role: 'windowMenu' },
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

// Single-instance: focus the existing window instead of spawning another.
if (!app.requestSingleInstanceLock()) {
  app.quit()
} else {
  app.on('second-instance', () => {
    if (win) { if (win.isMinimized()) win.restore(); win.focus() }
  })

  app.whenReady().then(async () => {
    if (isDev) {
      apiBase = process.env.ELECTRON_RENDERER_URL || 'http://localhost:5173'
    } else {
      apiBase = await startProdServer()
    }

    ipcMain.handle('pokered:openProject', openProjectDialog)
    buildMenu()
    createWindow()

    app.on('activate', () => {
      if (BrowserWindow.getAllWindows().length === 0) createWindow()
    })
  })

  app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') app.quit()
  })

  app.on('will-quit', async () => {
    if (apiServer) await apiServer.close().catch(() => {})
  })
}
