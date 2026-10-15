# Transform capture rate

Ordinary ball throws now use the original wild encounter species' header catch
rate. The original stores this in `wEnemyMonActualCatchRate`, separately from
the catch byte in the battle/party record. Transform copies the latter byte;
it does not replace the independent rate used by `ItemUseBall`.

The caller reads the original party identity through `party_mon`, including
while a different species is displayed after Transform, and uses the header
rate with the existing fallback of 255. Transform still copies the working
catch byte. Safari keeps its separate catch-rate path. The original bug that
rebuilds any captured transformed wild Pokémon as Ditto, including a non-Ditto
Mirror Move user, is retained. Save fields and the capture/RNG algorithms are
unchanged.

The original reference is `pret/pokered` revision
`fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`: `ram/wram.asm:1196–1199`,
`engine/battle/core.asm:6107–6121`,
`engine/battle/move_effects/transform.asm:57–71`, and
`engine/items/item_effects.asm:300–303,329` distinguish those two bytes.

## Core caller coverage

Validated source: `e23eda5daf60a1455f19ee8f1f51ac1ffd4fe863`, based on
`e6150de4cf06bfd8a8bbbaf3e65ceb8ecb6e0ce5`, using engine
`e533a3d23a0cdbed1524a1ac91184733da4a6f7e`.

Three tests in `battle/transform_capture_tests.rs` cover four fixtures:

- A real canonical enemy Ditto Transform copies 255 from a Gyarados produced
  by Magikarp evolution, which preserves Magikarp's stored catch byte.
- A real canonical enemy Ditto Transform copies Snorlax's lower value, 25.
- An untransformed wild Rattata supplies the ordinary rate-255 control.
- A faster player Ditto actually uses Transform before a wild Pidgeot uses
  Mirror Move in the same canonical turn. Capture uses Pidgeot's original
  rate, 45, while a successful capture still produces Ditto.

After the canonical effects and narration, each fixture uses the public battle
menu and Bag caller to throw a ball. Every seed from 0 through 255 runs for
Poké, Great, Ultra and Master Balls at full HP and at 1 HP: **8192 throws** in
each complete three-test run. There is no search for a favorable seed or
early successful-seed exit.

The reference capture context uses literal original rates (Ditto 35, Pidgeot
45 and Rattata 255), independently of the changed header lookup. Assertions
compare the exact Ball outcome/shake event, full captured record or absence,
one-item consumption, narration continuation, eight subsequent RNG bytes and
the complete serialized RNG state. The copied working catch byte remains
intact.

## Actual results and negative control

The first focused run passed all three tests. The complete `pokered-core`
suite then passed **3384 tests**: 2972 unit tests and 412 integration tests,
with zero failures and one ignored doctest.

An uncommitted negative control replaced only the caller's rate expression
with the exact old reader; tests, assertions and vectors stayed unchanged.
That run exited 101 with two controls passing and the expected Ditto test
failing at its first fixed vector: Poké Ball, seed 0, full HP, high-255 copied
byte. Both results were `BrokeFree`, but the old reader produced **two shakes**
against the literal rate-35 reference's **zero shakes**. This first assertion
does not demonstrate an incorrect capture at that seed. It stopped the main
negative test before the later high-value vectors or the low-25 fixture ran;
the untransformed and Mirror Move controls completed successfully.

Restoring the committed source byte-for-byte made the same three tests pass
again, including the complete 8192-throw matrix. The earlier full-suite result
was preserved without repeating that suite. All source, engine and GFX
freezes matched after restoration, and both negative and restored-source Cargo
process groups exited. A later documentation-only commit preserves these exact
validated non-document source bytes.

These are deterministic core-caller tests with unit fixtures, including an
explicit speed fixture for the Mirror Move turn. They are not a native Game
playthrough, original-emulator replay, App/CLI/GBA build, or official CI run.
The change is pure core logic, which falls under the repository's screenshot
exception.
