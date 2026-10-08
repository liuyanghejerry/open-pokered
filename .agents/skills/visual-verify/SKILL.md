---
name: visual-verify
description: Verify Pokemon Center healing machine rendering with the visual test harness, expected layout, and OAM-to-screen coordinate reference. Use when diagnosing healing overlay placement or animation rendering.
---

# Visual verification — healing machine

Use the shared renderer harness for layout checks. For animation or timing
claims, also use `key-animation-differential`: a manually seeded healing frame
is not a recording of the real nurse interaction.

```bash
cargo test -p pokered-app --test visual_verify_heal_machine -- --nocapture
```

The test writes four frames in the crate working directory. Run from the
workspace with `gfx/` fetched. For PR comparisons, check out master and the PR
branch and render the same state; commit screenshots and embed absolute raw
branch URLs as required by AGENTS.md.

## Original coordinates

Primary source: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`macros/gfx.asm`, `engine/overworld/healing_machine.asm`,
`home/reset_player_sprite.asm`, and `engine/gfx/sprite_oam.asm`.

`dbsprite x_tile, y_tile, x_pixel, y_pixel, tile, attributes` emits **raw OAM**:

- OAM X = `x_tile * 8 + x_pixel`.
- OAM Y = `y_tile * 8 + y_pixel`.
- LCD X = OAM X − 8; LCD Y = OAM Y − 16.

The first macro parameter is X, not Y. Do not swap them or confuse the raw
OAM coordinates with sprite-state screen pixels. `PrepareOAMData` adds hardware
offsets to character sprite-state coordinates; the LCD subtracts them again.
`ResetPlayerSpriteData` stores player screen `(64,60)`, while the background
cell origin under the player is `(64,64)`.

The canonical actual healing interaction is player `(3,3)` facing Up toward
nurse `(3,1)`. The nurse sprite screen top-left is `(64,28)`; its ground-cell
origin is `(64,32)`. The engine's renderer projects the machine relative to
that ground cell, so it stays on the machine if a diagnostic fixture moves the
camera. Do not subtract the character's extra four-pixel Y offset a second
time from the machine overlay.

| Sprite | Original dbsprite parameters (X,Y,sub-X,sub-Y) | LCD top-left (X,Y) |
| --- | --- | --- |
| Monitor | 6,4,4,4 | 44,20 |
| Ball 1 | 6,5,0,3 | 40,27 |
| Ball 2 | 7,5,0,3 | 48,27 |
| Ball 3 | 6,6,0,0 | 40,32 |
| Ball 4 | 7,6,0,0 | 48,32 |
| Ball 5 | 6,6,0,5 | 40,37 |
| Ball 6 | 7,6,0,5 | 48,37 |

Balls 2/4/6 have `OAM_XFLIP`. Each entry is **one 8×8 sprite**, not a repeated
16×16 or 32×32 grid. The 8×16 `heal_machine.png` contains monitor tile 0 above
ball tile 1; the original copy length of three tiles is a documented source
error (`should be 2`), not an instruction to enlarge the drawing.

## Palette and timing

`AnimateHealingMachine` writes OBP1 `$e0`:
`[transparent, white, #555555, black]`. `FlashSprite8Times` XORs `$28`, giving
`$c8`: `[transparent, #555555, white, black]`. The original swaps white and
dark gray here, not light gray and dark gray. It adds one ball per party member
with 30-frame waits, then eight 10-frame flash intervals; verify the real
sound/fade/fanfare boundaries before declaring an entire sequence aligned.

## Checks

Render the actual nurse interaction for behavioral claims; use the seeded
harness to inspect monitor/ball geometry and palette. Check tile 0/1 selection,
horizontal flips, transparent color zero, and the table above at the canonical
player/nurse positions. Include captured background and actor coordinates in
the report so camera or sprite-origin changes cannot be mistaken for machine
placement changes.
