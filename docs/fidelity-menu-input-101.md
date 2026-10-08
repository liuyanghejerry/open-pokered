# Full Joypad sample after UI return

Original Joypad calls from menus replace hJoyHeld, which JoypadOverworld's RunMapScript reads before the next Joypad call. The developing combined branch only synchronized A/START on UI return. A cached Down from opening START with Down could survive closing with B, then trigger Strength against the boulder without new d-pad input.

The shared runtime now seeds the entire previous physical UI button sample (up/down/left/right/A/B/START/SELECT), preserving newly pressed field buttons and suppressing held UI confirmations. The core exposes full-sample synchronization while retaining the previous A/START convenience API. A regression covers the menu-state handoff and absence of a push or new dialogue.

## Actual runtime and original reproduction

Actual source Seafoam SRAM → Title/Continue → actual party Strength →120 idle. Hold START+Down2frames, release and wait60 in menu, press B2frames, release and wait6, then record201 frames with no direction. Identical capture helper runs on intermediate444e2d7 and the repair. Before both repeats push the stone18,10→18,11 despite no new direction; after both repeats leave18,10 unchanged. Original MoveSprite never occurs. Each native repeat matches201PNG/JSON byte-for-byte; original repeats match201PNG/JSON/events. After matches every RGB pixel of the original201frames; before differs on200frames.

The before shot is explicitly the intermediate development regression. Overall PR review still uses actualmaster31b1eda before/after screenshots under fidelity-boulder-startup-99; this handoff bug was introduced and repaired within the same pending combined change. Archive/manifest retain source hooks, exact inputs, both repeats and driver hashes.

Validation: current core2718 passed; debug-server app195 passed,17 ignored. Continuous A is represented with begin_frame between held frames rather than synthetic fresh edges. Later capture-helper extensions are readonly; production is the tested full-sample handoff.

Other timing discrepancies remain open, including Victory Road2F startup, boulder end events/audio/control and player presentation. This is not a completed full fidelity verdict.
