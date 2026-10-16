# Tower 6F Marowak departure caller

The mother-identification window now stays open while the real Marowak cry
finishes, then for 30 further frames, before the calmed/departed text replaces it.
Only this caller opts into window retention. Existing `playCry` calls remain
nonblocking; `waitSound()` reads the existing logical SFX sequencer input.

## Original caller

At original source commit `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`,
[`PokemonTower6FMarowakDepartedText`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/scripts/PokemonTower6F.asm#L137)
prints the mother text, calls `PlayCry(RESTLESS_SOUL)`, waits for sound completion,
delays 30 frames, and prints the departure text. The
[mother text ends in `done`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/text/PokemonTower6F.asm#L1).
[`DoneText`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/text.asm#L221)
returns to the caller; the outer
[`DisplayTextID`](https://github.com/pret/pokered/blob/fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c/home/text_script.asm#L87)
waits/closes after it returns. The mother window therefore survives the cry and
30-frame delay without another final-page A acknowledgement.

`printFieldText(text, true)` explicitly owns the retained window. The optional
argument defaults to false in native, Boa and typed command conversion, and in
old snapshots. Subsequent text replaces ownership; script completion, reload,
cancellation, warp, connection, save loading and the direct soft-reset entry
clear it. No cry duration is hardcoded.

## Current source and checks

The final matched pair uses **actual master**
`0820a522ded9c9dd6a1958e372d60ccf2adb3940` and actual candidate
`087ebfd0cc7ae3a35b255c8d5f0795428101fc0d` containing that master through a
normal merge. Their entire non-document/compiler rosters, engine checkout and
702 external GFX files were frozen before and after builds. The Tower
non-document patch is byte-identical to the earlier isolated f35-to-a29 patch.

| Actual command scope | Actual source | Result |
| --- | --- | --- |
| Baseline debug CLI build | 0820a522 | exit 0 |
| App `field_cry_wait_fidelity_tests`, `debug-server` | 087ebfd0 | 2 passed, 0 failed |
| Candidate debug CLI build | 087ebfd0 | exit 0 |
| Cold-boot, ordinary-input native replay | 0820a522 / 087ebfd0 | both completed 1945 frames; owned processes terminated |

The App tests use the real Game request drain and logical PCM under
`no_audio=true`. One checks the retained window through actual cry completion
and exactly 30 additional frames. The other reaches that wait, directly calls
`soft_reset()` with an unreadable save destination, and checks the owner/cache
are cleared. All builds use offline/locked, one job, no incremental builds and
this worktree's dedicated target, with public engine
`e533a3d23a0cdbed1524a1ac91184733da4a6f7e`.

Historical development checks remain attributed to their actual source: full
core at `c6a4c940` passed 3405 (2993 unit + 412 integration; 1 doc test ignored),
its focused core filters passed 2+1+1, optional data argument passed 1, and Boa
API tests passed 13. Those full-core results are not relabeled as an 087e rerun.
The first c6a4 App run failed at parsing a newly authored DSL fixture. The
`e709eff5` child corrected only nine indentation lines; its App test/build passed.
`be89c290` then added the direct-reset cleanup and its second regression. Its
App2/build also passed. These old logs and the historical f35/a1/e709 and
a393/5856 native pairs are preserved separately in the provenance.

## Ordinary-input native comparison

The committed [fixture and inputs](fixtures/marowak-departure-1048/provenance.json)
use a preboot SaveData JSON at Tower 6F `(10,17)`: all three Channelers beaten,
Ghost Marowak unbeaten, one standard level-30 Bulbasaur at HP 73/73 and PP
35/40/0/0, and one Poké Doll. It was built offline with SaveBuilder from an
engine-exported canonical save; all changed leaves and byte hashes are recorded.

Both processes cold-boot normally with `--snapshot`, `--headless --speed 0`,
`--seed 42 --lang en --no-audio` and their own 968 byte-frozen runtime map files.
Ordinary buttons pass splash/language/title/Continue, walk onto the real
coordinate trigger, acknowledge the ghost encounter and select ITEM → POKé DOLL.
The same immutable 1945-frame button vector and ten capture frames are used
once per process. Client TCP_NODELAY only changes transport latency; input
frames and game speed are unchanged. There are no live setters, skip helpers,
warp, encounter injection, runtime RNG changes, restore or automatic retries.
No physical SAVE is represented as having occurred.

Before CLI SHA-256: `7c9b2d5e3dd4f4e18351f3f16aea47ac7f548847aecc9edac25b920a78a15ede`.
After CLI SHA-256: `5aec6adae768a0f9399a83849541121c2c33f4fcccc6576eb9e9f80ee9f90957`.
Each binary and its runtime maps come from the actual stated source head;
there is no equivalent-source substitute in this final pair.

All 1946 public observations are exactly equal through frame 1597. The first
intended difference is at frame 1598, `ShowDialogue` → `PrintFieldText`.
Every other public field remains equal for all 1946 frames, excluding only
`active_script_effect`, `script_effect`, `dialogue`, `dialogue_state` and
`audio_sfx`. This includes party, battle HP/PP, inventory, money, badges, map and
position. It is not direct RNG-state equality or original hardware phase proof.

| Absolute frame after update | Before | After |
| --- | --- | --- |
| 1677 | Mother text awaits final A | DONE has returned and installed PlayCry; window retained |
| 1678–1719 | Moves to calmed text after A | WaitSound; actual logical cry id 27 is playing |
| 1702 | Calmed text already appearing | Complete mother text remains visible |
| 1720–1749 | Calmed text awaiting input | Exactly 30 Delay states, remaining 30 down to 1; SFX idle |
| 1722 | Calmed text visible | Mother window retained, delay remaining 28 |
| 1750 / 1751 | Calmed dialogue | Next ShowDialogue installed / next text pending |
| 1945 | Script idle, window closed | Script idle, window closed |

The frame-1702 and frame-1722 screenshots were inspected at their original
160×144 size and copied without image manipulation. Of ten capture pairs,
six are byte-identical; only frames 1677, 1702, 1722 and 1762 differ. The final
frame-1945 pair is byte-identical. Subsequent ordinary A presses page through
and close the departure text. No separate after `get_flags`/`export_fixture`
query was performed, and logical sequencer observations are not speaker audio.

Latest raw-pair analysis SHA-256:
`290760507748515bbbee32d796564a536ed776fbb317078fb81ccfe3938f1914`.
Current App2/CLI result SHA-256:
`b50f03225c5d30e3cab8253155efa8eb85ed686a6583b747b0562b38eecd9933`.
Full raw RPC/state streams, immutable inputs, command argv, source freezes,
terminal receipts and the initial failure remain in the session audit records.
The committed provenance records their identities without relabeling old runs.
