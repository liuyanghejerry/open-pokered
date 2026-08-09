// ───────────────────────────────────────────────────────────────────────────
// IndexedDB delta store for the static-hosting mode.
//
// On GitHub Pages there is no /api backend: project files can't be read from
// or written to the repo. Instead the editor keeps the *baseline* data as
// static files bundled at deploy time and stores every user *modification*
// as a delta here (key = project-relative path, value = file content —
// `string` for text files, `Blob` for binary assets like edited sprites).
// Reads prefer a delta, then fall back to the static baseline; writes only
// touch this store. Export/import of the whole delta set lets a user move
// edits back into the repo.
// ───────────────────────────────────────────────────────────────────────────

const DB_NAME = 'pokered-editor-deltas'
const DB_VERSION = 1
const STORE = 'deltas'

export interface FileDelta {
  path: string
  content: string | Blob
}

let dbPromise: Promise<IDBDatabase> | null = null

function openDb(): Promise<IDBDatabase> {
  if (dbPromise) return dbPromise
  dbPromise = new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE)) {
        db.createObjectStore(STORE, { keyPath: 'path' })
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
  return dbPromise
}

function tx(mode: IDBTransactionMode): Promise<IDBObjectStore> {
  return openDb().then((db) => db.transaction(STORE, mode).objectStore(STORE))
}

/** Read a raw delta entry; `null` when none exists. */
export async function getDeltaEntry(path: string): Promise<FileDelta | null> {
  const store = await tx('readonly')
  return new Promise((resolve, reject) => {
    const req = store.get(path)
    req.onsuccess = () => resolve((req.result as FileDelta | undefined) ?? null)
    req.onerror = () => reject(req.error)
  })
}

/** Read a text delta; `null` when none exists (caller falls back to baseline). */
export async function getDelta(path: string): Promise<string | null> {
  const entry = await getDeltaEntry(path)
  if (!entry) return null
  return typeof entry.content === 'string' ? entry.content : null
}

/** Read a binary delta as a Blob; `null` when none exists. */
export async function getDeltaBlob(path: string): Promise<Blob | null> {
  const entry = await getDeltaEntry(path)
  if (!entry) return null
  return entry.content instanceof Blob ? entry.content : new Blob([entry.content])
}

/** Persist a text file modification. */
export async function putDelta(path: string, content: string): Promise<void> {
  const store = await tx('readwrite')
  return new Promise((resolve, reject) => {
    const req = store.put({ path, content } as FileDelta)
    req.onsuccess = () => resolve()
    req.onerror = () => reject(req.error)
  })
}

/** Persist a binary file modification (edited sprite / tileset PNG). */
export async function putDeltaBlob(path: string, content: Blob): Promise<void> {
  const store = await tx('readwrite')
  return new Promise((resolve, reject) => {
    const req = store.put({ path, content } as FileDelta)
    req.onsuccess = () => resolve()
    req.onerror = () => reject(req.error)
  })
}

/** Drop a single delta (e.g. after a revert). */
export async function deleteDelta(path: string): Promise<void> {
  const store = await tx('readwrite')
  return new Promise((resolve, reject) => {
    const req = store.delete(path)
    req.onsuccess = () => resolve()
    req.onerror = () => reject(req.error)
  })
}

/** All deltas, for export or wasm replay. */
export async function listDeltas(): Promise<FileDelta[]> {
  const store = await tx('readonly')
  return new Promise((resolve, reject) => {
    const req = store.getAll()
    req.onsuccess = () => resolve(req.result as FileDelta[])
    req.onerror = () => reject(req.error)
  })
}

/** Wipe every delta (editor reset). */
export async function clearDeltas(): Promise<void> {
  const store = await tx('readwrite')
  return new Promise((resolve, reject) => {
    const req = store.clear()
    req.onsuccess = () => resolve()
    req.onerror = () => reject(req.error)
  })
}

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
}

function dataUrlToBlob(dataUrl: string): Blob {
  const [meta, b64] = dataUrl.split(',')
  const mime = /data:([^;]+)/.exec(meta)?.[1] ?? 'application/octet-stream'
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return new Blob([bytes], { type: mime })
}

/** Export all deltas as a JSON payload (download / backup). Binary entries
 *  are serialized as data: URLs so the whole set stays JSON-serializable. */
export async function exportDeltasJson(): Promise<string> {
  const deltas = await listDeltas()
  const serializable = await Promise.all(
    deltas.map(async (d) => ({
      path: d.path,
      content: typeof d.content === 'string' ? d.content : await blobToDataUrl(d.content),
    })),
  )
  return JSON.stringify(serializable, null, 2)
}

/** Import a payload produced by `exportDeltasJson`, replacing all deltas. */
export async function importDeltasJson(json: string): Promise<number> {
  const parsed = JSON.parse(json) as { path: string; content: string }[]
  if (!Array.isArray(parsed)) throw new Error('delta export must be an array')
  const store = await tx('readwrite')
  return new Promise((resolve, reject) => {
    const txReq = store.transaction
    for (const d of parsed) {
      const entry: FileDelta = (typeof d.content === 'string' && d.content.startsWith('data:'))
        ? { path: d.path, content: dataUrlToBlob(d.content) }
        : { path: d.path, content: d.content }
      store.put(entry)
    }
    txReq.oncomplete = () => resolve(parsed.length)
    txReq.onerror = () => reject(txReq.error)
  })
}
