# Your First Game

The tutorial project for `tutorials/your-first-game.md`: a zero-Rust game
built entirely from a manifest, data tables, a `rules.ron` type chart and
Game DSL scenes. One town, one battle clearing, one battle, one save.

Run it:

```bash
cd workspace
cargo build --release --bin dotzuki
./target/release/dotzuki check examples/your-first-game
./target/release/dotzuki run examples/your-first-game            # windowed
./target/release/dotzuki run examples/your-first-game --headless --frames 180   # CI smoke
```
