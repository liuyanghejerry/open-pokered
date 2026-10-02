# Captain's back-rub music wait: continuous-playthrough regression

The prebuild continuous run passed m01–m15, then exhausted 300 cutscene rounds
at the captain. The retained state shows the player at (4,3), facing Up,
the captain at (4,2), and `active_script_effect: WaitMusic`. The interaction
and driver positioning succeeded; the dialogue had already closed.

`--no-audio` creates a PCM-only AudioOutput, and `PokemonGame::update_inner`
still calls `audio.update_frame()` on every game frame. The missing device
was therefore not preventing sequencer progress.

The engine's `Sequencer::music_playing` is set by play/stop requests and is
not cleared when a finite music channel reaches sound_ret. The previous
frontend passed `AudioManager::is_music_playing()` to WaitMusic, so the latch
remained true after the healed jingle had finished.

Original `scripts/SSAnneCaptainsRoom.asm:61-65` waits specifically for the
first music channel's sound ID to cease being MUSIC_PKMN_HEALED, then starts
default music. The app and TUI now read the sequencer's live first music
channel via an AudioOutput method, implemented for both host and GBA output.
No timer replaces the original audio completion condition.

A standalone Rust probe linked the prebuild cached pokered-audio library
and played its real PKMNHEALED byte streams. Its output was:

```
healed CHAN1 ended at frame 136; global music_playing=true; id=Some(PKMNHEALED)
```

The probe demonstrates the failing latch and the correct completion signal.
New regressions play the real healed stream and exercise the actual app's
no-audio WaitMusic feedback until that channel ends. Cargo and the continued
playthrough are run by root after integration; they were not run in this
worktree because the shared build queue is reserved for the final build.
