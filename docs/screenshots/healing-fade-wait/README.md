# Wait for the live music fade

The original healing machine starts FadeOutAudio(4) and waits until
wAudioFadeOutControl clears. Actual original recordings show 40 hardware
frames until the first ball. The app audio engine also needs 40 frames, but
the old script began healing after its fixed 32-frame counter plus a zero
frame, while the music was still fading.

Expose is_fading on native/GBA audio outputs, feed script_audio_fading to
the core, and wait on it instead of a fixed count. Runtime snapshots retain
this flag and older snapshots default to false. Nested effects receive the
same live state. The capture fixture starts center background music before
its warmup, on both the base and fix, and requires a live channel.

Same input and frame 646: the old runtime already shows a healing ball;
the fixed runtime waits for the fade to complete. Both fixed continuous
recordings match exactly. The fade/ball interval is 40 frames in both the
original and fixed runtime. This does not prove absolute scene timing or
map-music restart parity, which still need audit. Base includes #205;
refresh against merged master before merging this dependent patch.

Core regression checks a fade longer than the former timeout, snapshot
restore/legacy default and immediate progress after completion. Core 2798
and app 229 full library tests pass.

CI follow-up: sample the fade only during the healing fade wait. Unrelated
audio histories must not leak into the logic snapshot, and idle frames need
no additional manager query. The unchanged agent determinism assertion now
passes, all 41 app test executables pass, and the healing manifest and all
1285 PNGs are unchanged. GBA performance remains gated by latest-head CI.
Full audio-engine restore during active healing still needs validation.
