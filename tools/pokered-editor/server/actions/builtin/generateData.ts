// ───────────────────────────────────────────────────────────────────────────
// Actions: generate-data-set + batch-edit-data — schema-driven data generation.
//
// Both build a Zod schema from the table's TableDef.fields (the same structured-
// output pattern as refine-character) and emit each result as a `proposal` event,
// so they reuse the ChangeSet diff + the review tray + /api/ai/apply-change that
// the chat assistant already uses. Nothing is written until the author applies.
// ───────────────────────────────────────────────────────────────────────────
import type { AiAction, ActionContext } from '../types'
import { buildModel } from '../../ai'
import { ChangeSet } from '../changeSet'

interface FieldDef { key: string; type?: string; label?: unknown; options?: string[]; required?: boolean; description?: string }

function tableOf(ctx: ActionContext, tableId: string) {
  const table = ctx.project.listTables().find(t => t.id === tableId)
  if (!table) throw new Error('Table not found: ' + tableId)
  return table
}

/** Build a Zod object schema from TableDef.fields. */
function recordZod(fields: FieldDef[], z: any): any {
  const shape: Record<string, any> = {}
  for (const f of fields) {
    let s: any
    switch (f.type) {
      case 'number': s = z.number(); break
      case 'boolean': s = z.boolean(); break
      case 'select': s = Array.isArray(f.options) && f.options.length ? z.enum(f.options as [string, ...string[]]) : z.string(); break
      case 'multiselect': s = z.array(z.string()); break
      case 'array': s = z.array(z.any()); break
      case 'object': case 'json': s = z.any(); break
      default: s = z.string()
    }
    if (f.description) s = s.describe(f.description)
    shape[f.key] = f.required ? s : s.optional()
  }
  return z.object(shape)
}

/** Strip the list endpoint's internal bookkeeping keys before showing the model. */
function clean(rec: Record<string, unknown>): Record<string, unknown> {
  const { _file, _error, ...rest } = rec as any
  return rest
}

function fieldsBrief(fields: FieldDef[]): string {
  return fields.map(f => {
    const opts = Array.isArray(f.options) && f.options.length ? ` (one of: ${f.options.join(', ')})` : ''
    return `- ${f.key}: ${f.type ?? 'string'}${f.required ? ' [required]' : ''}${opts}${f.description ? ` — ${f.description}` : ''}`
  }).join('\n')
}

function emitProposal(ctx: ActionContext, cs: ChangeSet, tableId: string, idField: string, rec: Record<string, unknown>) {
  const id = String(rec[idField] ?? '')
  if (!id) return
  const cur = ctx.project.readRecord(tableId, id)
  const before = cur ? JSON.stringify(clean(cur), null, 2) : null
  const after = JSON.stringify(rec, null, 2)
  if (before === after) return // no change
  const pr = cs.add({
    target: { kind: 'data', table: tableId, id, path: `${tableId}/${id}` },
    title: `${before ? 'Edit' : 'Create'} ${tableId} "${id}"`,
    before, after,
  })
  ctx.emit('proposal', { id: pr.id, target: pr.target, title: pr.title, diff: pr.diff, after: pr.after })
}

export const generateDataSetAction: AiAction = {
  id: 'generate-data-set',
  kind: 'object',
  title: 'Generate a data-record set',
  async run(ctx: ActionContext) {
    const { generateObject } = await import('ai')
    const { z } = await import('zod')

    const tableId = String(ctx.input.tableId ?? '')
    const prompt = String(ctx.input.prompt ?? '').trim()
    if (!prompt) throw new Error('prompt is required')
    const count = Math.max(1, Math.min(20, Number(ctx.input.count) || 6))

    const table = tableOf(ctx, tableId)
    const fields: FieldDef[] = (table.fields as FieldDef[]) || []
    const idField = table.idField ?? 'id'
    const existing = ctx.project.listRecords(tableId).map(clean)

    const model = await buildModel(ctx.profile, ctx.apiKey)
    const schema = z.object({ records: z.array(recordZod(fields, z)) })
    const system = [
      `You design balanced game data for the "${tableId}" table of a 2D JRPG.`,
      'Each record must have EVERY required field and respect the field types/options below. Keep ids unique and short (kebab/ascii or a clear name).',
      'Place stats on a sensible curve; avoid dominant or dead options.',
      '\nFields:\n' + fieldsBrief(fields),
      existing.length ? `\nExisting records (for consistency/variety, do not duplicate ids):\n${JSON.stringify(existing, null, 2).slice(0, 6000)}` : '',
    ].filter(Boolean).join('\n')

    const { object } = await generateObject({
      model, schema, system,
      prompt: `Generate ${count} records for: ${prompt}`,
    })

    const cs = new ChangeSet()
    for (const rec of (object as any).records as Record<string, unknown>[]) emitProposal(ctx, cs, tableId, idField, rec)
    try { /* usage discarded by generateObject result; ignore */ } catch { /* noop */ }
    return { proposed: cs.proposals.length }
  },
}

export const batchEditDataAction: AiAction = {
  id: 'batch-edit-data',
  kind: 'object',
  title: 'Batch-edit data records',
  async run(ctx: ActionContext) {
    const { generateObject } = await import('ai')
    const { z } = await import('zod')

    const tableId = String(ctx.input.tableId ?? '')
    const prompt = String(ctx.input.prompt ?? '').trim()
    if (!prompt) throw new Error('prompt is required')

    const table = tableOf(ctx, tableId)
    const fields: FieldDef[] = (table.fields as FieldDef[]) || []
    const idField = table.idField ?? 'id'
    const all = ctx.project.listRecords(tableId).map(clean)
    const ids: string[] | null = Array.isArray(ctx.input.selectedRecordIds) && ctx.input.selectedRecordIds.length
      ? ctx.input.selectedRecordIds.map(String) : null
    const target = ids ? all.filter(r => ids.includes(String(r[idField]))) : all
    if (!target.length) throw new Error('no records to edit')

    const model = await buildModel(ctx.profile, ctx.apiKey)
    const schema = z.object({ edited: z.array(recordZod(fields, z)) })
    const system = [
      `You batch-edit records of the "${tableId}" table of a 2D JRPG per an instruction.`,
      'Return the COMPLETE edited record for EVERY record that should change (keep its id; preserve unrelated fields). Omit records that do not change. Respect the field types/options below.',
      '\nFields:\n' + fieldsBrief(fields),
    ].join('\n')

    const { object } = await generateObject({
      model, schema, system,
      prompt: `Instruction: ${prompt}\n\nRecords:\n${JSON.stringify(target, null, 2).slice(0, 12000)}`,
    })

    const cs = new ChangeSet()
    for (const rec of (object as any).edited as Record<string, unknown>[]) emitProposal(ctx, cs, tableId, idField, rec)
    return { proposed: cs.proposals.length }
  },
}
