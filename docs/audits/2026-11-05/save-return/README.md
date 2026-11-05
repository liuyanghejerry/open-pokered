# SAVE returns to the field

SAVE cancellation and completion now restore screen buffer 2, wait for held A to be released, then execute CloseTextDisplay. The player returns to the field instead of reopening START. Cancel retains TwoOptionMenu's 15-frame choice delay, preserves facing/position, and cannot write SRAM. The successful save still commits once before restoration begins.

Production commit: `32ace488f18840ca0ae32221456c0cde92a8836f`. Before: START stack `efef56d6670c0c9e42900feb7ab7858fe352b0e5` (production `f482e587`). The before/after SAVE prompt states and screenshot hashes are identical for all four cases. Fonts and the existing save-card layout are outside this change.

The reference is normal Red at pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, ROM SHA-256 `5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`, executed with audio enabled in PyBoy. A prepared original party state enters START with B, then selects SAVE using real input. Native uses ordinary SRAM cold CONTINUE at Viridian City (20,30), then the same menu selection and verdict inputs. No recording-time state setters are used.

| Input from the SAVE prompt, relative frame indexes | Original first field loop | Before | After |
| --- | --- | --- | --- |
| B on 0–1, release | 39 | remains START | 39 |
| B on 0–1, Right on 3–4 | 39 | remains START | 39 |
| select NO, hold A on 0–79, release | 104 | remains START | 103 |
| select YES, hold A on 0–599, release | 624 | remains START | 623 |

Each case records every logical frame (240 or 760), repeats twice, and checks exact per-frame state and selected image hashes. Each native case also boots a separate process with its resulting SRAM. Cancel/NO keep the bytes unchanged; YES writes once at frame 189 in both before and after. Reopening START after the return works.

**Timing qualification:** the native A-release gate polls physical input and keeps the shared IRQ/audio owner running, but does not yet execute the original Joypad spin loop instruction by instruction. The two held-A cases return one frame earlier than the original. This is a functional return/input fix, with animation timing **PARTIAL**; it does not claim exact SAVE text, write timing, entire actor traces or GBA physical duration parity. The prior save-commit timing is deliberately measured and unchanged.

Local application tests: 1,100 executions and 92 existing ignores. Core: 3,989 executions and one existing ignore. ARM check and unmodified default GBA release build passed. Fresh NEW GAME through m06 passed in 41.105 seconds; this is early-game smoke, not a SAVE-specific full m49 verdict. The parent Party/START full first-clear runs are tracked on their own PRs.

Default GBA release passed all four cases twice at Indigo Plateau, with 800/800/1,400/2,800 consecutive physical frames per repeat. Every frame and stage hash repeats; cancel and short-input image series match exactly. Actual cartridge SRAM changes only on YES, remains unchanged while reopening START, and survives eight independent GBA cold CONTINUE runs. Debug/release native runs also match every recorded frame and image. Details are in verification.json. The old bridge used `mCoreLoadSaveFile(..., temporary=true)`, so unchanged host file bytes did not establish unchanged cartridge SRAM. The read-only `savedataClone` bridge here exports the actual cartridge bytes without advancing frames or writing game state. The independent cold CONTINUE loads those exported bytes.

## Replay

Build the shared native binary with `--features debug-server`. The script defaults to the already committed `../start-menu-close/fixture.sav`; `--fixture` accepts another ordinary SRAM. Supply the binary's actual SHA-256 and a fresh output directory:

```sh
python3 docs/audits/2026-11-05/save-return/replay-native.py \
  --binary /absolute/path/to/pokered-app --sha256 ACTUAL_SHA256 \
  --output /absolute/path/to/new-output --candidate
```

Omit `--candidate` only when reproducing the before build's incorrect START destination. Compressed records retain the exact input timeline, complete native restoration observations and reference routine events. Repeated recordings and full build identities stay outside the repository; verification.json carries their hashes and compact results. Reference state/ROM files are not redistributed.


GBA replay uses mGBA 0.10.5 headers/library and the default release ROM (no autopilot, frame skipping or game-memory writes):

```sh
cc -shared -fPIC -O2 -I /path/to/mgba/include mgba-sram-bridge.c \
  -L /path/to/mgba/lib -lmgba -o /absolute/path/to/bridge.so
python3 replay-gba.py 1 save-held --rom /absolute/path/to/default.gba \
  --sha256 ACTUAL_ROM_SHA256 --bridge /absolute/path/to/bridge.so \
  --output /absolute/path/to/new-output
```

The script also accepts `cancel`, `short`, and `no-held`; repeat with argument `2`. Its fixture is the existing `../party-return/indigo/fixture.sav`. The helper includes the library's generated `mgba/flags.h` so the public `mCore` ABI matches the installed library. It observes SRAM with `savedataClone`, then cold boots that exported save; host file immutability is never substituted for actual cartridge persistence.

## CI follow-up: empty-NPC maps

After synchronizing START and NPC grass priority, the SAVE candidate's GBA gate reproduced two failures locally: idle 383 ticks (limit 382.9) and movement 395 (limit 394.4). Maps without NPCs were still resolving grass/blockset data for an empty sampling loop. The guard now skips that lookup only when there are no NPCs; nonempty maps execute the same sampling and movement code. No timer, input, rendering cadence, scenario window, or performance threshold changes.

`performance-followup.json` records the same benchmark before/after, ROM/source hashes and the unchanged baseline. Idle is now 379 ticks and movement 392; all seven scenarios pass. These are hardware-timer measurements, not desktop FPS measurements.

The follow-up passed 1,104 app / 3,991 core test executions and ARM/default GBA builds. Fresh m01–m06 passed in 41.46 seconds. All four SAVE cases × two native runs have byte-identical frame states and image hashes to the pre-optimization build, plus eight independent CONTINUE checks. Default GBA B and held-A/YES × two runs retain 7,200 physical frames; complete frame sequences repeat, only YES changes cartridge SRAM, and each cold CONTINUE renders exactly the saved field.
