> 最终范围更新：用户要求不改字体。本文记录此前 8px 字库实验及相关诊断，
> alphabet 替换与纯字体配套布局已撤回。最终范围和原字体回归见
> [font-preserved.md](font-preserved.md)，不可把本页旧图当作最终结果。

# Font integration repaint repairs

These fixes follow the integrated host-library failures on source `a5f2d1b`.
The tests compare every framebuffer pixel against a fresh render; their
expectations are retained. Root runs the focused and integrated Cargo checks.

* Mart: the actual main cursor is `(1,1)`, while `ShopVisualKey` still used
  `(1,2)`. The list moved to x=5 with four English / three Chinese entries,
  but the retained renderer still used x=2 and five entries. Share cursor
  coordinates and viewport calculation with `draw_mart`; a viewport change
  changes the visual hash and redraws the list. Local tests now use the real
  coordinates and reserve viewport transitions for a new production
  `RenderSession` versus full-frame regression, including both languages,
  all main choices, buy/sell boundaries and CANCEL.
* Battle party popup: labels begin at x=24; the right border begins at
  x=144. `CHARMANDER 65/65` occupies 128px in the original font and crossed
  that border, outside the old copied/cleared 120px band. Preserve complete
  health/status text and abbreviate a name with the original ellipsis tile
  when necessary. The old y=103, height=38 scroll band also copied the
  bottom border's ink upwards. Redraw only this bounded popup on viewport
  changes and report its full rectangle as damage. Existing all-transition
  pixel regressions stay in place; new level-25/100 long-name and fainted
  label tests assert the complete health/status suffix and actual 120px
  bound.
* Town Map: the old 13px label clear at y=128 covered the bottom border tile
  at y=136, erasing its original horizontal strokes (first failure x=8,
  y=138). Restore the three-row name box and then the new label in full
  render order; report that complete small box as damage. The existing
  view/FLY and every-landmark pixel comparisons remain unchanged.

Follow-up visual review found that the complete Chinese frame itself placed
13px Fusion glyphs at y=128, touching the restored original bottom border at
y=138. The Chinese name box now reserves two interior tile rows, starting at
y=112 and placing the name at y=120. English retains its original three-row
box/name at y=120/128. Marker visibility and reported damage use the same
language-specific box boundary. A pixel regression requires two blank rows
between the Chinese name and bottom border, and the original stroke at
(8,138); the existing every-landmark retained/full comparisons also remain.

This is a correction of real retained-frame behavior after introducing the
original font/borders, not a claim that the old hardcoded coordinates or
font bounds were correct. Source validation before handing the commit to
root: `git diff --check`; Cargo validation is recorded by root separately.
