# Script Engines: native `.scene` interpreter + Boa JS fallback

Status: the **native AST interpreter is canonical** (all builds). The Boa JS
engine is kept deliberately as a compile-time fallback behind the
`script-boa` feature.

## History

The original scripting design ran on a JavaScript engine (Boa, via
`dotzuki-engine-script`). That implementation was functionally complete, but
its runtime overhead proved too large for the shipping game, so the events
were migrated to a native interpreter: `dotzuki_engine_dsl` compiles each
map's `crates/pokered-data/maps/*/script.scene` to an AST at build time
(`pokered-data/build.rs`), and `pokered-core`'s `NativeScriptEngine`
(`crates/pokered-core/src/overworld/native_script.rs`) interprets it
directly.

The migration predates this repo's extraction from the monorepo; the only
in-repo fossil is the `script-boa` feature comment in
`crates/pokered-core/Cargo.toml` ("the AST interpreter is the canonical
scene semantics").

## Why the JS engine is kept

The fallback is a deliberate engineering decision, not leftovers:

- **Lower barrier for custom scripting.** JavaScript is about the most
  widely known language there is; asking modders to learn a bespoke DSL
  first raises the entry cost for no benefit to them.
- **Easier AI collaboration.** Models write idiomatic JS fluently, while a
  proprietary DSL needs far more context and hand-holding.
- **Deployment freedom.** Where the game ultimately runs — native
  interpreter for production, JS for tooling, prototyping, or other hosts —
  should stay a free decision, not one the architecture locks in.

Both engines are fed by the same source of truth: the build compiles each
`.scene` file into an embedded AST (`SCENE_ASTS`, consumed by the native
interpreter) and into a JS module (`SCENE_SCRIPTS`, consumed by the Boa
path), so the two can never drift. `dotzuki-engine-script` remains an
unconditional dependency because the shared `ScriptCommand`/`CommandResult`
protocol types live there.

## Caveats

- No crate in the workspace enables `script-boa` today; the Boa variant is
  compiled out of default builds.
- The single `@run` block (Vermilion Gym trash-can puzzle) is hand-ported
  to a native handler (see the `native_script.rs` file-header comment).
- A minimal example of the JS cutscene API (the old `assets/scripts`
  demo) is kept at `docs/examples/oaks_lab_intro.js`.
