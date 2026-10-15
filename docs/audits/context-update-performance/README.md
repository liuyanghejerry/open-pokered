# Frame context update performance

This change removes two recurring costs without changing query inputs, hashes,
rendered output, or game state. The measured base is master
`f0787e19e7623106eac1d6b8de7430ca1eb4428b`, with dotzuki
`76da597b1604305121c866831b5542e70c2b82c8`.

On ARM32 without an OS, multiplying an FNV hash by its fixed prime previously
called `__aeabi_lmul`. The replacement calculates the same wrapping product
using 32-bit limbs and 16-bit partial products. Other targets retain the
original multiplication. The prime, byte order, hash stream and cache ownership
are unchanged. Static inspection of the actual GBA query function finds 33
wide-multiply call sites before and zero after; this does not imply a dynamic
instruction count. Its function size grows from 2224 to 2924 bytes.

During ordinary and canonical link battles, the game also cloned the same
player name into the battle screen every frame. It now keeps an equal existing
string and still updates a changed name. No field was added to PokemonGame.

## Checks

- An independent `u128` product oracle covers 3,293,760 limb/carry cases.
  Complete streams, chunk boundaries, full-width scalar values and byte order
  are checked separately. A diagnostic-only GBA build also asserts every actual
  query chunk against the original wide product across all seven benchmark
  scenarios; the diagnostic code is absent from production.
- Real Game updates preserve a pre-reserved player-name allocation in both
  battle callers and propagate changed names. The two negative controls fail
  against the untouched base and pass after the change.
- Five focused tests pass with default and debug-server features. Full core:
  2889 passed. Full debug-server app, normal parallel execution: 301 passed,
  39 ignored. The locked app debug build and normal GBA release build pass.
- A separate ordinary BOOT/CONTINUE, movement, Giovanni battle, departure and
  START/SAVE run records every one of 9749 updates. Input, context, RGBA,
  logical audio, four APU controls and continuously rendered PCM match exactly.
  The 31,196,844-byte WAV, SRAM, script flags and saved screenshot are byte
  identical, and the SRAM checksum is valid. These controls start from the same
  declared preBOOT save; they do not claim complete-world reachability or
  original-game fidelity.

## GBA measurements

Same nightly-2025-12-07, release configuration, emulator, benchmark scenarios
and unchanged performance baseline/limits for every column. Values below are
average update ticks; all 31 gated metrics are retained in verification.json.

| Scenario | Before, pin76 | FNV only, pin76 | Final, pin76 | Final, pin99c |
| --- | ---: | ---: | ---: | ---: |
| Battle entry | 216 | 216 | 204 | 206 |
| Intro/title | 118 | 118 | 118 | 117 |
| Oak dialogue | 188 | 188 | 188 | 186 |
| Overworld idle | 382 | 349 | 349 | 350 |
| Overworld movement | 388 | 354 | 354 | 355 |
| Pokédex entry | 116 | 116 | 116 | 117 |
| Trainer entry | 232 | 232 | 223 | 224 |

The FNV-only pin99c counter-experiment still fails trainer update at 234 ticks
(limit 232.3), establishing why the second optimization is needed. The final
combination passes all 31 metrics both with the existing pin76 and in an
isolated pin99c experiment. This branch retains pin76; it does not include the
new engine pin or audio snapshot feature.

The final performance samples were taken from an earlier successful locked
build. A subsequent check changed only a cfg(test) import; after it completed,
the final source manifests and ELF bytes were verified identical to the
measured artifacts. Both identities are retained in verification.json.

Raw source manifests, compiler logs, normal/diagnostic/performance ROMs,
benchmark logs, independent negative controls and full-prefix evidence remain
under the audit prefix `audit-929-` in the local audit directory
`.task-tmp/fidelity-39-plus`. The checked-in verification file includes the
measured metrics and artifact hashes. No performance budget was changed.

This is a pure performance change with identical frame output; it has no
visual difference requiring comparison screenshots. Silph departure timing and
the generic audio snapshot change remain separate work.

## Current master integration

After the original f078 controls and commit `ca71a45a` were complete, this
branch merged master `32f74146eff45ec883e706d119ad56f90c2be8f0` (Old Man
capture fix). The original measurements above retain their original identity.
A separate 6762-file source freeze and two 2165-file GBA mirrors were used for
new integration checks: focused tests 5+5, core 2890, parallel debug app 303
(39 ignored), locked app build and normal GBA release all pass.

All 31 performance limits pass again on both pins. Current pin76 update ticks:
idle349, movement354, trainer223, battle205. Isolated pin99c: idle350,
movement355, trainer225, battle206. The complete current metrics and ROM hashes
are recorded separately in verification.json. No threshold was changed.

The newly compiled integration runner also completes the same ordinary 9749
frames and SAVE. Its complete ledger, WAV, SRAM, flags and screenshot match the
untouched f078 BEFORE bytes; this is an additional integration check, not a
relabeling of the original matched f078 before/after controls. Both integrated
GBA query functions still contain zero static wide-multiply call sites.

## Independent root review

`root-review.json` verifies the entire 6,762-file current compiler freeze (only the two declared documentation files changed afterward), all current test/build logs, both exact 2,165-file GBA mirrors and every one of their 31 budget rows. It independently compares all three raw 9,749-frame prefix arrays and the complete WAV, SRAM, flags and saved PNG; the original f078 BEFORE/AFTER and the new 32f integration identities remain separate. The production/test diff was reviewed in full. `review-logs.zip` contains the explicit small log/report allowlist and SHA manifest; ROMs, executables, hardware states and the large WAV are kept outside this PR.
