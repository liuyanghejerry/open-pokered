//! # trigger_manager
//!
//! Generic trigger system for JRPG maps.  Triggers bind a script function to a
//! tile area and fire when the player steps on, enters, or interacts with that
//! area.  Triggers can be sourced from Tiled custom properties, exported JS
//! function-name conventions, or hand-authored config files.
//!
//! The [`TriggerManager`] tracks active triggers and provides position-based
//! queries used by the game loop.

use crate::metatile::TriggerType;

// ---------------------------------------------------------------------------
// Trigger
// ---------------------------------------------------------------------------

/// A trigger bound to a specific tile or rectangular area on a map.
///
/// When the player satisfies the [`trigger_type`](Trigger::trigger_type)
/// condition (stepping on, entering, or interacting with the tile), the
/// associated [`script_name`](Trigger::script_name) is returned so the game
/// loop can call the corresponding JS function on the script engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// Unique identifier for this trigger (e.g. `"start_town_prof"`).
    pub id: String,
    /// The map this trigger lives on (e.g. `"StartTown"`).
    pub map_id: String,
    /// When the trigger fires — every frame ([`OnStep`](TriggerType::OnStep)),
    /// once on entry ([`OnEnter`](TriggerType::OnEnter)), or on A-press
    /// ([`OnInteract`](TriggerType::OnInteract)).
    pub trigger_type: TriggerType,
    /// Top-left X coordinate of the trigger area (in tile units).
    pub x: u32,
    /// Top-left Y coordinate of the trigger area (in tile units).
    pub y: u32,
    /// Width of the trigger area in tiles.  `1` for a single-tile trigger;
    /// larger values for rectangular area triggers.
    pub width: u32,
    /// Height of the trigger area in tiles.
    pub height: u32,
    /// Name of the exported JS async function to call when this trigger fires.
    pub script_name: String,
    /// If `true`, the trigger fires only once and then is permanently disabled.
    pub one_shot: bool,
    /// Whether this trigger has already been activated (relevant for one-shot
    /// triggers).  Call [`TriggerManager::reset_fired_for_map`] to re-enable
    /// triggers after re-entering a map.
    pub fired: bool,
}

impl Trigger {
    /// Creates a single-tile trigger.
    pub fn single_tile(
        id: impl Into<String>,
        map_id: impl Into<String>,
        trigger_type: TriggerType,
        x: u32,
        y: u32,
        script_name: impl Into<String>,
        one_shot: bool,
    ) -> Self {
        Self {
            id: id.into(),
            map_id: map_id.into(),
            trigger_type,
            x,
            y,
            width: 1,
            height: 1,
            script_name: script_name.into(),
            one_shot,
            fired: false,
        }
    }

    /// Returns `true` when tile `(tile_x, tile_y)` falls inside this trigger's
    /// axis-aligned bounding box.
    #[inline]
    pub fn contains(&self, tile_x: u32, tile_y: u32) -> bool {
        tile_x >= self.x
            && tile_x < self.x.saturating_add(self.width)
            && tile_y >= self.y
            && tile_y < self.y.saturating_add(self.height)
    }
}

// ---------------------------------------------------------------------------
// TriggerManager
// ---------------------------------------------------------------------------

/// Manages a collection of [`Trigger`]s and determines which ones should fire
/// given the player's current (and previous) position.
///
/// # Usage
///
/// ```ignore
/// let mut mgr = TriggerManager::new();
/// mgr.add_trigger(Trigger::single_tile(
///     "heal_npc", "StartTown", TriggerType::OnStep,
///     3, 5, "healParty", true,
/// ));
///
/// // Every frame
/// let triggered: Vec<String> = mgr.check_triggers("StartTown", 3, 5);
/// for name in &triggered {
///     script_engine.call_function(name);
/// }
///
/// // On A button press
/// if let Some(name) = mgr.check_interact_mut("StartTown", facing_x, facing_y) {
///     script_engine.call_function(&name);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct TriggerManager {
    triggers: Vec<Trigger>,
    /// Tracked so [`OnEnter`](TriggerType::OnEnter) triggers can detect when
    /// the player *enters* an area rather than simply standing in it.
    prev_player_map: Option<String>,
    prev_player_x: u32,
    prev_player_y: u32,
}

impl TriggerManager {
    /// Creates an empty trigger manager.
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
            prev_player_map: None,
            prev_player_x: 0,
            prev_player_y: 0,
        }
    }

    // ------------------------------------------------------------------
    // Registration
    // ------------------------------------------------------------------

    /// Registers a single trigger.
    pub fn add_trigger(&mut self, trigger: Trigger) {
        self.triggers.push(trigger);
    }

    /// Registers multiple triggers in bulk.
    pub fn add_triggers(&mut self, triggers: impl IntoIterator<Item = Trigger>) {
        self.triggers.extend(triggers);
    }

    /// Removes all triggers associated with `map_id`.
    pub fn remove_triggers_for_map(&mut self, map_id: &str) {
        self.triggers.retain(|t| t.map_id != map_id);
    }

    /// Resets the [`fired`](Trigger::fired) flag for every trigger on the
    /// given map, allowing one-shot triggers to fire again (e.g. after the
    /// player re-enters the map).
    pub fn reset_fired_for_map(&mut self, map_id: &str) {
        for trigger in &mut self.triggers {
            if trigger.map_id == map_id {
                trigger.fired = false;
            }
        }
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Returns an iterator over all registered triggers (regardless of map).
    pub fn all_triggers(&self) -> impl Iterator<Item = &Trigger> {
        self.triggers.iter()
    }

    /// Number of registered triggers.
    pub fn len(&self) -> usize {
        self.triggers.len()
    }

    /// Returns `true` when there are no registered triggers.
    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    // ------------------------------------------------------------------
    // Core trigger checking
    // ------------------------------------------------------------------

    /// Checks all triggers for the given map against the player's current
    /// position.  Handles [`OnStep`](TriggerType::OnStep) and
    /// [`OnEnter`](TriggerType::OnEnter) triggers.
    ///
    /// Returns the [`script_name`](Trigger::script_name) of every trigger
    /// that should fire this frame.  Callers should iterate the returned
    /// list and invoke each function on the script engine.
    ///
    /// One-shot triggers are automatically marked as `fired` so they won't
    /// activate again until [`reset_fired_for_map`](Self::reset_fired_for_map)
    /// is called.
    ///
    /// # OnEnter vs OnStep
    ///
    /// * **OnEnter** — fires when the player was *not* in the area last frame
    ///   but *is* this frame (entry detection).
    /// * **OnStep** — fires every frame the player stands inside the area.
    pub fn check_triggers(&mut self, map_id: &str, player_x: u32, player_y: u32) -> Vec<String> {
        let same_map = self.prev_player_map.as_deref() == Some(map_id);

        // Helper: was the player inside this trigger's area last frame?
        let was_inside = |t: &Trigger| -> bool {
            same_map && t.contains(self.prev_player_x, self.prev_player_y)
        };

        // Helper: is the player inside this trigger's area now?
        let is_inside = |t: &Trigger| -> bool { t.contains(player_x, player_y) };

        let mut triggered = Vec::new();

        for trigger in &mut self.triggers {
            if trigger.map_id != map_id {
                continue;
            }

            // One-shot triggers that already fired are skipped.
            if trigger.one_shot && trigger.fired {
                continue;
            }

            let should_fire = match trigger.trigger_type {
                TriggerType::OnStep => is_inside(trigger),
                TriggerType::OnEnter => is_inside(trigger) && !was_inside(trigger),
                // OnInteract is checked via check_interact / check_interact_mut
                TriggerType::OnInteract => false,
            };

            if should_fire {
                trigger.fired = true;
                triggered.push(trigger.script_name.clone());
            }
        }

        // Record position for next frame's OnEnter detection.
        self.prev_player_map = Some(map_id.to_string());
        self.prev_player_x = player_x;
        self.prev_player_y = player_y;

        triggered
    }

    // ------------------------------------------------------------------
    // OnInteract (A-button) queries
    // ------------------------------------------------------------------

    /// Checks for [`OnInteract`](TriggerType::OnInteract) triggers at the tile
    /// the player is facing.  Returns the script name if one is found.
    ///
    /// This is a read-only query — it does **not** mark one-shot triggers
    /// as fired.  Use [`check_interact_mut`](Self::check_interact_mut) when
    /// you also want to disable the trigger after activation.
    pub fn check_interact(&self, map_id: &str, facing_x: u32, facing_y: u32) -> Option<&str> {
        self.triggers
            .iter()
            .find(|t| {
                t.map_id == map_id
                    && t.trigger_type == TriggerType::OnInteract
                    && (!t.one_shot || !t.fired)
                    && t.contains(facing_x, facing_y)
            })
            .map(|t| t.script_name.as_str())
    }

    /// Checks for [`OnInteract`](TriggerType::OnInteract) triggers at the tile
    /// the player is facing.  Returns the script name and marks the trigger as
    /// `fired` (so one-shot triggers won't activate again).
    pub fn check_interact_mut(
        &mut self,
        map_id: &str,
        facing_x: u32,
        facing_y: u32,
    ) -> Option<String> {
        for trigger in &mut self.triggers {
            if trigger.map_id != map_id {
                continue;
            }
            if trigger.trigger_type != TriggerType::OnInteract {
                continue;
            }
            if trigger.one_shot && trigger.fired {
                continue;
            }
            if trigger.contains(facing_x, facing_y) {
                trigger.fired = true;
                return Some(trigger.script_name.clone());
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trigger(
        id: &str,
        map_id: &str,
        tt: TriggerType,
        x: u32,
        y: u32,
        script: &str,
        one_shot: bool,
    ) -> Trigger {
        Trigger::single_tile(id, map_id, tt, x, y, script, one_shot)
    }

    // -- OnStep ----------------------------------------------------------

    #[test]
    fn on_step_fires_when_standing() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnStep,
            2,
            3,
            "stepFn",
            false,
        ));
        // Need one call to establish previous position
        mgr.check_triggers("map1", 0, 0);
        let names = mgr.check_triggers("map1", 2, 3);
        assert_eq!(names, vec!["stepFn"]);
    }

    #[test]
    fn on_step_fires_every_frame() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnStep,
            2,
            3,
            "stepFn",
            false,
        ));
        mgr.check_triggers("map1", 0, 0); // establish prev
        let first = mgr.check_triggers("map1", 2, 3);
        let second = mgr.check_triggers("map1", 2, 3);
        assert_eq!(first, vec!["stepFn"]);
        assert_eq!(second, vec!["stepFn"]);
    }

    #[test]
    fn on_step_does_not_fire_when_not_standing() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnStep,
            2,
            3,
            "stepFn",
            false,
        ));
        mgr.check_triggers("map1", 0, 0);
        let names = mgr.check_triggers("map1", 5, 5);
        assert!(names.is_empty());
    }

    // -- OnEnter ---------------------------------------------------------

    #[test]
    fn on_enter_fires_once_on_entry() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnEnter,
            2,
            3,
            "enterFn",
            false,
        ));
        // Player is outside area
        mgr.check_triggers("map1", 0, 0);
        // Player walks in
        let names = mgr.check_triggers("map1", 2, 3);
        assert_eq!(names, vec!["enterFn"]);
    }

    #[test]
    fn on_enter_does_not_refire() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnEnter,
            2,
            3,
            "enterFn",
            false,
        ));
        mgr.check_triggers("map1", 0, 0); // outside
        mgr.check_triggers("map1", 2, 3); // enter — fires
        let second = mgr.check_triggers("map1", 2, 3); // still inside — no fire
        assert!(second.is_empty());
    }

    #[test]
    fn on_enter_fires_on_different_map_entry() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map2",
            TriggerType::OnEnter,
            0,
            0,
            "enterFn",
            false,
        ));
        mgr.check_triggers("map1", 5, 5); // different map
        let names = mgr.check_triggers("map2", 0, 0); // first frame on map2
        assert_eq!(names, vec!["enterFn"]);
    }

    // -- One-shot --------------------------------------------------------

    #[test]
    fn one_shot_trigger_fires_once() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnStep,
            2,
            3,
            "onceFn",
            true,
        ));
        mgr.check_triggers("map1", 0, 0);
        let first = mgr.check_triggers("map1", 2, 3);
        let second = mgr.check_triggers("map1", 2, 3);
        assert_eq!(first, vec!["onceFn"]);
        assert!(second.is_empty());
    }

    #[test]
    fn one_shot_reset_allows_refire() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnStep,
            2,
            3,
            "onceFn",
            true,
        ));
        mgr.check_triggers("map1", 0, 0);
        mgr.check_triggers("map1", 2, 3); // fires
        mgr.reset_fired_for_map("map1");
        mgr.check_triggers("map1", 0, 0);
        let again = mgr.check_triggers("map1", 2, 3);
        assert_eq!(again, vec!["onceFn"]);
    }

    // -- OnInteract ------------------------------------------------------

    #[test]
    fn on_interact_fires_on_facing_tile() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnInteract,
            5,
            5,
            "talkFn",
            false,
        ));
        let name = mgr.check_interact("map1", 5, 5);
        assert_eq!(name, Some("talkFn"));
    }

    #[test]
    fn on_interact_ignores_wrong_position() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnInteract,
            5,
            5,
            "talkFn",
            false,
        ));
        let name = mgr.check_interact("map1", 0, 0);
        assert!(name.is_none());
    }

    #[test]
    fn on_interact_mut_marks_fired() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnInteract,
            5,
            5,
            "talkFn",
            true,
        ));
        let first = mgr.check_interact_mut("map1", 5, 5);
        assert_eq!(first, Some("talkFn".into()));
        let second = mgr.check_interact("map1", 5, 5);
        assert!(second.is_none());
    }

    // -- Area triggers ---------------------------------------------------

    #[test]
    fn area_trigger_2x2() {
        let mut mgr = TriggerManager::new();
        let t = Trigger {
            id: "area".into(),
            map_id: "map1".into(),
            trigger_type: TriggerType::OnStep,
            x: 4,
            y: 4,
            width: 2,
            height: 2,
            script_name: "areaFn".into(),
            one_shot: false,
            fired: false,
        };
        mgr.add_trigger(t);
        mgr.check_triggers("map1", 0, 0);
        // (4,4), (5,4), (4,5), (5,5) should all match
        for (x, y) in [(4, 4), (5, 4), (4, 5), (5, 5)] {
            assert!(mgr.check_triggers("map1", x, y).contains(&"areaFn".into()));
        }
        // Outside
        mgr.check_triggers("map1", 0, 0);
        assert!(!mgr.check_triggers("map1", 3, 4).contains(&"areaFn".into()));
        assert!(!mgr.check_triggers("map1", 6, 4).contains(&"areaFn".into()));
    }

    // -- Removal ---------------------------------------------------------

    #[test]
    fn remove_triggers_for_map() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "a",
            "map1",
            TriggerType::OnStep,
            0,
            0,
            "fn1",
            false,
        ));
        mgr.add_trigger(make_trigger(
            "b",
            "map2",
            TriggerType::OnStep,
            0,
            0,
            "fn2",
            false,
        ));
        mgr.remove_triggers_for_map("map1");
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.triggers[0].script_name, "fn2");
    }

    // -- Multiple trigger types on same tile ------------------------------

    #[test]
    fn on_step_and_on_enter_both_fire_on_entry() {
        let mut mgr = TriggerManager::new();
        mgr.add_trigger(make_trigger(
            "s",
            "map1",
            TriggerType::OnStep,
            2,
            3,
            "stepFn",
            false,
        ));
        mgr.add_trigger(make_trigger(
            "e",
            "map1",
            TriggerType::OnEnter,
            2,
            3,
            "enterFn",
            false,
        ));
        mgr.check_triggers("map1", 0, 0); // outside
        let names = mgr.check_triggers("map1", 2, 3);
        // Both should fire on entry frame
        assert!(names.contains(&"stepFn".into()));
        assert!(names.contains(&"enterFn".into()));
    }
}
