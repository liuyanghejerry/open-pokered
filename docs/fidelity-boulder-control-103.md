# Boulder completion Joypad boundary

Original hooks (same pinned reference and 3F fixture as fidelity102): Joypad at73, DisplayTextID at74, DrawStartMenu/initial sound at94. A held START from71 and a single START pulse at73 both reach DisplayTextID74 in repeat-identical source probes. Intermediate native dc6c88d opens START77 for the held case.

The core now samples physical buttons at MoveSprite+70, clears the previous sample like DiscardButtonPresses, and handles that captured sample across the next hardware boundary. It does not require the button to remain held until the last LCD image disappears. The matching boulder image continues to the existing +72 boundary, including during UI takeover. The control_ready predicate uses logical completion rather than retained image lifetime. The sampled handoff is cleared on map changes.

Actual current runtime: both one-frame START73 repeats enter StartMenu74, with event73, retained boulder73/74 and no retained boulder75. Actual master31b1eda drops the same pulse in both repeats. Master's older clock requires its original one-frame turn+idle preparation; the newer clock requires two continuous hardware frames. Pretrigger RGB/coordinates match, and all201 recorded raw input bits match. A failed before setup with two-frame preparation had already pushed the stone; that capture is rejected and its failure retained.

![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-boulder-control-103/start-pulse-before.png)
![后](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/fix/fidelity-overworld-cadence-91/docs/screenshots/fidelity-boulder-control-103/start-pulse-after.png)

Both shots are t74 after the same trigger input, showing the combined pending change against actual master. Four directions and3F hole with no START each match original201RGB frames in both repeats; native PNG/JSON repeats are identical. START timing is supported by source hooks, not by a full menu RGB comparison: earlier capture scripts compared START recordings to the no-START original, an invalid input comparison, retained as rejected evidence.

Validation: core2720 passed. The first complete app run passed194 and failed its legacy assertion that control_ready must remain false until the final image; that test now checks the original logical boundary. The final optimized full app rerun passed195,17 ignored (7.18s). Source menu initialization still takes20 frames between DisplayTextID and DrawStartMenu; native menu input availability during that interval remains a separate open audit. Victory Road2F startup, player presentation, NPC/script/connection clocks and latest fresh playthrough remain open. This is not a full fidelity verdict.
