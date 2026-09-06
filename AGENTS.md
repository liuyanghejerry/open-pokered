# AGENTS.md

Guidance for AI agents contributing to this repository. Complements `CLAUDE.md`
(architecture, build/test commands, debug CLI) — read both.

## PR policy: visual changes require before/after screenshots

Every PR that changes on-screen output must attach **before/after comparison
screenshots** to the PR description. This applies to all contributors, human
or agent. A change counts as visual if a player would see anything different
in a frame.

**In scope** — screenshots required:

- Menu, dialogue, textbox, and HUD layout (including `.gui` layout changes)
- Fonts, glyphs, and text rendering; tile/GFX assets
- Palettes, colors, fades, and flashes
- Battle and overworld rendering, animations, transitions
- Any renderer/frontend code whose behavior shows up on screen

**Out of scope** — no screenshots needed: pure game logic (damage calc, AI,
scripting, save format), tooling, docs, tests, audio.

**How to capture:**

1. Check out the base branch (`master`), capture the "before" frame.
2. Check out the PR branch, capture the **same screen and frame** as "after",
   so the diff is attributable to the PR alone.
3. Prefer the headless screenshot CLI (no window, deterministic):
   ```bash
   cargo run --release --bin pokered-app -- screenshot --screen battle -o before.png -f 10
   cargo run --release --bin pokered-app -- screenshot-all -o shots/
   ```
   Screen targets: `copyright title main-menu oak overworld battle
   start-menu options save`. For input-dependent states, drive with the debug
   server (`run --headless --debug-port` + `press_sequence` / `step_frames`;
   see `.claude/skills/pokered-debug`).
4. Commit the captures under `docs/screenshots/` (existing convention) and
   embed both images in the PR body, labeled `前` / `后` (before / after).
   For regression fixes, the "before" shot doubles as the bug evidence.
5. **Embed the images with absolute raw URLs — never relative paths.**
   GitHub does **not** resolve relative image paths in a PR body against the
   PR branch: while the PNG only exists on the PR (not yet on `master`),
   `![前](docs/screenshots/x.png)` renders as a broken image in the PR
   conversation. Reference the raw URL pinned to your branch instead:

   ```markdown
   ![前](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/<branch>/docs/screenshots/x-before.png)
   ```

   When drafting the body with `gh pr create --body`, rewrite the paths
   mechanically before submitting (and re-check after any `gh pr edit`):

   ```bash
   BRANCH=$(git branch --show-current)
   perl -pe "s{\]\(docs/screenshots/}{](https://raw.githubusercontent.com/liuyanghejerry/open-pokered/$BRANCH/docs/screenshots/}g" body.draft.md > body.md
   gh pr create --body-file body.md
   ```

   Branch-pinned links are the review-time source of truth; the merged
   copies under `docs/screenshots/` on `master` remain the long-term
   archive even if the branch (and its pinned URLs) is later deleted.

If a changed surface genuinely cannot be captured headless, say so explicitly
in the PR description instead of skipping the rule silently.
