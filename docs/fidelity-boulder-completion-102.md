# Boulder completion: logical state and LCD image

Pinned original: pret/pokered fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c. The controlled Victory Road 3F fixture uses the normal map loader, original object10 at22,15 and player21,15, actual party Strength and a16-frame Right input. Both original201-frame captures are repeat-identical. Primary hooks record MoveSprite at3, LoadPlayerSpriteGraphics at69, SFX_CUT (PlaySound172), HideObject and ShowObject at73. The displayed stone disappears at75. This distinguishes WRAM/script state from the retained OAM/LCD image.

The developing branch committed events and SFX_CUT at75, matching images while leaving logical state two frames late. BoulderPushState now defines COMPLETION_FRAME70 relative to MoveSprite. Seafoam/3F hole events, object toggles and the completion sound occur at that boundary, while rendering retains the boulder's previous image through LAST_FRAME72. The renderer only retains the matching active boulder, rather than revealing other hidden NPCs.

Core regressions check both event/visibility boundaries and the single completion cue; the existing downstairs restoration test now expects the source-derived logical boundary. The first full core run failed its old frame72 expectation; that failure is retained in boulder-102-core.log, rather than counted as a passing run.

This is a staged correction. Player control is still blocked until the end of the retained image; original Joypad resumes at73 and DisplayTextID at74 for held START. The 2F startup discrepancy and ordinary player sprite presentation also remain open. No full fidelity verdict or merge is authorized by these narrow results.

Evidence paths: docs/screenshots/fidelity-boulder-completion-102/manifest.json and raw-captures.zip , with the original primary sound/control hooks retained. Current validation: core2719 passed; app195 passed,17 ignored. Four directions and3F hole each201RGB frames match original in both repeats; all PNG/JSON repeat-identical. Metadata-enabled actual runtime records first event and logical hide at73; frames73/74 retain the source LCD image and75 hides it. Overall combined-PR before/after remains the actual master31b1eda comparison under fidelity-boulder-startup-99.

Subsequent [fidelity103](fidelity-boulder-control-103.md) addresses completion-time Joypad and START processing. Menu initialization latency remains open.
