# Battle move maximum PP verification

FIGHT now uses the existing `get_max_pp_with_ups` calculation for each move's original physical slot. The PP-Up count is masked to its two serialized bits. Each PP Up adds `min(base_pp / 5, 7)`, matching original `GetMaxPP` / `AddBonusPP`.

Transform writes copied working move slots with 5 PP and zero PP Ups. The existing original-identity owner retains the real party move/PP-Up data for switching back. Mimic continues to retain its original slot's PP Ups.

Original source: `pret/pokered` revision `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`, `engine/items/item_effects.asm:GetMaxPP/AddBonusPP`, `engine/battle/core.asm:PrintMenuItem`, `engine/battle/move_effects/transform.asm`, and `engine/battle/effects.asm:MimicEffect`.

## Source and builds

- Actual base: `4211f425105840c53bb0cb7bc27afcc955a5e5b1`.
- Tested fix: `2623c94543149ad4083b96746d28333726a6a6e5`.
- Before uses the preserved `9624c7f8e4f793a57abd4c940b9a8d2a8b99fdee` build, binary SHA256 `065913373a2561d015db20382bb163448b2ba63652de46cf23d41c2768c54245`. All 2175 compiler inputs (modes, Git blobs, literal bytes) and all 702 local GFX files were checked equal to the actual base; the original build identity is retained.
- After is a new build of the tested fix, binary SHA256 `991f215db27d0c59b3006e6b21b9021d1b006eb0a304575623dda0f4f2547861`.
- Both use the unchanged engine revision `e533a3d23a0cdbed1524a1ac91184733da4a6f7e`.

## Checks

- `cargo test --locked -p pokered-core --lib`: 2949 passed, including all five new `max_pp_menu_tests` exactly once.
- `cargo test --locked -p pokered-app --lib --features debug-server`: 334 passed, 39 ignored.
- Debug CLI build, new test-file rustfmt check and `git diff --check`: passed.
- Five new cases cover all PP-Up counts and the seven-point cap, zero current PP, physical slot/Disable alignment, serialized bit masking, Transform/switch restoration, and Mimic/switch restoration.

## Matched screenshots

Each version was run once from a private copy of the same 32768-byte ordinary save (SHA256 `e51ac96a99fc89c9822ce2eb00798f5eb96fe13854d909c2d52cdf44f6693bd8`), with seed 42, cold boot/Continue and identical 2121 normal input frames. The last frame is a single Down press. No battle turn was executed.

| Frame | Selection | Before | After |
| --- | --- | --- | --- |
| 2120 | Headbutt, one PP Up | 0/15 | 0/18 |
| 2121 | Amnesia, three PP Ups | 32/20 | 32/32 |

All input receipts report one advanced frame and zero padding. Capture responses and surrounding public state are identical within each capture; capture does not advance frames. HP, current PP, moves, inventory and the private SRAM bytes remain unchanged. Pixel differences are confined to the PP numbers: eight pixels for Headbutt and seventeen for Amnesia. These are deterministic `Game.draw` screenshots.

The complete protocol records, input vectors, process receipts and local review reports are retained in the session's `audit-1032-before-max-pp` and `audit-1032-after-max-pp` evidence directories.

| Capture | SHA256 |
| --- | --- |
| headbutt-before.png | 779209f0eed92bfe48343c0996955963c020b1a8aeb17d6b479e53a7f59a196b |
| amnesia-before.png | 6a00080b9c1890431f468ffd13e4a5e83ceecc677f02939b6e7c88decfc3687d |
| headbutt-after.png | 9879eb8d2e0add0352ca6ed80de4ca80d68b832400d745e046f7511f3c7af9e8 |
| amnesia-after.png | 03cf7e81ba488b0764a23269302ef6cb134e3be59408ac462802d6eeeef7818b |
