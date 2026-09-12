# Five Months Rebuilding Pokémon Red with AI: Progress, Detours, and Lessons

**中文版：**[用 AI 复刻《宝可梦 红》的五个月：进展、弯路与收获](my-retro-2026-08.md)

In March 2026, I started rebuilding *Pokémon Red* in Rust with AI. I wanted to see how far AI could go when the task grew from a small coding exercise into a complete classic game.

Five months later, the game is playable on macOS, Windows, the web, Android, and iOS. It even has a terminal interface (TUI). My goal is to reproduce the original's behavior and visuals as faithfully as possible. Some animations and details still need work.

![The remake's town, battle, map, and opening screens](images/opening-screenshot-1.png)

*The remake so far: Pallet Town, a battle, the town map, and the opening dialogue.*

**[Play in your browser](https://liuyanghejerry.github.io/open-pokered/) · [View the source](https://github.com/liuyanghejerry/open-pokered)**

A question kept coming up throughout those five months: after AI makes a change, how do I know it got it right? Answering that shaped my choice of models, debugging tools, and architecture. It also changed how I spent my own time on the project.

This is an account of those changes, including the problems I still haven't solved.

## Why Having the Original Code Wasn't Enough

The community's [pret/pokered](https://github.com/pret/pokered) project provides an annotated disassembly, so I could refer directly to the original implementation. The graphics are based on the original assets; the game logic is reimplemented in Rust so it can run independently of Game Boy hardware.

Having that reference doesn't make the translation straightforward. The original spans 1,923 assembly files and roughly 174,000 lines of assembly. Much of it is tied to the hardware, with game logic and rendering intertwined. Changing the language, platform, and architecture meant checking those semantics all over again.

Then there is the scale: 151 Pokémon, 165 moves, and 245 maps, plus trainers, NPCs, items, and story events. The numbers, trigger conditions, and animation sequences all need to match. When designing my own game, I can choose the rules. In a remake, the original's behavior is the reference.

A battle screenshot can look right while the random-number sampling for catches, poison state transitions, or critical-hit table lookups behave differently. A character can reach the correct position without matching the original's movement timing along the way.

At the time of writing, the project had **1,823 commits, 189 merged PRs, and 416,000 lines of code**, covering the game, engine, and toolchain across Rust, web code, DSLs, and Python. Those figures describe the project's scale. Fidelity still has to be checked behavior by behavior.

The work went through four broad stages:

| Stage | Main work | The next question |
| --- | --- | --- |
| Getting started | Extract data from assembly; build data models and unit tests | Do passing tests mean it matches the original? |
| Tools and engine | Separate subsystems; add debugging access and a scripting engine | How can AI check its own work? |
| Story and visuals | Migrate scenes in batches; compare screenshots and animations | How can I handle so much repetitive work efficiently? |
| Playthroughs and finishing work | Run complete playthroughs; check events and details | What does completing the main story still miss? |

The [full timeline](images/timeline-en.png) records the dates, tools, and model changes. I'll focus here on the shifts that made the biggest difference.

## Stronger Models Let Me Delegate Bigger Tasks

Two kinds of work shaped my view of model capability.

Early models were already useful for writing parsers, moving data, and migrating scenes with well-defined formats. I had to intervene much more often when the task was to judge whether two implementations were equivalent. With assembly on one side and Rust on the other, the model needed to understand both and check their edge cases.

When its reasoning fell short, AI could produce a convincing report claiming that the implementations matched. I still had to recheck the important behavior myself. That taught me that **model capability determines which judgments I can delegate.**

Mobile integration made this especially clear. Earlier models repeatedly got stuck on lifecycle and rendering integration when connecting the Rust framebuffer module to Android and iOS. After I switched to Claude Opus, it worked through the same problem in about half an hour. That experience changed how I chose models: when a reasoning task kept getting stuck, I became quicker to try a stronger one.

Vision changed the division of work too. A font shadow displaced by one pixel or a misaligned menu cursor used to require me to inspect screenshots and describe the difference to AI. Pixel comparisons could highlight differences, but I still had to interpret them and guide the fix. Once models could analyze screenshots directly, I could gradually hand over more of that work.

The coding agent environment, often called the **harness**, mattered as well. It runs the model, manages context, and connects tools, affecting how long the agent can keep working productively. I used four main environments:

| Environment | What I could delegate | My main role |
| --- | --- | --- |
| Cursor / VS Code, March | Changes to individual files | Review every diff and organize commits |
| opencode + OMO, from late March | Divide a feature among planning, implementation, and review roles | Break down tasks and coordinate models |
| Claude Code, around late May | A sustained series of related changes | Turn debugging and verification into repeatable project instructions and skills |
| kimi-code, from late August | Extended investigations across repositories and Git history | Set the goal and review the evidence and results |

OMO's role assignments and context compaction helped an agent carry a feature through to completion. Later, project instructions, reusable playbooks called skills, and longer context in Claude Code made debugging and screenshot verification repeatable. By the kimi-code stage, tracing Git history across three repositories had become a routine assignment. Sometimes the initial brief could be as broad as “figure out what happened here.”

My role shifted from author to coordinator to reviewer. Eventually, I could delegate some of the task breakdown I had previously done myself. But the models still needed clear completion criteria and tools to check the results.

## Give AI a Way to Check Its Work

At the start, I had AI read the assembly, build data structures for maps, Pokémon, moves, and the Pokédex, and add unit tests.

Passing tests can create a false sense of certainty. **Tests check the cases they cover. If their expectations differ from the original game, passing them doesn't establish fidelity.** I also had AI document the original's map and battle logic, then use those findings to check the implementation.

Many problems required running the game.

To check encounter logic, I had to walk into an encounter. To test a trainer, I had to reach them first. I tried having AI control an emulator of the original using screenshots alone. It spent so much time deciding basic forward and backward movements that little time was left for analyzing the game.

That pushed me to build dedicated testing interfaces. Battles are a relatively independent subsystem, so I added a command-line interface that let AI set up a battle, run it, and read the result directly. Checking the algorithms first, then adding visual verification, removed the need to replay story sequences just to reach a test.

Later, taking inspiration from Chrome's DevTools Protocol, I built a debug server for the game. It accepts commands over TCP and returns structured data:

| What needs checking | What the tools provide |
| --- | --- |
| What is happening now? | Queries for game state, NPCs, and story flags |
| Does this location behave correctly? | Teleportation to a specified map and position |
| What does this sequence of actions trigger? | Batched button inputs |
| At which frame does an animation diverge? | Synchronous frame stepping and screenshots |

The first debugging interface arrived in May. In July, I added better playtesting controls, frame stepping, and a headless mode that runs without a window. Those tools made sustained audits of battles and visuals much easier.

With controlled scenarios, frame comparisons became useful: bring the original and the remake to corresponding states, then compare how their screens change under the same inputs.

![Walking left in the bedroom: original, remake, and pixel differences](images/bedroom_walk_left-comparison.png)

*Original on the left, Rust remake in the middle, and pixel differences highlighted in red on the right. Differences in character position and scene edges need to be interpreted alongside the inputs and frame timing.*

![The Cut sequence in the original and the remake, compared over time](images/cut-raw-time.png)

*The Cut sequence: original above, remake at the time of comparison below. Dialogue, screen transitions, and the end of the sequence all need timing checks. A screenshot of the final state would miss these differences.*

Screenshots also became feedback for interface fixes. A model could inspect the before and after images, check for overlapping text or misplaced cursors and borders, and make another revision.

| Before | After |
| --- | --- |
| ![Chinese stats screen before the layout fix](images/ui-fix-stats-page1-zh-before.png) | ![Chinese stats screen after the layout fix](images/ui-fix-stats-page1-zh-after.png) |

*Layout fixes in the Chinese stats screen. The name and several labels overlap or are obscured on the left. The right image shows the corrected text and border positions.*

Alongside unit tests, I added visual snapshot tests for menus, dialogue boxes, and battle screens. If a later change alters a baseline image, the test flags it for review. This protects previously checked output from changing unnoticed. Whether the baseline itself matches the original still requires a separate comparison.

These checks gradually became part of GitHub CI, running automatically on pull requests. The models and tools kept changing, but the accumulated checks could stay.

More recently, I started having AI write gameplay scripts that simulate a complete playthrough. These connect story events that isolated tests can miss, checking whether special items and events correctly unlock later sequences.

Playthroughs have their own blind spots, though. **Without extra guidance, AI's scripts tend to follow the main story and rarely explore side content.** One successful run doesn't check the whole game. An unexpected result also needs investigation: the game might be wrong, or the script's inputs or expectations might be wrong.

I've started adding free exploration alongside the fixed playthroughs, and still need to broaden coverage of side content. Interpreting consecutive frames and matching some animations also remain unfinished work.

These experiences made me more willing to invest in verification tools early. Each new state query or control can reduce the manual work needed for many later fixes.

## The Stack and Architecture Shape the Cost of Future Changes

Choices made in the first week were still affecting development months later.

**One of Rust's biggest benefits in this project was reducing the cost of checking AI's work.** The compiler catches type, ownership, and interface errors before the game runs, providing feedback the model can act on. By day three, the project had 2,452 tests running. Together with the compiler, they supported the larger changes that followed.

April was the only month when the codebase shrank: roughly 48,000 lines added and 55,000 deleted, including an editor rewrite and a refactor of the battle system. Existing checks gave me the confidence to let AI make changes on that scale.

That made automated verification a more important consideration in language selection. Rust's compile-time constraints, testing support, and cross-platform ecosystem all helped this project.

The language could only do part of the job, though. Adding platforms, changing story content, and fixing visuals also needed clear architectural boundaries.

![Architecture of the game, reusable engine, and platform integrations](images/open-pokered-architecture-en.png)

*The project is divided by responsibility: game rules and content, the reusable engine, rendering, and platform integration each have a place.*

I made three main separations:

- **Game and engine.** I extracted reusable capabilities while keeping Pokémon-specific rules and content in the game, leaving room to build other games later.
- **Game logic and platform integration.** Windowing, hardware input, and display implementations are isolated so the platforms can share core logic. The terminal version can use those same game capabilities.
- **Content descriptions and runtime behavior.** Dedicated domain-specific languages (DSLs) describe story scripts and dialogue layouts, with JSON for other structured data. That gives content editing a consistent interface and supports hot reload during development.

Cross-platform results came early. The WebAssembly (WASM) build worked three days into the project. After I separated renderer features in May, Android and iOS followed by the end of the month. Platform integration still involved lifecycle and rendering problems, but sharing the core reduced duplicate implementations and the work needed to keep them in sync.

Separating content from code also changed how I filled out the story. I had worried that AI would struggle with a DSL I designed myself. In practice, a syntax checker and an accessible reference manual let the models write scenes according to its rules. Once the scripting engine was stable, I used Flash-series models to fill in story content in batches.

![Editing the main menu DSL with a rendered preview](images/editor-4.jpeg)

*The layout editor puts the description and result together: edit the main menu's DSL below and inspect the rendered preview above.*

Dialogue boxes benefited too. Layouts that had been scattered through the implementation now had a consistent representation and a clear place to edit and check them.

As the code and tests grew, module boundaries also affected regression testing. Running the full suite for every change means longer waits. Modules with clear responsibilities make it easier to identify what a change affects and choose the relevant checks.

Building that structure took time upfront. Its value became clearer as new platforms, story additions, and visual fixes each had an obvious place to go.

## Cheaper Models Made More Work Worth Doing

As the project grew, I paid more attention to how I assigned tasks.

MiMo, DeepSeek, and Kimi models produced a substantial share of the code and content, while Opus continued handling difficult judgments and reviews. I assigned work according to its demands: stronger reasoning for semantic checks; cheaper models for batches of well-defined work that could be checked automatically.

Story migration is one example. Once the scripting engine, syntax checks, and reference manual were ready, many scenes could follow the same rules. The tools made each task easier to specify, and lower model costs made batch execution more affordable.

Scene comparisons and additional checks followed a similar pattern. An individual task might be straightforward, but there were many of them. Work I would once have hesitated to repeat across the entire game could now run in parallel, backed by automated checks and reviews from stronger models.

I've kept a [record of monthly commits and model assignments](images/monthly-commits.png). Commit counts help trace the pace of work, but task types, commit sizes, models, and tools were all changing together. Those numbers alone can't isolate the productivity gain from a particular model.

As models from Chinese providers took on more of the work, I also gained more practical options. Reliable account access, subscription costs, and ease of continued use all matter in a project that runs for months.

For me, the most direct effect of falling costs was being able to run checks more often as part of everyday development.

## What Remains

Five months in, I can delegate much more of the implementation, debugging, and testing. More of my own effort goes into defining goals, designing boundaries, and deciding whether the evidence is sufficient.

The next tasks are concrete: improve animation timing, expand playtesting of side content and special events, and investigate behavior that a single screenshot or a successful run through the main story won't reveal.

Stronger models have changed this project. Every test, debugging command, and clear module boundary also helps the next round of work avoid repeating earlier detours. That is why I feel more confident about continuing.

The game is [open source](https://github.com/liuyanghejerry/open-pokered/) and [playable in your browser](https://liuyanghejerry.github.io/open-pokered/). The public repository contains the version after the engine was split out. That engine, [dotzuki](https://github.com/liuyanghejerry/dotzuki), also has an [online demo](https://liuyanghejerry.github.io/dotzuki/stable/).

If you try the game, I'd especially welcome some exploration beyond the main story. The places my playthrough scripts rarely visit are exactly where I want to look next.
