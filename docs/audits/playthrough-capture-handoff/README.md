# Playthrough capture handoff

The ordinary fresh run on `af6a12f8` passed m01–m27, then stopped in m28.
An A acknowledgment at the end of SafariMenuPreparation selected the default
SAFARI BALL before the driver observed the menu. Doduo was caught; the driver
returned as soon as the screen changed from battle to Pokédex. Navigation then
sent movement while the capture entry was still open.

A capture is complete only after its dex entry and nickname/transfer continuation.
Follow the exported `capture_phase`, acknowledge Entry with A, and use B for the
other pages to decline the optional nickname. Fail if the bounded allowance ends
with a capture still active, or an unexpected nickname editor is already open.
Use B for intro acknowledgments so crossing into a menu cannot select its default
FIGHT or SAFARI BALL action.

The new tests first reproduced three failures. All 117 playthrough tests now pass.
The actual modified `Game.battle_loop` also completed a real Master Ball capture
through Entry and Choice to overworld, preserving the caught Rattata and consuming
the ball. This used an explicitly seeded isolated fixture on the same pinned game
binary; it is not a replacement for a fresh full playthrough. Hashes and observed
frames are in `verification.json`.
