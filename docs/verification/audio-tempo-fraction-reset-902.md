# Tempo commands restart the fractional note-delay accumulator

The engine update in [dotzuki#93](https://github.com/liuyanghejerry/dotzuki/pull/93) resets
all four fractional note-delay remainders in the command's music or SFX group before
parsing the next note. The other group and previously calculated integer delays remain
unchanged. All root and standalone GBA manifests and both lockfiles use the same revision.

The original arithmetic controls use source revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` and ROM SHA-256
`5ca7ba01642a3b27b0cc0b5349b52792795b62d3ed977e98a09390659af96b7b`.

The old game pin `976f86b42813a657047db89ca4dca2d6b9172dff` has the same source tree
as engine master `9f3407448bb420f9cf8f773ecdd940d928f64456`. The new pin
`76da597b1604305121c866831b5542e70c2b82c8` adds only this audio behavior, its tests,
and the English/Chinese reference update. It retains the NPC stride-direction fix.
No application fade or departure prototype is included.

## Independent arithmetic controls

Nine original `Audio3_PlayNextNote` calls execute the original Tempo command followed
by a length-1 rest at note speed 6. Each uses an explicit initial remainder of 0, 64,
or 255. These are isolated function controls with interrupts disabled, rather than
normal IRQ, rendered-frame, or full-song parity evidence. The native comparison calls
the real sequencer implementation from the old and new engine libraries.

| Tempo | Initial remainder | Original delay/remainder | Old native delay/remainder | Updated native delay/remainder |
|---|---|---|---|---|
| `0x00A0` | 0 | 3 / 192 | 3 / 192 | 3 / 192 |
| `0x00A0` | 64 | 3 / 192 | 4 / 0 | 3 / 192 |
| `0x00A0` | 255 | 3 / 192 | 4 / 191 | 3 / 192 |
| `0x007C` | 0 | 2 / 232 | 2 / 232 | 2 / 232 |
| `0x007C` | 64 | 2 / 232 | 3 / 40 | 2 / 232 |
| `0x007C` | 255 | 2 / 232 | 3 / 231 | 2 / 232 |
| `0x0100` | 0 | 6 / 0 | 6 / 0 | 6 / 0 |
| `0x0100` | 64 | 6 / 0 | 6 / 64 | 6 / 0 |
| `0x0100` | 255 | 6 / 0 | 6 / 255 | 6 / 0 |

The updated native result also resets the other three music fractions to zero in all
nine cases. Separate SFX controls confirm group isolation at tempo `0x0100`.
Custom SFX tempo handling in the original `SetSfxTempo` is outside this comparison.
The engine regression suite checks a Tempo command on a later channel: it resets an
earlier channel's fractional remainder without recalculating that channel's integer delay.

The original controls are test oracles only; game runtime code does not load captured
states, timing tables, or an original ROM. This change has no on-screen output, so
before/after screenshots are outside the audio-only PR's scope.

## Validation

- Upstream engine: 157 default tests, 181 all-feature tests, Clippy correctness,
  workspace formatting, and the local bare-metal audio check passed. The three
  new regression tests failed against the unchanged engine before the fix.
- Game consumer at base `b7c81d82d7d94a06555f631c3c70d114f299d9d8`:
  98 audio tests and 2,887 core library tests passed; the desktop debug-server
  application built successfully against the new Git dependency.
- All 37 dependency entries in 15 manifests agree. Replacing the new SHA with
  the old SHA in either lockfile reproduces its base bytes, so neither lockfile
  contains unrelated dependency changes.
- The 6,638-file source freeze was checked after the builds, with no changed
  input. Only this validation section and provenance text were then added.

The root `cargo metadata --locked --offline` resolution passed. A standalone GBA
metadata attempt from the nested temporary worktree hit the outer repository's
workspace discovery error; no manifest workspace changes were made to bypass it.
The two GBA lockfile revisions were checked by the byte comparison above.

The engine PR must merge before this game dependency update; both PRs require
passing CI at their final heads. The game's GBA build and performance gates run
in its CI; the local consumer checks above do not claim a GBA game build.
