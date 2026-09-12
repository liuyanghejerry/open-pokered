//! Generator for the M4 static artifacts:
//! - `crates/pokered-data/story/graph.json` — the committed event graph
//!   (kept in sync by the `graph_json_matches_committed_file` test).
//! - `target/agent/world_semantics.json` — the per-map semantics +
//!   coverage artifact (build output, not committed).
//!
//! Run from the workspace root: `cargo run -p pokered-agent --bin gen_event_graph`.

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let graph_path = std::path::Path::new(manifest)
        .join("../pokered-data/story/graph.json")
        .canonicalize()
        .expect("story/graph.json path resolves");
    let out_dir = std::path::Path::new(manifest)
        .join("../../target/agent")
        .canonicalize()
        .unwrap_or_else(|_| std::path::Path::new(manifest).join("../../target/agent"));

    let graph_json = pokered_agent::generate_event_graph_json();
    std::fs::write(&graph_path, &graph_json).expect("write story/graph.json");

    std::fs::create_dir_all(&out_dir).expect("create target/agent");
    let world = pokered_agent::generate_world_semantics();
    let semantics_path = out_dir.join("world_semantics.json");
    let mut json = serde_json::to_string_pretty(&world).expect("world semantics serializes");
    json.push('\n');
    std::fs::write(&semantics_path, json).expect("write world_semantics.json");

    let edge_count = graph_json.matches("\"from\"").count();
    println!(
        "wrote {} ({} edges)\nwrote {} ({} maps, {} storylines; commands {} recognized / {} unknown; calls {} recognized / {} unknown)",
        graph_path.display(),
        edge_count,
        semantics_path.display(),
        world.coverage.maps_analyzed,
        world.coverage.storylines_analyzed,
        world.coverage.commands_recognized,
        world.coverage.commands_unknown,
        world.coverage.calls_recognized,
        world.coverage.calls_unknown,
    );
    if !world.coverage.unknown_constructs.is_empty() {
        println!("unknown constructs:");
        for u in &world.coverage.unknown_constructs {
            println!("  {} x{} (first: {})", u.name, u.count, u.example_map);
        }
    }
}
