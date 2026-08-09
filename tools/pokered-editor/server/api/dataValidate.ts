// ───────────────────────────────────────────────────────────────────────────
// dataValidate — pre-save validation for a data-table record. Pure + testable
// (no fs / http), so the save route can reject the footguns the audit flagged
// instead of silently corrupting the table:
//   • a JSON value that parses but isn't a record OBJECT (e.g. a bare string) —
//     previously written verbatim;
//   • a missing/empty id field — an unfindable, broken record;
//   • a DUPLICATE id — another file in the table already uses this id value, so
//     the two records collide (the audit's "silently overwrites" case).
//
// Note: filenames are independent of the id value (e.g. zuiquan.json → id "醉拳"),
// so uniqueness is checked against the id FIELD across the table, not the filename.
// ───────────────────────────────────────────────────────────────────────────

/** A sibling record already in the table (for the uniqueness check). */
export interface ExistingRecord {
  /** The record's file name (e.g. "zuiquan.json"). */
  file: string
  /** The record's id-field value (whatever type it is on disk). */
  id: unknown
}

export interface DataSaveInput {
  /** The table's id field name (TableDef.idField, default "id"). */
  idField: string
  /** The file being written (e.g. "zuiquan.json"). */
  fileName: string
  /** The raw request body. */
  body: string
  /** Every OTHER record in the table (exclude the file being saved). */
  existing: ExistingRecord[]
}

export type DataSaveResult =
  | { ok: true; json: Record<string, unknown> }
  | { ok: false; error: string }

export function validateDataSave(input: DataSaveInput): DataSaveResult {
  let json: unknown
  try {
    json = JSON.parse(input.body)
  } catch (e) {
    return { ok: false, error: 'Body is not valid JSON: ' + (e as Error).message }
  }
  if (json === null || typeof json !== 'object' || Array.isArray(json)) {
    return { ok: false, error: 'A data record must be a JSON object (got ' + (Array.isArray(json) ? 'an array' : typeof json) + ').' }
  }
  const rec = json as Record<string, unknown>

  const idVal = rec[input.idField]
  if (idVal === undefined || idVal === null || String(idVal).trim() === '') {
    return { ok: false, error: `Record is missing required id field "${input.idField}".` }
  }

  // Duplicate-id guard: no OTHER file in the table may carry the same id value.
  const clash = input.existing.find(r => r.file !== input.fileName && String(r.id) === String(idVal))
  if (clash) {
    return { ok: false, error: `id "${String(idVal)}" is already used by "${clash.file}". Ids must be unique within the table.` }
  }

  return { ok: true, json: rec }
}
