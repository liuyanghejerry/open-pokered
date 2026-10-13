use std::collections::{HashMap, HashSet};

use crate::compiler::RouteEntry;

/// Result of conflict detection.
pub struct ConflictResult {
    pub has_conflicts: bool,
    pub warnings: Vec<String>,
}

/// Check for conflicting storylines (same map+npc, no after chain).
///
/// Two storylines on the same `(map, npc)` conflict if they are not connected
/// by an `after` relationship chain. Routes without an NPC binding (on-enter
/// only) are excluded from conflict detection.
pub fn detect_conflicts(routes: &[RouteEntry]) -> ConflictResult {
    let mut result = ConflictResult {
        has_conflicts: false,
        warnings: Vec::new(),
    };

    // ── Build global after-chain graph (all routes, all maps) ───────────
    // This lets us trace cross-map chains like prof_ask → mart_pickup → prof_delivery
    let all_names: HashSet<&str> = routes.iter().map(|r| r.storyline.as_str()).collect();
    let mut global_adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for route in routes {
        if let Some(after) = &route.after {
            if all_names.contains(after.as_str()) {
                global_adj.entry(&route.storyline).or_default().push(after);
                global_adj
                    .entry(after.as_str())
                    .or_default()
                    .push(&route.storyline);
            }
        }
    }

    // ── Group routes by (map, npc) ──────────────────────────────────────
    // Only NPC-triggered routes participate; onEnter-only routes are skipped
    // because they don't compete for the same interaction slot.
    let mut groups: HashMap<(String, Option<String>), Vec<&RouteEntry>> = HashMap::new();
    for route in routes {
        if route.npc.is_some() {
            let key = (route.map.clone(), route.npc.clone());
            groups.entry(key).or_default().push(route);
        }
    }

    for ((map, npc), group) in &groups {
        if group.len() <= 1 {
            continue;
        }

        // ── DFS on GLOBAL after graph (not per-group) ──────────────────
        // Use the global adjacency so cross-map chains are recognized.
        let group_names: Vec<&str> = group.iter().map(|r| r.storyline.as_str()).collect();
        let mut visited: HashSet<&str> = HashSet::new();
        let mut components: Vec<Vec<&str>> = Vec::new();
        for &name in &group_names {
            if !visited.contains(name) {
                let mut component = Vec::new();
                let mut stack = vec![name];
                while let Some(n) = stack.pop() {
                    if visited.insert(n) {
                        component.push(n);
                        if let Some(neighbors) = global_adj.get(n) {
                            for &nb in neighbors {
                                if !visited.contains(nb) {
                                    stack.push(nb);
                                }
                            }
                        }
                    }
                }
                // Only keep storylines from THIS group in the component
                let group_component: Vec<&str> = component
                    .into_iter()
                    .filter(|n| group_names.contains(n))
                    .collect();
                if !group_component.is_empty() {
                    components.push(group_component);
                }
            }
        }

        // ── Multiple components = conflict ───────────────────────────────
        if components.len() > 1 {
            result.has_conflicts = true;
            let npc_name = npc.as_deref().unwrap_or("onEnter");
            for i in 1..components.len() {
                result.warnings.push(format!(
                    "CONFLICT: @storyline(\"{}\") and @storyline(\"{}\") on ({}, {}): no \"after\" relationship",
                    components[i - 1][0], components[i][0], map, npc_name
                ));
            }
        }
    }

    result
}

// ══════════════════════════════════════════════════════════════════════════════
// Unit tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn route(map: &str, npc: &str, storyline: &str, after: Option<&str>) -> RouteEntry {
        RouteEntry {
            map: map.to_string(),
            npc: Some(npc.to_string()),
            on_enter: false,
            storyline: storyline.to_string(),
            after: after.map(String::from),
        }
    }

    fn route_on_enter(map: &str, storyline: &str) -> RouteEntry {
        RouteEntry {
            map: map.to_string(),
            npc: None,
            on_enter: true,
            storyline: storyline.to_string(),
            after: None,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_no_conflict_single_route
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_no_conflict_single_route() {
        let routes = vec![route("ProfLab", "Prof", "prof_ask", None)];
        let result = detect_conflicts(&routes);
        assert!(!result.has_conflicts, "Single route should not conflict");
        assert!(result.warnings.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_no_conflict_after_chain
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_no_conflict_after_chain() {
        let routes = vec![
            route("ProfLab", "Prof", "prof_ask", None),
            route("ProfLab", "Prof", "rival_challenge", Some("prof_ask")),
        ];
        let result = detect_conflicts(&routes);
        assert!(
            !result.has_conflicts,
            "Routes connected by after chain should not conflict"
        );
        assert!(result.warnings.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_conflict_independent_routes
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_conflict_independent_routes() {
        let routes = vec![
            route("ProfLab", "Prof", "prof_ask", None),
            route("ProfLab", "Prof", "rival_challenge", None),
        ];
        let result = detect_conflicts(&routes);
        assert!(result.has_conflicts, "Independent routes should conflict");
        assert_eq!(result.warnings.len(), 1);
        let w = &result.warnings[0];
        assert!(w.contains("CONFLICT"), "Warning should contain CONFLICT");
        assert!(
            w.contains("prof_ask"),
            "Warning should mention prof_ask, got: {}",
            w
        );
        assert!(
            w.contains("rival_challenge"),
            "Warning should mention rival_challenge"
        );
        assert!(w.contains("ProfLab"), "Warning should mention map ProfLab");
        assert!(w.contains("Prof"), "Warning should mention NPC Prof");
        assert!(
            w.contains("no \"after\" relationship"),
            "Warning should mention missing after relationship"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_no_conflict_different_npc
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_no_conflict_different_npc() {
        let routes = vec![
            route("ProfLab", "Prof", "prof_ask", None),
            route("ProfLab", "Aide", "aide_help", None),
        ];
        let result = detect_conflicts(&routes);
        assert!(
            !result.has_conflicts,
            "Different NPCs on the same map should not conflict"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_no_conflict_different_map
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_no_conflict_different_map() {
        let routes = vec![
            route("ProfLab", "Prof", "prof_ask", None),
            route("CityCenter", "Prof", "prof_heal", None),
        ];
        let result = detect_conflicts(&routes);
        assert!(
            !result.has_conflicts,
            "Same NPC on different maps should not conflict"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_no_conflict_on_enter_and_npc
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_no_conflict_on_enter_and_npc() {
        let routes = vec![
            route_on_enter("ProfLab", "lab_entry"),
            route("ProfLab", "Prof", "prof_ask", None),
        ];
        let result = detect_conflicts(&routes);
        assert!(
            !result.has_conflicts,
            "onEnter route and NPC route on same map should not conflict"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // test_conflict_three_routes_two_conflict
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_conflict_three_routes_two_conflict() {
        let routes = vec![
            route("ProfLab", "Prof", "prof_ask", None),
            route("ProfLab", "Prof", "rival_challenge", Some("prof_ask")),
            route("ProfLab", "Prof", "post_game", None),
        ];
        let result = detect_conflicts(&routes);
        assert!(
            result.has_conflicts,
            "3 routes with 2 in chain and 1 orphan should conflict"
        );
        // prof_ask + rival_challenge form one chain; post_game is orphan
        let w = &result.warnings[0];
        assert!(w.contains("CONFLICT"));
        assert!(
            w.contains("post_game"),
            "Warning should mention orphan storyline post_game, got: {}",
            w
        );
    }
}
