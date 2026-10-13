# Wait for healing music

AnimateHealingMachine polls wChannelSoundIDs until MUSIC_PKMN_HEALED
ends, then calls DelayFrames(32). A fixed 120-frame wait after flashing
cannot follow live music termination.

Use the existing app-to-core script_music_playing channel-1 state, then
enter a separate 32-frame PostMusicDelay phase before removing the overlay.
The initial eight palette intervals are unchanged by this patch.

Two original recordings and two native PCM-sequencer recordings are exact
within each implementation. Both songs end after 136 frames, and both hold
the overlay for 32 further frames. Native start/end anchors differ from the
original; fade and command-dispatch timing are not claimed equivalent.

Before and after show the same five-member fixture and input at frame 964.
The base is the preceding interval fix (#204). The old fixed wait still
shows machine sprites; the corrected live-music wait has completed.
Regression keeps music active for over 120 frames, verifies no early exit,
then checks all 32 frames after termination. Full core/app suites pass.
