# Safari encounter intro

Safari encounters now keep RED’s back picture, omit the party Pokémon HP HUD,
and enter the BALL/BAIT/ROCK/RUN menu after the original explicit, unskippable
40-frame `StartBattle` delay. The extra “Go!” prompt, Pokémon send-out animation,
and persistent party-ball row are skipped. The temporary party balls in the
wild-appearance text remain.

The original selects `RedPicBack` and branches from `StartBattle` directly to
`DisplaySafariMenu` for battle type 2. Ordinary battles and the Old Man tutorial
have separate paths. Existing `gfx/player/redb.png` matches the original asset;
no new graphics are included. Capture continuation/audio and the last Safari
ball message are separate changes.

`verification.json` records frozen builds, actual input/SAVE controls, exact
40-frame boundaries, regression baselines, full test and GBA checks.
`screenshots.json` records a same-fixture/seed/input-prefix/simulation-frame pair.
`fixture.json` is the complete preboot modern snapshot used by the native trials.
Both builds share the complete input prefix through frame 2703. BEFORE then
needs one more A plus 300 neutral observation frames before RUN; AFTER goes
directly to RUN, so those later input sequences are not claimed identical.
The resulting SRAM clock/checksum differences are recorded without normalization.
It is a constructed active Safari Center visit, with 30 balls and 100 steps;
these trials do not test admission/payment or a fresh full playthrough.

The original trials use an explicitly documented preinput bootstrap plus party,
box, player, dex and event facts from a normal native SAVE. After construction,
validation uses only normal walking/menu/RUN/SAVE inputs and read-only hooks.
Source and native natural encounters differ (NIDORAN♂22 / VENONAT22); enemy pixels,
DVs, encounter entropy and whole-intro timing/trajectory/audio-waveform parity
are excluded. The source hooks reach `StartBattle` 8 frames after the text ACK,
then reach the menu after its explicit 40-frame delay. Only that explicit delay
is implemented here; the measured total of 48 is not a runtime constant or a
whole-clock parity claim.

The compact `evidence.zip` retains the original input logs, one PNG per physical
frame, frame manifests, read-only observations, normal SRAM saves, source/build
identity manifests and excluded failed controls. ROMs, executables and emulator
states are excluded. Verify it without extraction:

```sh
python3 extract_evidence.py evidence.zip --verify-only
```

Replace `--verify-only` with an output directory to restore the original paths.
Both content hashes and byte lengths are checked. The detailed source bootstrap
state remains private because emulator states/retail ROM data are excluded.

Latest tested integration: master `216cb034` merged at `3bd9ed30`; full core
2,889 and app 281 tests passed (39 ignored), with all seven GBA performance
scenarios inside the existing budget (trainer update 232 ticks). The two Rust
files changed by this fix remain byte-identical to the frozen `6abff9bf` capture
build. Historical captures retain their 6085/6abff identities.
