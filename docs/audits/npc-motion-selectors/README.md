# Preserve original NPC motion selectors

Original reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
`data/maps/objects/*.asm` and `engine/overworld/movement.asm`.

The native map's `range` cannot also represent the original movement byte:
210 stationary objects have `range=2` (208 automatic trainers and the
scripted Nugget Bridge/Fossil trainers), which the stationary movement adapter
interpreted as LEFT_RIGHT, despite their original fixed DOWN/UP/LEFT/RIGHT.
21 boulders lost their original selector 16. The two link-room STAY ANY_DIR
opponents already use Stationary with legacy range 254; their classification
and random-facing behavior are preserved, with the explicit original selector 0.

`movementByte2` now preserves the original direction/axis independently. Both
runtime loaders consume it; older/custom maps without the field keep their
legacy behavior. Trainer range values, sight headers, coordinates, initial
facings, and NPC order remain intact. Editor save keeps the optional property.
The static embedded data and its owned conversion preserve it too.

All 918 NPCs in 223 original object maps are imported. The remaining 25 native
map entries have no NPCs. Recheck against the pinned external reference with:

```sh
python3 scripts/sync_npc_motion.py /path/to/pret-pokered --check
```

The script validates all rows before writing and preserves unrelated JSON
formatting. `original_npc_motion.json` in the core overworld module records the
reference rows; runtime integration checks both loaders against all 918 rows.

## Actual game verification

`fixture.sav` is an offline constructed SRAM positioning the player at Route10
(3,59), between the original Hikers at (3,57), facing UP, and (3,61), facing DOWN.
It derives from the Name Rater audit fixture, changes map/coordinates/parity,
and recomputes the bank-1 checksum. It does not prove fresh travel to Route10.

Both runs boot normally, CONTINUE, use seed42 and only physical input timelines
plus readonly observations. Captures use the same absolute frames 2000..2512
in increments of 32. On the base the northern Hiker repeatedly turns LEFT/RIGHT;
on the candidate every sample faces UP. The frame2000 screenshots are committed
under `docs/screenshots/npc-motion-selectors/`. `comparison.json` records binary,
fixture and map hashes, frames and directions; gzip files retain command logs
and readonly observations. These are functional observations, not an original
animation parity PASS.

A Museum2F control used the same seed and absolute frames: all 17 screenshots
and sampled game/NPC states were identical. **Correction to the initial audit:**
all 251 NONE objects already use range 255 on master and turn randomly; they are
not 251 missing features. The new field preserves that existing behavior.

The tested base executable is efff758f; master506f differs only in committed
audit documents/screenshots, with no crates, scripts or Cargo changes.

Local verification: 3,182 core and 63 agent library tests, 26 trainer integration
tests, and the embedded-data selector/serialization test pass. The final native
build passes both portable replay cases.

```sh
python3 docs/audits/npc-motion-selectors/replay.py --binary /path/to/pokered-app \
  --output /new/empty/output --expect fixed
# Existing random-facing control:
python3 docs/audits/npc-motion-selectors/replay.py --binary /path/to/pokered-app \
  --output /another/new/output --expect fixed --case museum
```

Fresh NEW GAME through m10 also passes: the player loses once in the forest,
uses the existing driver's training/retry policy, and earns the Boulder Badge
at frame151079. No startup fixture or live setters are used. This broader
regression retains the existing `wait_until`, `step_frames` and `skip_dialogue`
helpers; it is not physical-only full-playthrough evidence. `fresh-*` archives
retain the actual runner, source manifest, milestone observations and log.
The runner's local paths can be replaced for reproduction.

A client-only transport adapter ACKs incoming TCP chunks immediately to avoid
the server's split-write delayed-ACK stall. It changes no JSON command or game
code. The native Route10 control produces identical game/NPC state and all17
PNGs at the same absolute frames with the adapter. `validation-transport.py`
and `transport-control.json` record that auxiliary validation. The adaptive
milestone driver's exact input counts/frame anchors are not a parity claim.

An earlier ordinary session ended with SIGTERM during training (cause not
established); the second slow run was deliberately stopped after the transport
control passed. Neither is counted as a passing run. Only the completed fresh
third run supplies the m10 verdict. Full m01–m49/HOF/credits/independent CONTINUE
and original animation parity remain outside this PR's verified scope.
