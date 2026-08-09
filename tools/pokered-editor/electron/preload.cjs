// Preload — the only bridge between the locked-down renderer and the main
// process. Exposes a tiny, explicit surface under window.pokeredDesktop; the
// app works without it (plain browser build), so every field is optional
// there. pokered-editor is a single-project editor, so the only bridge is the
// Open Repo Folder… flow (no new-project wizard, no directory picker).
const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('pokeredDesktop', {
  isElectron: true,
  platform: process.platform,
  /** Native folder picker → open a repo root; resolves { ok, path?, error? }. */
  openProject: () => ipcRenderer.invoke('pokered:openProject'),
})
