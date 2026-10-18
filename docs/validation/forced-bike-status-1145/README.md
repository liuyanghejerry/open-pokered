# Forced-bike status flag regression

The original `BIT_ALWAYS_ON_BIKE` belongs to `wStatusFlags6` bit 5, not `wStatusFlags5` bit 5 (`BIT_DISABLE_JOYPAD`). In this implementation these are packed slots 6 and 5 respectively. Original `ram/wram.asm`, status constants, `engine/menus/start_sub_menus.asm` and the Route 18 gate script provide the independent source references. SRAM addresses are 0x29de and 0x29dc. Both restore and write must use slot 6 and preserve slot 5.

`fixture.sav` is an offline constructed, checksum-valid 32 KiB SRAM fixture: Route 18 (30,8), Bicycle in bag, already riding, Flags5=0, Flags6=0x23. `fixture-provenance.json` records the parent fixture and byte edits. This is a controlled save regression, not a fresh progression or original-ROM gameplay proof.

Before source: master 378367b25778dc982dbae946e949c3d88705c6ca. After source: ce87a6ff63ab26c511c7fd2eabefe1cd0a194622. Both were built separately and replayed twice with identical seed and ordinary input. Binary and screenshot hashes are in `evidence.json`.

At frame 858, before dismounts and after refuses with “You can't get off here.” At frame 960 both are back on the field at the same coordinates: before walking, after biking. Water animation also differs because the refusal keeps the bag open longer and field animation currently pauses there; the images are unmodified and the comparison is not restricted to the avatar.

The fixed replay additionally performs ordinary UI SAVE, verifies the raw bits and checksum, closes the first process, independently starts Continue, and confirms Bicycle still cannot dismount. Saved Flags5=0, Flags6=35; saved SHA-256 is recorded in `evidence.json`. Process termination at cleanup is intentional.

Run from a checkout with graphics fetched and an ordinary debug-server-enabled pokered-app binary built from that checkout:

```sh
python3 docs/validation/forced-bike-status-1145/replay.py \
  --binary /absolute/path/to/pokered-app \
  --output-dir /absolute/path/to/new-output-directory \
  --expected-transport Biking
```

Use `--expected-transport Walking` against the base binary. The output directory must not already exist. The script imports the checkout's playthrough input driver, strips POKERED_* overrides, and uses no runtime setters, debug save, or skip-intro. Binary identity is recorded, not inferred from the checkout HEAD. Unit tests separately cover imported raw SRAM and preservation of unrelated input/text and status bits.
