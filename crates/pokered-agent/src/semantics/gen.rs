//! Deterministic artifact generation: the whole-world semantics payload
//! (per-map semantics + coverage) and the `story/graph.json` document.
//! Both are pure functions of the embedded scene ASTs — same input,
//! same bytes.

use serde::{Deserialize, Serialize};

use super::extract::{extract_map_with_coverage, CoverageAccum};
use super::graph::EventGraph;
use super::types::{CoverageReport, MapSemantics};

/// The full world-semantics artifact (`target/agent/world_semantics.json`
/// shape and the `get_script_semantics` wire payload when unscoped).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSemantics {
    pub maps: Vec<MapSemantics>,
    pub coverage: CoverageReport,
}

/// Extract semantics for every embedded scene AST. `scene_asts()` is
/// sorted by map name; shared modules (`shared/*`) are not map scripts
/// and are skipped.
pub fn generate_world_semantics() -> WorldSemantics {
    let mut coverage = CoverageAccum::default();
    let mut maps = Vec::new();
    for (name, _) in pokered_data::embedded_scenes::scene_asts() {
        if name.starts_with("shared/") {
            continue;
        }
        let Some(scene) = pokered_data::embedded_scenes::get_scene_ast(name) else {
            coverage.maps_without_scene += 1;
            continue;
        };
        maps.push(extract_map_with_coverage(&scene, &mut coverage));
    }
    let coverage = coverage.into_report();
    WorldSemantics { maps, coverage }
}

/// The deterministic `story/graph.json` document: the event graph's
/// edges, sorted, pretty-printed with a trailing newline.
pub fn generate_event_graph_json() -> String {
    let world = generate_world_semantics();
    let graph = EventGraph::build(&world.maps);
    #[derive(Serialize)]
    struct GraphDoc<'a> {
        edges: &'a [super::graph::EventEdge],
    }
    let mut json = serde_json::to_string_pretty(&GraphDoc {
        edges: graph.edges(),
    })
    .expect("event graph serializes");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(generate_event_graph_json(), generate_event_graph_json());
        let a = generate_world_semantics();
        let b = generate_world_semantics();
        assert_eq!(a.maps, b.maps);
        assert_eq!(a.coverage, b.coverage);
    }

    #[test]
    fn graph_json_matches_committed_file() {
        // story/graph.json is GENERATED (see pokered-agent's
        // gen_event_graph bin); drift fails this test — regenerate it.
        let committed = include_str!("../../../pokered-data/story/graph.json");
        assert_eq!(generate_event_graph_json(), committed);
    }

    #[test]
    fn coverage_has_no_unbounded_unknowns() {
        let world = generate_world_semantics();
        assert!(world.coverage.maps_analyzed > 200);
        assert!(world.coverage.storylines_analyzed > 1000);
        // Every unknown construct must be one of the known-difficult
        // escape hatches — a new name here means the extractor regressed
        // (or a scene introduced a genuinely new construct to analyze).
        //   run_js: the native-ported VermilionGym trash-can puzzle.
        //   hasMoney(dynamic_arg): Daycare's computed-cost check.
        const ALLOWED: &[&str] = &["run_js", "hasMoney(dynamic_arg)"];
        for unknown in &world.coverage.unknown_constructs {
            assert!(
                ALLOWED.contains(&unknown.name.as_str()),
                "unexpected unknown construct {:?} (first seen on {})",
                unknown.name,
                unknown.example_map
            );
        }
    }

    #[test]
    fn objectives_flags_all_resolve() {
        #[derive(Deserialize)]
        struct ObjectiveFile {
            objectives: Vec<Objective>,
        }
        #[derive(Deserialize)]
        struct Objective {
            id: String,
            satisfied_when: SatisfiedWhen,
        }
        #[derive(Deserialize)]
        struct SatisfiedWhen {
            flag: String,
        }
        let doc: ObjectiveFile = serde_json::from_str(include_str!(
            "../../../pokered-data/story/objectives.json"
        ))
        .expect("objectives.json parses");
        assert!(!doc.objectives.is_empty());
        for objective in &doc.objectives {
            assert!(
                pokered_data::event_flags::EventFlag::from_name(&objective.satisfied_when.flag)
                    .is_some(),
                "objective {} references unknown flag {}",
                objective.id,
                objective.satisfied_when.flag
            );
        }
    }
}
