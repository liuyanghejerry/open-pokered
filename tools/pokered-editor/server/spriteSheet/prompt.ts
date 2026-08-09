// ───────────────────────────────────────────────────────────────────────────
// prompt.ts — contract-based prompt assembly (port of prompt.go).
//
// Fixed written contracts (style, keying canvas, sprite design, row layout,
// reject clause, facing) replace vague instructions, and defect-driven retry
// hints (from inspect.ts) narrow constraints instead of re-rolling the dice.
// ───────────────────────────────────────────────────────────────────────────
import type { StateSpec } from './types'
import { facingPromptSection } from './direction'
import { motionHint } from './presets'

/** Selectable style contracts. */
export const StylePresets: Record<string, string> = {
  pixel: 'true low-resolution pixel-art game sprite, like a 32-64px sprite enlarged on the canvas, ' +
    'chunky readable silhouette, clean dark 1px outline, visible square pixel blocks, ' +
    'grid-aligned hard pixel edges, limited shared palette, solid tone clusters, ' +
    'flat color shading with at most one highlight step and one shadow step, ' +
    'simple readable face and clearly separated limbs. ' +
    'Never use painterly rendering, smooth gradients, airbrush shading, glossy lighting, ' +
    'anti-aliased fine detail, high-definition pixel art, fine-grained pixel art, anime illustration, concept art, or 3D rendering.',
  chibi: 'cute chibi game sprite with oversized head and small body, ' +
    'bold dark outline, flat bright colors, minimal shading, large expressive eyes, ' +
    'clean cartoon shapes readable at small size. ' +
    'Never use realistic proportions, gradients, or painterly detail.',
  cartoon: 'clean 2D cartoon game sprite, bold uniform outline, flat vivid colors, ' +
    'simple two-tone cel shading, smooth rounded shapes, expressive but simple face. ' +
    'Never use pixelation, gradients, photo textures, or 3D rendering.',
  retro16: '16-bit retro console era game sprite, restrained palette of 16-24 colors, ' +
    'dark outline, dithering only where needed, compact proportions, ' +
    'crisp hard pixel edges like a classic arcade fighter sprite. ' +
    'Never use modern smooth shading or high-resolution detail.',
}

/** The keying-background phrase (the color matting separates out). */
const keyColorPhrase = 'pure keying magenta (#FF00FF), perfectly uniform edge to edge'

/** resolveStyle — preset key or custom text → a style contract. */
export function resolveStyle(presetKey: string, custom: string): string {
  if (custom.trim() !== '') return custom.trim()
  return StylePresets[presetKey] ?? StylePresets.pixel
}

/** canvasContract — the keying-canvas rules the matting stage depends on. */
function canvasContract(): string {
  return 'Keying canvas (the renderer mattes this away — obey exactly):\n' +
    `- Fill the ENTIRE background, edge to edge, with ${keyColorPhrase} — a single flat color touching all four image borders. No gradient, texture, scenery, floor, panel, frame, or border of any kind.\n` +
    '- The subject must avoid magenta, pink and purple entirely — clothing, props, highlights and effects included — so the keyer never eats part of the character.\n' +
    '- Drop every shadow and contact patch; the ground is implied, never painted.\n'
}

function spriteDesignContract(): string {
  return 'Game-sprite design contract:\n' +
    '- Interpret the subject as a game-ready character sprite, not an illustration, poster, sticker, mascot logo, or concept-art render.\n' +
    "- Preserve the subject's identity through a strong silhouette, hairstyle, outfit shapes, accessories, weapon or signature prop, and dominant color blocks.\n" +
    '- Simplify anatomy into readable sprite shapes: compact torso, clear head shape, simple arms and legs, minimal joint detail, no tiny anatomy rendering.\n' +
    '- Hair, clothing layers, capes, hats, weapons and accessories should read as distinct hard-edged pixel shapes, not detailed painted textures.\n' +
    '- Keep the face simple at sprite scale: readable eyes and mouth, minimal facial detail, no realistic nose or painted skin texture.\n'
}

function lowResPixelContract(): string {
  return 'Pixel rendering contract:\n' +
    '- The image must look like a 32-64px game sprite enlarged to the canvas, not newly painted at high resolution.\n' +
    '- Use chunky square pixel blocks, clean 1px outline, solid tone clusters, limited palette, minimal two-step flat shading.\n' +
    '- No dithering, no smooth gradients, no soft shadow, no blur, no airbrush, no texture, no fine hair strands, no tiny jewelry detail that would vanish at 64px.\n' +
    '- Every important shape must remain readable when shrunk to a thumbnail: silhouette first, details second.\n'
}

function pixelStyleContracts(style: string): string {
  const s = style.toLowerCase()
  if (!s.includes('pixel') && !s.includes('sprite') && !s.includes('mmorpg')) return ''
  return spriteDesignContract() + '\n' + lowResPixelContract()
}

/** rejectClause — concise contract rejecting elements that break extraction. */
function rejectClause(): string {
  return 'Reject (these break automatic extraction):\n' +
    '- ANY frame, border, or decoration around the image or around a pose: no film strip, no sprocket holes or perforations, no photo/polaroid frame, no panel dividers, no outline box, no vignette. The background reaches every edge unbroken.\n' +
    '- Motion garnish — streaks, speed lines, blur, after-images, arcs, swooshes, trails.\n' +
    '- Free-floating bits — sparkles, stars, dust, smoke puffs, icons, symbols, or any mark not fused to the body.\n' +
    '- Text, numbers, captions, grids, rulers, speech or thought bubbles, UI, watermarks.\n' +
    '- Any pose that is clipped by the edge, or whose pixels bridge into the neighbouring pose.\n'
}

/** buildCharacterPrompt — text description → base character image prompt. */
export function buildCharacterPrompt(description: string, style: string): string {
  let b = 'Produce one complete game-character reference sprite in a relaxed player-avatar standing pose.\n\n'
  b += `Subject: ${description.trim()}.\n\n`
  b += "Feature audit before drawing (do this internally, then render): identify and preserve the subject's hairstyle, hair color, eye color, outfit layers, accessories, weapon or signature prop, symbolic motifs, and dominant colors.\n\n"
  b += `Render contract (obey strictly): ${style}\n\n`
  const extra = pixelStyleContracts(style)
  if (extra) b += extra + '\n'
  b += 'Framing:\n'
  b += '- A single figure, head to feet, vertically centered, occupying about three quarters of the canvas height with generous breathing room on every side.\n'
  b += '- Idle standing sprite pose: feet level, weight balanced, arms relaxed but readable.\n'
  b += '- Almost flat 2D game-sprite view; avoid dramatic perspective, foreshortening, cinematic camera angles, and illustration-style posing.\n'
  b += '- One continuous silhouette — nothing detached, no trailing accessories or particles.\n\n'
  b += canvasContract()
  b += '\n'
  b += rejectClause()
  return b
}

/** buildStripPrompt — per-state horizontal strip generation prompt. */
export function buildStripPrompt(description: string, style: string, spec: StateSpec, feedback: string): string {
  const n = spec.frames
  let b = `Draw a single horizontal row of exactly ${n} game-sprite poses of one character for the "${spec.name}" animation, ordered left to right. This is raw sprite art, not a photo or a film — draw only the character poses on a flat background.\n\n`

  b += 'Subject lock (top priority):\n'
  b += '- The attached image is the canonical character. Match it exactly across every pose: face, hairstyle, build, outfit, accessories.\n'
  b += "- Palette is binding. Re-sample each region's hue, saturation and value from the reference — skin, hair, every garment, every piece of gear. Do not re-tint, re-light, brighten, darken, or substitute a similar shade.\n"
  b += '- Hold one fixed camera and facing. The figure never rotates, mirrors, ages, or restyles between poses — only the body moves.\n\n'

  const d = description.trim()
  if (d) b += `Subject notes: ${d}.\n\n`
  b += `Render contract (obey strictly): ${style}\n\n`
  const extra = pixelStyleContracts(style)
  if (extra) b += extra + '\n'

  const sec = facingPromptSection(spec.facing)
  if (sec) b += sec + '\n'

  let action = spec.action.trim()
  if (action === '') action = spec.name
  b += `Movement: ${action}.\n`
  const hint = motionHint(spec.name)
  if (hint) b += `Choreography: ${hint}\n`
  b += `Treat the ${n} poses as evenly timed beats of one continuous motion — pose k is phase k of ${n}, and neighbours read as smooth in-betweens, never unrelated stances.\n`
  b += spec.loop
    ? 'It loops: the final pose must hand off cleanly into the first.\n\n'
    : 'It plays once: give it a clear start, peak, and settle.\n\n'

  b += 'Row layout:\n'
  b += `- Place exactly ${n} poses in one horizontal row, evenly spaced left to right — ${n} poses, no more and no fewer. Count them before finishing.\n`
  b += '- Every pose is the SAME size at one shared scale, each filling about 70-85% of the canvas height. No pose may be noticeably smaller, larger, or set further back than the others.\n'
  b += '- Leave a generous band of the flat keying background between every pair of poses. The gap must be wide enough that a human can easily see each pose is separate — never touching, overlapping, or bridging.\n'
  b += '- Each pose is ONE whole, connected body. Never split a body into separate pieces, and never let two poses touch, overlap, or merge.\n'
  b += "- Center each pose's torso horizontally in its share of the row; arms, legs and head move, but the torso stays put and no body part is cut off by the canvas edge.\n"
  b += '- Keep all poses standing on one common ground line, unless the action leaves the ground (a jump).\n'
  b += "- When the body leans or reaches far to one side, keep the torso/hips within the pose's column so that poses do not bridge into the next gap.\n\n"

  b += canvasContract()
  b += '\n'
  b += rejectClause()
  b += '- Favor changes of pose, weight and expression over decoration; any effect must be opaque, hard-edged, and fused to the body.\n'
  b += '- Keep every pose legible at thumbnail size: bold silhouette, clear limbs, no detail that vanishes when shrunk.\n'

  const f = feedback.trim()
  if (f) b += `\nArtist revision (apply over everything above): ${f}\n`
  return b
}

/** aspectForFrames — generation aspect ratio for a frame count. */
export function aspectForFrames(frames: number): string {
  if (frames <= 1) return '1:1'
  if (frames <= 3) return '16:9'
  return '21:9'
}
