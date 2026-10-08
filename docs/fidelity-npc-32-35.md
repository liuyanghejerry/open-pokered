# NPC interactions: fidelity findings 32–35

Reference: pret/pokered `fbcf7d0e19a3a2db505440d3ccd3d40ca996c15c`.
Base: open-pokered `4af9cde8d47c7dff21dc0721924f56f7cea834d8`.

32. SilphCo2F TM36: first delivery ends after the receipt; the SELFDESTRUCT explanation belongs to subsequent conversations.
33. Daycare: cancelling the party selector says “All right then, come again.” without depositing or playing a cry. HM rejection and the last-mon guard keep their separate responses.
34. Museum admission, Daycare withdrawal and MtMoon Magikarp sale show the current money before confirmation, refresh after payment and clear it on exit. GameCorner's exchange counter shows **coins**, as in `GameCornerDrawCoinBox`; it does not show a money box or add a purchase sound. Balance information is functional coverage; font and dialogue-box layout fidelity remain excluded.
35. Receipt fanfares now match reachable original gift paths: Route1 Potion, Town Map, Poké Flute, Old Amber, TM28/29/36/39/41, GameCorner's three coin gifts and the eight gym TMs (including retry after a full bag). Daycare deposit/withdrawal uses species cries, independently of nickname; withdrawal plays PURCHASE before the return message and moves the mon after that message. Badge sounds preserve the original bank mistakes: Brock/Giovanni use LEVEL_UP; Misty/Sabrina/Blaine use only BALL_POOF's noise stream. Surge/Erika/Koga do not gain an invented badge sound. TM24 and TM06 retain GET_KEY_ITEM; other gym TMs use GET_ITEM_1.

## Verification

- Core + audio: 3,204 tests passed; one doc test ignored. Includes native success/repeat/refusal branches, balance lifecycle, snapshot serialization/restoration, nickname-independent cries and badge sound channels/lifetime.
- Shared app with debug-server: 160 passed, three ignored harness/capture tests.
- Data with Boa fallback: 439 passed, one doc test ignored. Both interpreters expose the same new capabilities.
- `scripts/fidelity_npc_receipts.py DRIVER ARTIFACT_DIR`: production runtime, new-game save template, collision-checked initial fixture positions, actual inputs. Confirms cancellation leaves two party mons, TM36 is delivered once, payments debit 50/100/500/1000 correctly, Magikarp/Daycare add the returned mon and the counter adds 50 coins.
- Six before/after captures under `docs/screenshots/fidelity-32-35/`. Before captured with master checked out; after on this branch. Same map, player position and absolute frame (1800; Daycare withdrawal 2400). `--before` captures the base without asserting the repaired behavior.
- Fresh power-on m01–m49 passed, including ending autosave and separate-process CONTINUE. First attempt exhausted the three permitted MtMoon blackout retries; second attempt passed without changing the driver or supplying a checkpoint. Both outcomes were retained locally. The prior MtMoon/Tower/VictoryRoad navigation failures remain an audit item; a passing retry is not proof that no navigation bug exists.

This batch is not an exhaustive fidelity claim. The continuing goal excludes fonts, Chinese, pinyin input and dialogue-box layout, and still requires broader battle, persistence and event coverage.
