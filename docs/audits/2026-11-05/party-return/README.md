# Party menu return and SWAP audio

Code commit `ff00099a`, stacked on FLASH PR #324 (`9fc696cf`) and master
`303aaceb`. This extracts the remaining Party return behavior from #144; it
does not assert that every other change in that old mixed draft is covered.

Cancelling the ordinary party menu plays the accepted PressAB sound, then restores the saved START background,
sprite patterns and map palettes before accepting menu input. A short Down/A
pulse during restoration is discarded; a direction held until the next real
poll moves the START cursor once. Holding the closing B does not close START
again. Returning from bag medicine still goes directly back to the bag.

The shared restoration work uses the original `StartMenu_Pokemon.exitMenu`
call order, retaining the foreground input history and current cave darkness.
It omits the extra `CloseTextDisplay` work used when FLASH returns to the bare
map. Safari counters and the native START tile namespace survive the transfer.
There is no fixed white-screen countdown or fitted white strip.

In the original game a two-member party can finish SWAP and exit while the
second swap pulse is still active. The source audio cursor now handles both
finite SWAP streams, including a simultaneous PressAB on the first pulse.
The independent ROM oracle checks 30 note/end cases in banks 2 and 31, every
instruction duration and 126 APU accesses. Each instruction is also checked
after refusal and serialization; complete manager tests check native state,
partial continuation, input rejection and exactly one audio commit per frame.

Validation uses frozen native binary SHA-256
`58088488b46d7b3e10f7d02cb41ff401ae8c9cf085d9008c87fea19e2e52bedc`.
All compiled source-file hashes match the code commit. Application 467 tests,
core 3,981 tests, audio 222 tests, Python driver 11 tests, and ARM check pass.
The existing ignored tests remain ignored (40 application, 1 core).

Prepared-save controls:

- Current master and candidate: neutral, short and held input, two runs each.
  Their initial images and frame numbers match. Master incorrectly enters the
  bag for the short pulse; the candidate remains at START.
- Bedroom, dark Rock Tunnel and Safari: nine cases, each repeated twice,
  plus 18 independent saved CONTINUE runs. Darkness, Safari steps and balls
  remain unchanged. All repeated state and image records match.
- Two-member action-menu SWITCH followed by immediate B: two runs, followed
  by two independent CONTINUE checks retaining the exchanged party. PCM audio
  stays active even though host audio output is disabled.
- Default release GBA ROM: ordinary cold SRAM boot and physical key inputs,
  neutral/short/held controls twice each, all 600 frame hashes repeat. Neutral
  and short controls restore exactly the saved START image; held Down selects
  ITEM. No autopilot, frame skipping, or recording-time RAM writes.

**Remaining limitation:** default GBA restoration takes 227 physical frames
(3.801 seconds). This is not original-speed parity. The original/native LCD
entry phases are not claimed identical. The latest complete NEW GAME through
m49, ending autosave and independent CONTINUE is still running; prepared saves
are not counted as a fresh playthrough verdict.

See `verification.json` and `docs/screenshots/party-return-1390/` for compact
evidence. Before/after captures use identical inputs and capture indices;
the compiled master baseline's full crate/Cargo contents match `303aaceb`.
