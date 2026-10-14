# Silph liberation enemy cleanup

Original reference: `pret/pokered` revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
The `SilphCo11F.asm` departure list hides 40 objects: nine Saffron blockers
and 31 enemy NPCs across Silph floors 2–11. Native master `d10b2f79`
hid only the nine blockers and three local boardroom enemies. The 28 enemies
on other floors remained visible after an actual Giovanni victory.

`crates/pokered-data/src/silph_liberation_golden.json` records the original
31 enemy mappings. Each mapping joins the original hide-list constant,
`constants/toggle_constants.asm` bit, `data/maps/toggleable_objects.asm`
object symbol, and map object's `const_export` index. Constant suffixes
are not object IDs: the first 6F enemy is NPC 6, not NPC 1.

Both Giovanni handlers now hide all 40 original targets and show the six
Saffron citizens before setting the victory flag. Nineteen missing aliases
and six floors' scene/config bindings are added. Save layout is unchanged.

## Verification

- 2,840 core unit tests; 55 agent tests; data/config roundtrip checks pass.
- Both coordinate entry routes were driven into actual battles using a
  constructed strong party. Victory-written SRAM independently exported
  and verified: victory flag and all 31 enemy hide bits are present.
- Separate native processes actually CONTINUE both completed battle saves;
  boardroom enemies remain hidden and control returns without another battle.
- Separate processes reload the below-entry battle save and debug-warp to
  all ten occupied floors. All 31 enemies are hidden after the fix; other
  NPC/item visibility matches the before version. This is not stair-traversal
  or a fresh-playthrough claim.
- Before/after screenshots use identical seeded setup, actual victory-written
  saves, identical warp coordinates and 120 neutral frames. The base executable
  represents master `d10b2f79`; the after executable was frozen from this branch.

The command-level test verifies both handlers' full original hide/show lists,
and that losing does not clear enemies or set the victory flag. The map reload
test advances the real scheduled warp transition before checking bindings and
visibility (calling `warp_to_map` alone does not load the destination).

Departure fade animation and exact original frame timing remain a separate
unfinished audit. This change establishes functional object cleanup only.
Existing saves already past Giovanni with missing hide bits are not migrated;
this fixes victories played with the corrected scripts.
