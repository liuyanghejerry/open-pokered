# Field typing controls: standalone split 202

Only held A/B gradual printing and the final letter delay are repaired. Short A/B
while a field message is printing previously completed the entire page and emitted
TextAdvance. It now shortens letter waits to one frame without scrolling or closing
the page. Finishing a page includes its last letter's 1/3/5-frame speed wait.
Direct/scripted/automatic-item/nested gift/reading-menu paths share these controls.
Unit tests cover A/B, script/non-script, no scroll sound and release before closing.

Core2709 and app190 pass, with17 app capture tests intentionally ignored. The
owned capture runs real CONTINUE from controlled original SRAM, ends the fixture
Safari session, warps to the information worker at(3,4), reseeds field RNG1 after
loading, faces left and uses Medium text speed. Trigger A at t0 is released at t1;
the two-frame A/B pulse is at t12/13 and released at t14. Idle has no second pulse.
Same inputs and frame numbers are used on actual master7d5fb20 and the repair.
Master gets30 characters at the pulse; repair gets6 at t14. Master displays the
ready arrow at t89; repair waits through the last glyph until t92.

All145 frames of all three cases on both sides were repeated twice. All1740PNG
and full raw JSON repeats match byte-for-byte without masks. Every frame compares
retained rendering against a complete draw at every pixel. The original160 raw
recordings were independently re-read: first19 glyph counts match the repair
relative to the first glyph; original Medium final glyph wait is3 frames. Original
first glyph is t24 and native t2: absolute entry CPU/font/PPU timing is not claimed.
Native still needs acknowledgement before the following choice; original inner
PrintText automatic return is a separate pending fix. Question retention, global
NoDelay call sites, dialogue initialization/closing, movements and PC flows are
excluded. No font/layout change or global RNG/audio-waveform parity is claimed.

Native frozen tests/captures are pinned to d02b558 with master7d5fb20. The later
merge of master25c56ca adds the independent quantity fix; all three typing/capture
source SHA256 values remain unchanged (asserted when packaging). Current-head CI
must independently validate that combination, including unchanged GBA budgets.
Evidence contains original-only historical records, primary assembly, fresh raw
recordings, frozen binary/source hashes and tests. Failed a2 capture-helper import
build log is retained as a diagnostic; passing a3 supersedes it. No ROM or binary
is distributed in the archive.
