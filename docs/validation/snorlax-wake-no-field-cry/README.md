# Snorlax wake: remove the extra field cry

Route12 and Route16 originally print the wake text and start the level-30 wild battle without a separate field cry. The two scene scripts had an extra `playCry("SNORLAX")` before that text. The fix removes those two calls; the battle intro still plays Snorlax's cry.

The final normal-CLI pair used one constructed physical SRAM, normal boot/Continue, START → ITEM → POKé FLUTE → USE, ordinary completed-page confirmations, then the wild-battle command menu. Both builds received the **same 1905-frame literal tape**, seed 42, English, default features plus `debug-server`. No runtime setters, warp, skip-intro, snapshot, or script override were used.

| Recorded build | Source | Field cry: id 24 playing | Battle cry: id 24 / WildReveal |
| --- | --- | --- | --- |
| Before (M14) | `f6dd110e5f2a4f24d6a40d6794eda63ec3789bdf` | 1265–1280 | 1735–1750 |
| After | `b5fb543f8ff120247d33c6c11aab75deb3018284` | absent | 1735–1750 |

Both ended at absolute frame 1905 in `PlayerMenu`, with Snorlax level 30, party HP, money, bag, and final script flags matching. Audio observations are the live application's **logical audio ID and phase**, not a PCM recording. Five relevant Core tests passed, including the parameterized real Flute/wake consumer for Route12 **and** Route16. Only Route12 was run through this normal-CLI pair.

The images under `docs/screenshots/snorlax-wake-no-field-cry-{before,after}.png` are both **absolute frame 1269**, using that same full tape: before shows `S`, after `SN`. Deleting the extra script-effect update begins the wake text one frame earlier. This is visual evidence; the screenshots do not prove the sound difference. Of the eight captured frames 1267–1274, only 1269 and 1272 differ; the other six PNGs match byte for byte.

One earlier v2 **before** recording failed because its controller supplied only neutral battle inputs and omitted the ordinary intro-page ACKs. It timed out at `WildReveal, wait_frames: 0`; that result remains a failure. The final v3 pair added only the two fresh ordinary ACKs after completed intro text, retained the final menu assertion, and passed. Accounting: **one failed Game plus two successful Games**, three total. The independent public `replay.py` below was prepared afterward and checked only by AST parsing and lossless tape expansion; it has **not** been run as another Game.

## Physical fixture

`fixture.sav` is 32768 bytes, Route12 `(9,62)`, beside the visible Snorlax `(10,62)`, with one POKé FLUTE and Pidgey level 20 at full HP. It was constructed offline from the existing public `field-question-ack-1075/fixtures/bike-sale.sav`, changing map/position metadata and bag. The pure SaveData codec actually used Core source `077034dbe197af52423eb02dc9dafbb8e7c0c76b`, not M14. Canonical import/checksum and exact typed-JSON/byte re-export passed; checksum at `0x3523` is `0xD8`. The original flute-coordinate table permits `(9,62)`, and current collision tile `0x39` is passable. This is a constructed physical save, not a naturally reached playthrough save or a runtime/RNG snapshot. Exact hashes are in `validation.json`.

## Reproduce

Build a normal `pokered-app` with `debug-server` from each recorded source above, and provide that exact clean checkout as `--source-root`. Keep this published evidence directory available separately when checking out an older source. Checkout validation alone cannot establish the supplied binary's compiler provenance; the recorded binary hashes are included in `validation.json`.

```bash
python3 /path/to/evidence/replay.py --case before \
  --source-root /path/to/clean-M14-checkout --binary /path/to/M14/pokered-app \
  --output /path/to/new-before-output
python3 /path/to/evidence/replay.py --case after \
  --source-root /path/to/clean-b5fb-checkout --binary /path/to/b5fb/pokered-app \
  --output /path/to/new-after-output
```

The standard-library script expands the shared RLE tape without changing it, copies the SRAM into a fresh output directory, and drives only physical buttons through the normal CLI. It checks the real menus, flute playback, exact cry frames, species, flags, final menu, and frame-1269 image. `--no-audio` keeps the headless logical sequencer active. It captures one PNG, saves each state once, and stops only its own process group after the diagnostic boundary.
