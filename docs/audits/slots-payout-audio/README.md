# Slot payout music and sound wait

Original `engine/slots/slot_machine.asm:SlotMachine_PayCoinsToPlayer` sets
`wMuteAudioAndPauseMusic` before `WaitForSoundToFinish`, then pays one coin
every8 frames (4 for bar/seven), including the final delay. The caller clears
the pause flag afterwards. `Audio*_UpdateMusic` freezes music channels while
SFX continues, muting NR51 and resetting the wave DAC once. The sound wait
polls CHAN5, CHAN6 and CHAN8, skips CHAN7, and bypasses an active health alarm.

The native game previously advanced music throughout payout, credited coins
before the previous SFX finished, and omitted the last coin's full delay.
The shared Game now supplies the real pending-sound status and pause state.
Both hosted and GBA outputs use the same audio manager; GBA accounts for
elapsed VBlanks under the old pause state before changing it. The engine
music-only update continues SFX without restarting or caching the song.
Unsupported source-clock admission refuses the native paused state rather
than pretending the unpaused source work describes it.

## Evidence and scope

`original-runtime.json.gz` records two identical normal boot/CONTINUE runs
with physical directional/A inputs and read-only hooks. On the third spin,
15 coins are paid over120 frames. Music pointers and delays remain frozen,
reward SFX is active, and music advances during the60 frames after unpausing.
No original font is rendered. `original-fixture.sav` is an offline prepared
fixture, not fresh travel: source-derived map/music/tileset fields and the
checksum were corrected before boot. Earlier incomplete fixtures were invalid
and provide no gameplay evidence. The original ROM is not included.

Reproduce using the existing PyBoy environment and the matching pinned
`pret/pokered@fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c` symbols/ROM:

```sh
python replay-original.py --rom /path/to/reference.gbc --symbols /path/to/pokered.sym \
  --fixture original-fixture.sav --out /path/to/evidence
```

The shared Game regression is a controlled payout fixture. It presses A with
a real17-frame Purchase SFX still playing, checks deferred coin credit and
frozen music, then verifies persistence and music resumption. The before/after
images show the same frame: A followed by8 neutral Game updates. The old game
already credited coins; the fixed game is still waiting. These checks do not
claim continuous source/native PCM equivalence or fix Hall of Fame loading.

Local checks:22 core slots tests,168 game audio tests,191 engine audio tests,
2 engine doc tests, the shared Game integration test, and locked GBA memory
scenario release checking pass. Full applicable game CI remains required.

The first CI candidate rejected two hardware-timer update averages: trainer
battle entry234 ticks (limit232.3) and Pokédex entry119 ticks (limit118).
It had settled elapsed audio time twice in every GBA Game update. The
follow-up switches pause state only when the payout stage changes, and on
forced scene exit/reset; ordinary frames retain their original single audio
update. The shared Game test also covers forced exit during a live pause.
The full original timing and memory gates must still pass on this follow-up.
