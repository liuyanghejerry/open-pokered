//! Cutscene system built on top of the Boa JS engine's async/await.
//!
//! Cutscenes are expressed directly as JS async functions — no separate
//! scripting language needed. A cutscene is simply an `export async function`
//! that uses the existing `game.*` API (`showText`, `movePlayer`, `delay`,
//! `fadeScreen`, `playSound`, etc.).
//!
//! # Architecture
//!
//! ```text
//! Trigger / Map Enter
//!     │
//!     ▼
//! CutsceneManager::start_cutscene("prof_lab_intro")
//!     │  sets active=true, current_script=Some("prof_lab_intro")
//!     │
//!     ▼
//! game loop: sees cutscene active → calls script_engine.call_function("prof_lab_intro")
//!     │
//!     ▼
//! JS: export async function prof_lab_intro() {
//!       await game.fadeScreen("out");
//!       await game.showText("Prof: Hello!");
//!       await game.movePlayer(x, y);
//!       await game.delay(30);
//!     }
//!     │  each await → ScriptCommand → Rust handles → promise resolved → next await
//!     │
//!     ▼
//! function returns → cutscene ends → player regains control
//! ```
//!
//! While a cutscene is active, the game loop suspends normal player directional
//! input. Dialog/choice interaction remains functional so that `await game.showText()`
//! and `await game.showChoice()` can proceed.

use alloc::collections::VecDeque;

/// Manages cutscene execution state.
///
/// A cutscene is a named JS async function that runs to completion,
/// suspending normal player input while active. Cutscenes can be
/// queued — when one finishes, the next one starts automatically.
#[derive(Debug, Clone)]
pub struct CutsceneManager {
    /// Whether a cutscene is currently executing.
    pub active: bool,
    /// The name of the currently running cutscene script (JS export name).
    pub current_script: Option<String>,
    /// When `true`, player directional/movement input is blocked.
    /// When `false`, the cutscene runs in parallel with player movement
    /// (useful for ambient NPC chatter or environmental effects).
    pub blocking: bool,
    /// Queue of pending cutscene script names. When the current cutscene
    /// finishes, the next one is started automatically.
    pub queue: VecDeque<String>,
    /// Internal: whether the current cutscene's script function has been
    /// called (via ScriptEngine::call_function). Reset to `false` when
    /// a new cutscene starts or a queued one is promoted.
    pub started: bool,
}

impl CutsceneManager {
    /// Creates a new cutscene manager with no active cutscene.
    pub fn new() -> Self {
        Self {
            active: false,
            current_script: None,
            blocking: true,
            queue: VecDeque::new(),
            started: false,
        }
    }

    /// Starts a cutscene with the given script name.
    ///
    /// If a cutscene is already active, the new script is appended to
    /// the queue instead of interrupting the current one.
    ///
    /// `blocking`: if `true`, player movement input is suspended.
    pub fn start_cutscene(&mut self, script_name: &str, blocking: bool) {
        if self.active {
            self.queue.push_back(script_name.to_string());
            return;
        }
        self.active = true;
        self.current_script = Some(script_name.to_string());
        self.blocking = blocking;
        self.started = false;
    }

    /// Returns `true` if a cutscene is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns `true` if the cutscene blocks player movement input.
    pub fn is_blocking(&self) -> bool {
        self.active && self.blocking
    }

    /// Returns the name of the currently running cutscene, if any.
    pub fn current_script_name(&self) -> Option<&str> {
        self.current_script.as_deref()
    }

    /// Ends the current cutscene and starts the next queued one if any.
    ///
    /// Returns `Some(script_name)` if a queued cutscene was started.
    pub fn end_cutscene(&mut self) -> Option<String> {
        self.active = false;
        self.current_script = None;
        self.started = false;

        if let Some(next) = self.queue.pop_front() {
            self.active = true;
            self.current_script = Some(next.clone());
            self.started = false;
            return Some(next);
        }
        None
    }

    /// Adds a script to the end of the cutscene queue.
    pub fn queue_script(&mut self, script_name: &str) {
        self.queue.push_back(script_name.to_string());
    }

    /// Returns the number of scripts waiting in the queue.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Returns `true` if there are scripts waiting in the queue.
    pub fn has_queued(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Clears all queued scripts.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Forcefully stops the current cutscene and clears the queue.
    pub fn force_stop(&mut self) {
        self.active = false;
        self.current_script = None;
        self.queue.clear();
        self.started = false;
    }

    pub fn mark_started(&mut self) {
        self.started = true;
    }

    pub fn needs_start(&self) -> bool {
        self.active && !self.started
    }
}

impl Default for CutsceneManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cutscene_manager() {
        let cm = CutsceneManager::new();
        assert!(!cm.is_active());
        assert!(cm.current_script.is_none());
        assert!(cm.queue.is_empty());
    }

    #[test]
    fn test_start_and_end_cutscene() {
        let mut cm = CutsceneManager::new();
        cm.start_cutscene("prof_lab_intro", true);
        assert!(cm.is_active());
        assert!(cm.is_blocking());
        assert_eq!(cm.current_script_name(), Some("prof_lab_intro"));

        cm.end_cutscene();
        assert!(!cm.is_active());
        assert!(cm.current_script.is_none());
    }

    #[test]
    fn test_non_blocking_cutscene() {
        let mut cm = CutsceneManager::new();
        cm.start_cutscene("ambient_chatter", false);
        assert!(cm.is_active());
        assert!(!cm.is_blocking());
    }

    #[test]
    fn test_queue_when_active() {
        let mut cm = CutsceneManager::new();
        cm.start_cutscene("first", true);
        cm.start_cutscene("second", true);
        cm.start_cutscene("third", true);

        // Only "first" should be running; others are queued.
        assert_eq!(cm.current_script_name(), Some("first"));
        assert_eq!(cm.queue_len(), 2);

        // End "first" → "second" starts.
        let next = cm.end_cutscene();
        assert_eq!(next, Some("second".to_string()));
        assert_eq!(cm.current_script_name(), Some("second"));
        assert_eq!(cm.queue_len(), 1);

        // End "second" → "third" starts.
        let next = cm.end_cutscene();
        assert_eq!(next, Some("third".to_string()));
        assert_eq!(cm.current_script_name(), Some("third"));
        assert_eq!(cm.queue_len(), 0);

        // End "third" → nothing queued, cutscene ends.
        let next = cm.end_cutscene();
        assert_eq!(next, None);
        assert!(!cm.is_active());
    }

    #[test]
    fn test_queue_script() {
        let mut cm = CutsceneManager::new();
        cm.queue_script("intro");
        cm.queue_script("prof_arrives");
        assert_eq!(cm.queue_len(), 2);
        assert!(!cm.is_active()); // queueing alone doesn't start

        // Start the first manually.
        cm.start_cutscene("manual_start", true);
        assert_eq!(cm.current_script_name(), Some("manual_start"));

        // End it → first queued script starts.
        let next = cm.end_cutscene();
        assert_eq!(next, Some("intro".to_string()));

        // End again → second queued script starts.
        let next = cm.end_cutscene();
        assert_eq!(next, Some("prof_arrives".to_string()));
    }

    #[test]
    fn test_force_stop() {
        let mut cm = CutsceneManager::new();
        cm.start_cutscene("running", true);
        cm.queue_script("next1");
        cm.queue_script("next2");

        cm.force_stop();
        assert!(!cm.is_active());
        assert!(cm.current_script.is_none());
        assert!(cm.queue.is_empty());
    }

    #[test]
    fn test_clear_queue() {
        let mut cm = CutsceneManager::new();
        cm.queue_script("a");
        cm.queue_script("b");
        cm.clear_queue();
        assert_eq!(cm.queue_len(), 0);
        assert!(cm.queue.is_empty());
    }

    #[test]
    fn test_default() {
        let cm: CutsceneManager = Default::default();
        assert!(!cm.is_active());
        assert!(cm.queue.is_empty());
    }
}
