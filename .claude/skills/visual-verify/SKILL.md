# Visual Verification Skill — workspace Healing Machine

Use this skill to verify the Pokemon Center healing machine animation rendering
in the workspace project. This skill provides:
1. A test harness to render the healing machine overlay and save a PNG
2. Coordinate system reference for OAM → screen-space mapping
3. Expected visual layout documentation

## Quick Start

To verify the healing machine rendering, run the visual verification test:

```bash
cd workspace
cargo test -p pokered-app --test visual_verify_heal_machine -- --nocapture
```

The test outputs `heal_machine_frame.png` in the current directory.

## Coordinate System

### OAM → Screen Mapping

The healing machine uses dbsprite OAM coordinates from the original Game Boy.
The mapping in the Rust renderer is:

| dbsprite value | Meaning | Screen pixel formula |
|---|---|---|
| `y` (first param) | OAM Y in 8px units | `y * 8` = screen Y |
| `x` (second param) | OAM X in 8px units | `x * 8` = screen X |
| sub_y (third param) | sub-pixel Y offset | added directly to screen Y |
| sub_x (fourth param) | sub-pixel X offset | added directly to screen X |

**Key rules:**
- No viewport offset (`view_origin_tx/ty`) — healing machine is a screen overlay
- No `* 2` metatile multiplier — dbsprite values are OAM units, not map tiles
- No sub-pixel scroll (`view_sub_x/y`) — fixed position on screen

### heal_machine.png Tileset Layout

The asset is **8×16 pixels** = 2 tiles (1 wide × 2 tall):

```
┌─────────────┐
│  Tile 0     │  ← Monitor tile (8×8 px)
│  monitor    │
├─────────────┤
│  Tile 1     │  ← Pokeball tile (8×8 px)
│  pokeball   │
└─────────────┘
```

- Monitor sprite: **4×4 tile grid** (32×32 px), using tile 0 repeated
- Pokeball sprites: **2×2 tile grid** (16×16 px) each, using tile 1 repeated

### Expected Sprite Positions (160×144 native resolution)

| Sprite | Screen pixel (x, y) | dbsprite source |
|---|---|---|
| Monitor | (32, 48) | `dbsprite 6, 4, 4, 4, $7c, OAM_PAL1` |
| Ball 1 | (40, 48+3=51) | `dbsprite 6, 5, 0, 3, $7d, OAM_PAL1` |
| Ball 2 | (40, 56+3=59) | `dbsprite 7, 5, 0, 3, $7d, OAM_PAL1\|OAM_XFLIP` |
| Ball 3 | (48, 48+0=48) | `dbsprite 6, 6, 0, 0, $7d, OAM_PAL1` |
| Ball 4 | (48, 56+0=56) | `dbsprite 7, 6, 0, 0, $7d, OAM_PAL1\|OAM_XFLIP` |
| Ball 5 | (48, 48+5=53) | `dbsprite 6, 6, 0, 5, $7d, OAM_PAL1` |
| Ball 6 | (48, 56+5=61) | `dbsprite 7, 6, 0, 5, $7d, OAM_PAL1\|OAM_XFLIP` |

Ball layout:
```
         (40,51)[B1] (48,48)[B3]
Monitor:       [B2-flip]    [B4-flip]
(32,48)        (40,59)      (48,56)
               [B5]         (48,53)
               [B6-flip]    (48,61)
```

### Flash Effect

The flash effect swaps the sprite palette's light-gray and dark-gray colors:

```
Normal palette:    [transparent, #AAAAAA, #555555, #000000]
Flash palette:     [transparent, #555555, #AAAAAA, #000000]
```

Both monitor and pokeball sprites use the same palette — when `flash_active` is true,
all healing machine sprites flash simultaneously (matching original rOBP1 XOR behavior).

## Debugging Rendering Issues

### Check tile indices
The tile index formulas in `pokered-app/src/render/overworld.rs`:
```
Monitor:  tile 0 repeated 4×4 (32×32 px sprite)
Pokeball: tile 1 repeated 2×2 (16×16 px sprite)
```

### Check position formulas
```
monitor_x = 4 * TILE_SIZE          (= 32)
monitor_y = 6 * TILE_SIZE          (= 48)
ball_x    = px * TILE_SIZE          (px = dbsprite_x)
ball_y    = py * TILE_SIZE + y_off  (py = dbsprite_y)
```

If images appear at wrong positions, verify:
- No `view_origin_*` subtraction
- No `* 2` multiplier on dbsprite values
- No `view_sub_*` subtraction

### Expected visual output
The PNG output should show:
- A monitor sprite at the top-left area (pixels 32-47, 48-63)
- 6 pokeball sprites arranged in 2 columns × 3 vertical positions
- If `flash_active`: sprites use light↔dark swapped palette
