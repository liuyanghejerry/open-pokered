//! Ordered visual handoff for a turn already resolved by the deterministic engine.
//! Metadata is attached before localization/pagination, never parsed from a page.
use super::{
    pokered_rules::{runtime, PokeVolatile, PokeredRules},
    BattleScreen, BattleSide, FaintCue,
};
use crate::alloc_prelude::*;
use dotzuki_engine::battle::{
    stack::{EffectState, HpChangeCause, TurnEvent, TurnLog},
    BattleState,
};
use pokered_data::{lang_data::move_name, moves::MoveId, species::Species};
use serde::{Deserialize, Serialize};

/// The existing text publication owner, without transient move/VFX state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextPagePublication {
    enabled: bool,
    page: Option<usize>,
    #[serde(default)]
    faints: Vec<Option<(FaintCue, bool, bool)>>,
    #[serde(default)]
    faint_serial: u32,
    #[serde(default)]
    active_faint: Option<(u32, FaintCue)>,
    #[serde(default)]
    faint_waiting: bool,
    #[serde(default)]
    level_ups: Vec<Option<LevelUpDisplay>>,
    #[serde(default)]
    transforms: Vec<Option<TransformDisplay>>,
    #[serde(default)]
    rests: Vec<Option<RestText>>,
    #[serde(default)]
    active_rest: Option<(u32, bool)>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct RestText {
    player: bool,
    hp: [u16; 2],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct TransformDisplay {
    player: bool,
    species: Species,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(super) struct LevelUpDisplay {
    pub active: bool,
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
}

#[derive(Debug, Clone)]
struct MoveStep {
    announcement: String,
    rest_sleep: bool,
    level_up: Option<LevelUpDisplay>,
    transform: Option<TransformDisplay>,
    faint: Option<FaintCue>,
    faint_continuation: bool,
    faint_text_published: bool,
    move_id: Option<MoveId>,
    effectiveness: pokered_data::types::Effectiveness,
    leech_seed: bool,
    charge: Option<MoveId>,
    missed: bool,
    player: bool,
    hp: [u16; 2],
    hits: Vec<[u16; 2]>,
}

impl MoveStep {
    fn faint(cue: FaintCue, announcement: String, hp: [u16; 2]) -> Self {
        Self {
            announcement, rest_sleep: false, level_up: None, transform: None, faint: Some(cue), faint_continuation: false,
            faint_text_published: false, move_id: None, effectiveness: pokered_data::types::Effectiveness::Normal, leech_seed: false,
            charge: None, missed: false, player: cue.side == super::FaintSide::Player,
            hp, hits: Vec::new(),
        }
    }

    fn level_up(announcement: String, display: LevelUpDisplay) -> Self {
        Self {
            announcement, rest_sleep: false, level_up: Some(display), transform: None, faint: None,
            faint_continuation: false, faint_text_published: false,
            move_id: None, effectiveness: pokered_data::types::Effectiveness::Normal,
            leech_seed: false, charge: None, missed: false, player: true,
            hp: [0; 2], hits: Vec::new(),
        }
    }

    fn transform(announcement: String, display: TransformDisplay) -> Self {
        Self {
            announcement, rest_sleep: false, level_up: None, transform: Some(display), faint: None,
            faint_continuation: false, faint_text_published: false,
            move_id: None, effectiveness: pokered_data::types::Effectiveness::Normal,
            leech_seed: false, charge: None, missed: false, player: display.player,
            hp: [0; 2], hits: Vec::new(),
        }
    }

    fn rest_sleep(announcement: String, display: RestText) -> Self {
        Self {
            announcement, rest_sleep: true, level_up: None, transform: None, faint: None,
            faint_continuation: false, faint_text_published: false,
            move_id: Some(MoveId::Rest), effectiveness: pokered_data::types::Effectiveness::Normal,
            leech_seed: false, charge: None, missed: false, player: display.player,
            hp: display.hp, hits: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BattlePresentation {
    /// Frontends opt in only when they can acknowledge visual completion.
    pub enabled: bool,
    pub waiting: bool,
    queued: Vec<MoveStep>,
    pages: Vec<Option<MoveStep>>,
    page: Option<usize>,
    serial: u32,
    current: Option<(u32, MoveId, bool)>,
    current_leech_seed: Option<(u32, bool)>,
    current_charge: Option<(u32, MoveId, bool)>,
    current_faint: Option<(u32, FaintCue)>,
    hit_index: usize,
}
impl BattlePresentation {
    pub(crate) fn capture_text_publication(&self) -> TextPagePublication {
        TextPagePublication {
            enabled: self.enabled, page: self.page,
            faints: self.pages.iter().map(|step| step.as_ref().and_then(|step| {
                step.faint.map(|cue| (cue, step.faint_continuation, step.faint_text_published))
            })).collect(),
            faint_serial: self.serial, active_faint: self.current_faint,
            faint_waiting: self.current_faint.is_some() && self.waiting,
            level_ups: self.pages.iter().map(|step| step.as_ref().and_then(|step| step.level_up)).collect(),
            transforms: self.pages.iter().map(|step| step.as_ref().and_then(|step| step.transform)).collect(),
            rests: self.pages.iter().map(|step| step.as_ref().filter(|step| step.rest_sleep)
                .map(|step| RestText { player: step.player, hp: step.hp })).collect(),
            active_rest: self.current.filter(|_| self.waiting && self.page.is_some_and(|page| self.rest_sleep_at(page)))
                .map(|(serial, _, player)| (serial, player)),
        }
    }

    pub(crate) fn restore_text_publication(&mut self, saved: TextPagePublication) {
        self.enabled = saved.enabled;
        self.page = saved.page;
        if self.current_faint.is_some() { self.waiting = false; }
        self.current_faint = None;
        self.queued.retain(|step| step.faint.is_none() && step.level_up.is_none() && step.transform.is_none() && !step.rest_sleep);
        for page in &mut self.pages {
            if page.as_ref().is_some_and(|step| step.faint.is_some()) { *page = None; }
        }
        // This vector is positional: all-None ordinary pages must survive
        // capture -> restore -> capture just as pages with a faint cue do.
        let count = saved.faints.len().max(saved.level_ups.len()).max(saved.transforms.len()).max(saved.rests.len());
        let mut faints = saved.faints.into_iter();
        let mut level_ups = saved.level_ups.into_iter();
        let mut transforms = saved.transforms.into_iter();
        let mut rests = saved.rests.into_iter();
        self.pages = (0..count).map(|_| {
            let mut step = faints.next().flatten().map(|(cue, continuation, published)| {
                let mut step = MoveStep::faint(cue, String::new(), [0; 2]);
                step.faint_continuation = continuation;
                step.faint_text_published = published;
                step
            });
            if let Some(display) = level_ups.next().flatten() {
                step = Some(MoveStep::level_up(String::new(), display));
            }
            if let Some(display) = transforms.next().flatten() {
                step = Some(MoveStep::transform(String::new(), display));
            }
            if let Some(display) = rests.next().flatten() {
                step = Some(MoveStep::rest_sleep(String::new(), display));
            }
            step
        }).collect();
        self.serial = saved.faint_serial;
        self.current_faint = saved.active_faint;
        self.current = saved.active_rest.map(|(serial, player)| (serial, MoveId::Rest, player));
        self.waiting = saved.faint_waiting || saved.active_rest.is_some();
    }

    /// Old snapshots did not record publication. Preserve their prior ACK
    /// behavior only for this current page; future pages activate normally.
    pub(crate) fn restore_legacy_text_page(&mut self, page: usize) {
        self.page = Some(page);
    }

    pub fn current_faint(&self) -> Option<(u32, FaintCue)> {
        self.current_faint
    }

    pub(super) fn faint_at(&self, page: usize) -> Option<FaintCue> {
        self.pages.get(page).and_then(Option::as_ref).and_then(|step| step.faint)
    }

    /// Only the final sleep page ends in DONE. Earlier wrapped pages and the
    /// following RegainedHealthText retain their normal text acknowledgements.
    pub(super) fn rest_sleep_at(&self, page: usize) -> bool {
        self.pages.get(page).and_then(Option::as_ref).is_some_and(|step| step.rest_sleep)
    }

    pub(super) fn publish_faint_text(&mut self, page: usize) -> bool {
        let Some(step) = self.pages.get_mut(page).and_then(Option::as_mut) else { return false; };
        if step.faint.is_some_and(|cue| cue.text.is_some()) && !step.faint_text_published {
            step.faint_text_published = true;
            true
        } else { false }
    }

    pub(super) fn has_silent_faint(&self) -> bool {
        self.queued.iter().any(|step| step.faint.is_some_and(|cue| cue.text.is_none()))
    }

    pub fn current_leech_seed(&self) -> Option<(u32, bool)> {
        self.current_leech_seed
    }

    pub fn current_charge(&self) -> Option<(u32, MoveId, bool)> {
        self.current_charge
    }
    pub fn current_move_missed(&self) -> bool {
        self.page
            .and_then(|page| self.pages.get(page))
            .and_then(Option::as_ref)
            .is_some_and(|step| step.missed)
    }
    pub(super) fn charge_in_order(
        &mut self,
        messages: &mut Vec<String>,
        name: &str,
        move_id: MoveId,
        player: bool,
    ) {
        let used = format!("{} used {}!", name, move_name(move_id, false));
        let charged = format!("{} {}", name, super::charge_message(move_id));
        if let Some(index) = messages.iter().position(|message| message == &used) {
            messages[index] = charged.clone();
        } else {
            messages.push(charged.clone());
        }
        if let Some(step) = self
            .queued
            .iter_mut()
            .find(|step| step.player == player && step.move_id == Some(move_id))
        {
            step.announcement = charged;
            step.move_id = None;
            step.charge = Some(move_id);
        }
    }

    pub(super) fn called_move_in_order(
        &mut self,
        messages: &mut Vec<String>,
        name: &str,
        label: &str,
        resolved: MoveId,
        player: bool,
        failed: bool,
        initial_hp: [u16; 2],
    ) {
        let resolved_text = format!("{} used {}!", name, move_name(resolved, false));
        let index = messages
            .iter()
            .position(|message| message == &resolved_text);
        // A blocked actor never used the calling move either.
        if index.is_none() && !failed {
            return;
        }
        let index = index.unwrap_or(0);
        let announcement = format!("{name} used {label}!");
        messages.insert(index, announcement.clone());
        if failed {
            messages.insert(index + 1, "But it failed!".to_string());
        }
        if !self.enabled {
            return;
        }
        let step_index = self
            .queued
            .iter()
            .position(|step| step.player == player && step.move_id == Some(resolved))
            .unwrap_or(0);
        let hp = step_index
            .checked_sub(1)
            .map(|i| self.queued[i].hp)
            .unwrap_or(initial_hp);
        self.queued.insert(
            step_index,
            MoveStep {
                announcement,
                rest_sleep: false,
                level_up: None,
                transform: None,
                faint: None,
                faint_continuation: false,
                faint_text_published: false,
                move_id: Some(if label == "METRONOME" {
                    MoveId::Metronome
                } else {
                    MoveId::MirrorMove
                }),
                effectiveness: pokered_data::types::Effectiveness::Normal,
                charge: None,
                leech_seed: false,
                missed: failed,
                player,
                hp,
                hits: Vec::new(),
            },
        );
    }

    /// The original presentation byte recorded for this action, not a
    /// post-turn type lookup or a multiplier inferred from a translated page.
    pub fn current_hit_effectiveness(&self) -> pokered_data::types::Effectiveness {
        self.page.and_then(|page| self.pages.get(page)).and_then(Option::as_ref)
            .map_or(pokered_data::types::Effectiveness::Normal, |step| step.effectiveness)
    }

    pub fn current_move(&self) -> Option<(u32, MoveId, bool)> {
        self.current
    }
    pub fn manages_moves(&self) -> bool {
        self.enabled
            && self
                .pages
                .iter()
                .flatten()
                .any(|step| step.move_id.is_some())
    }
    pub(super) fn holds_hp(&self) -> bool {
        self.enabled && (!self.queued.is_empty() || self.pages.iter().any(Option::is_some))
    }

    pub(super) fn holds_level_up(&self) -> bool {
        self.enabled && self.queued.iter().chain(self.pages.iter().flatten())
            .any(|step| step.level_up.is_some_and(|display| display.active))
    }

    pub(super) fn holds_species(&self, player: bool) -> bool {
        self.enabled && self.queued.iter().chain(self.pages.iter().flatten())
            .any(|step| step.transform.is_some_and(|display| display.player == player))
    }

    /// The species copied by this action, for AnimationTransformMon's picture
    /// transfer before the result page publishes the stable display identity.
    pub fn current_transform_target(&self) -> Option<(bool, Species)> {
        let (_, move_id, player) = self.current?;
        if move_id != MoveId::Transform || self.current_move_missed() { return None; }
        self.pages.iter().skip(self.page? + 1).flatten().filter_map(|step| step.transform)
            .find(|display| display.player == player).map(|display| (player, display.species))
    }

    pub(super) fn queue_level_up(&mut self, announcement: String, display: LevelUpDisplay) {
        if self.enabled { self.queued.push(MoveStep::level_up(announcement, display)); }
    }

    pub(super) fn record_turn(
        &mut self,
        log: &TurnLog<PokeredRules>,
        state: &BattleState<PokeredRules>,
        effects: &[EffectState<PokeredRules>],
        mut hp: [u16; 2],
        faint_plan: Option<&super::faint::FaintPlan>,
        names: Option<&runtime::BattleNameContext>,
    ) {
        self.queued.clear();
        // Page matching must use narration's names, including nicknames and
        // the original identity retained after Transform changes species.
        for (event_index, event) in log.events.iter().enumerate() {
            if let Some(cue) = faint_plan.and_then(|plan| plan.at(event_index)) {
                self.queued.push(MoveStep::faint(cue, runtime::faint_caption(cue, state, names), hp));
            }
            if !self.enabled {
                if let TurnEvent::MoveUsed { actor, move_: MoveId::Rest } = event {
                    if super::pokered_rules::rest_had_status(actor.side == 0).is_some() {
                        // Headless callers publish resolved HP immediately and
                        // need no visual ACK, but DONE still belongs to the text.
                        self.queued.push(MoveStep::rest_sleep(runtime::rest_sleep_caption(state, *actor, names),
                            RestText { player: actor.side == 0, hp: [state.player_battlers[0].hp, state.opponent_battlers[0].hp] }));
                    }
                }
                continue;
            }
            if let Some(announcement) = runtime::presentation_hp_message_named(event, state, effects, names) {
                self.queued.push(MoveStep {
                    announcement,
                    rest_sleep: false,
                    level_up: None,
                    transform: None,
                    faint: None,
                    faint_continuation: false,
                    faint_text_published: false,
                    hits: Vec::new(),
                    move_id: None,
                    effectiveness: pokered_data::types::Effectiveness::Normal,
                    charge: None,
                    missed: false,
                    leech_seed: matches!(
                        event,
                        TurnEvent::Damaged {
                            cause: Some(HpChangeCause::Volatile(PokeVolatile::LeechSeed)),
                            ..
                        }
                    ),
                    // The absorbing animation originates from the OTHER side.
                    player: matches!(event, TurnEvent::Damaged { target, .. } if target.side != 0),
                    hp,
                });
            }
            match event {
                TurnEvent::MoveUsed { actor, move_ } => {
                    let missed = super::pokered_rules::presented_move_missed(actor.side == 0, *move_).unwrap_or(false);
                    let rest_sleep = *move_ == MoveId::Rest && !missed
                        && super::pokered_rules::rest_had_status(actor.side == 0).is_some();
                    self.queued.push(MoveStep {
                        announcement: if rest_sleep {
                            // HealEffect prints the sleep result before playing
                            // Rest's animation and updating the HP bar.
                            runtime::rest_sleep_caption(state, *actor, names)
                        } else {
                            runtime::move_action_caption(state, *actor, *move_, names)
                        },
                        rest_sleep,
                        level_up: None,
                        transform: None,
                        hits: {
                            let recorded = super::pokered_rules::presented_hits(actor.side == 0);
                            let mut staged = hp;
                            let mut hits = Vec::new();
                            if recorded[5] > 1 {
                                for loss in recorded.iter().take(recorded[5] as usize) {
                                    let target = usize::from(actor.side == 0);
                                    staged[target] = staged[target].saturating_sub(*loss);
                                    hits.push(staged);
                                }
                            }
                            hits
                        },
                        faint: None,
                        faint_continuation: false,
                        faint_text_published: false,
                        move_id: Some(*move_),
                        effectiveness: super::pokered_rules::presented_effectiveness(actor.side == 0, *move_)
                            .unwrap_or(pokered_data::types::Effectiveness::Normal),
                        leech_seed: false,
                        charge: None,
                        missed,
                        player: actor.side == 0,
                        hp,
                    });
                    if *move_ == MoveId::Transform && !missed && effects.iter().any(|entry|
                        entry.host == *actor && matches!(entry.kind, PokeVolatile::Transformed)) {
                        let (caption, species) = runtime::transformed_result(state, *actor, names);
                        self.queued.push(MoveStep::transform(caption, TransformDisplay { player: actor.side == 0, species }));
                    }
                },
                TurnEvent::Missed { actor } => {
                    if let Some(step) = self
                        .queued
                        .iter_mut()
                        .rev()
                        .find(|step| step.move_id.is_some() && step.player == (actor.side == 0))
                    {
                        step.missed = true;
                    }
                }
                TurnEvent::Damaged { target, amount, .. } => {
                    hp[target.side as usize] = hp[target.side as usize].saturating_sub(*amount)
                }
                TurnEvent::Healed { target, amount, .. } => {
                    hp[target.side as usize] = hp[target.side as usize].saturating_add(*amount)
                }
                _ => {}
            }
            if let Some(step) = self.queued.iter_mut().rev().find(|step| step.faint.is_none()) {
                step.hp = hp;
            }
        }
    }

    pub(super) fn begin_pages(&mut self) {
        self.pages.clear();
        self.page = None;
        self.current = None;
        self.current_leech_seed = None;
        self.current_charge = None;
        self.current_faint = None;
        self.waiting = false;
    }
    pub(super) fn add_pages(&mut self, message: &str, count: usize) {
        let step = self
            .queued
            .iter()
            .position(|s| s.announcement == message)
            .map(|i| self.queued.remove(i));
        if step.as_ref().is_some_and(|step| step.faint.is_some()) {
            let step = step.unwrap();
            // The slide/sound precedes the FIRST text page. Later localized
            // pages retain the same typed identity without replaying it.
            self.pages.extend((0..count).map(|page| {
                let mut page_step = step.clone();
                page_step.faint_continuation = page != 0;
                Some(page_step)
            }));
        } else if step.as_ref().is_some_and(|step| step.level_up.is_some() || step.transform.is_some()) {
            // Growth HUD and copied identity updates precede the first result
            // page, including translated or long captions spanning pages.
            if count > 0 {
                self.pages.push(step);
                self.pages.extend((1..count).map(|_| None));
            }
        } else {
            self.pages.extend((0..count).map(|_| None));
            if count > 0 { *self.pages.last_mut().unwrap() = step; }
        }
    }
    /// Publish one page. A frontend must present a newly activated page
    /// before its acknowledgement can advance the caller.
    pub(super) fn activate(&mut self, page: usize, publish_text_without_animation: bool) -> bool {
        if (!self.enabled && !publish_text_without_animation) || self.page == Some(page) {
            return false;
        }
        self.page = Some(page);
        // LinkBattleDriver renders its text through the app's screen mirror,
        // but does not wait for move-animation acknowledgements. Its pages
        // still need one publication before input may dismiss them.
        if !self.enabled { return true; }
        let step = self.pages.get(page).and_then(Option::as_ref);
        self.waiting = step.is_some_and(|step| !step.faint_continuation && step.level_up.is_none() && step.transform.is_none());
        self.hit_index = 0;
        if !step.is_some_and(|step| step.faint_continuation) {
            self.current_faint = step.and_then(|step| step.faint).map(|cue| {
                self.serial = self.serial.wrapping_add(1);
                (self.serial, cue)
            });
        }
        self.current_charge = step.and_then(|step| {
            step.charge.map(|move_id| {
                self.serial = self.serial.wrapping_add(1);
                (self.serial, move_id, step.player)
            })
        });
        self.current_leech_seed = step.filter(|step| step.leech_seed).map(|step| {
            self.serial = self.serial.wrapping_add(1);
            (self.serial, step.player)
        });
        self.current = step.and_then(|step| {
            step.move_id.map(|move_id| {
                self.serial = self.serial.wrapping_add(1);
                (self.serial, move_id, step.player)
            })
        });
        true
    }
    pub(super) fn next_hit(&mut self) -> bool {
        let Some(step) = self
            .page
            .and_then(|page| self.pages.get(page))
            .and_then(Option::as_ref)
        else {
            return false;
        };
        if !self.waiting && self.hit_index > 0 && self.hit_index < step.hits.len() {
            self.serial = self.serial.wrapping_add(1);
            self.current = step.move_id.map(|id| (self.serial, id, step.player));
            self.waiting = true;
            return true;
        }
        false
    }
    pub(super) fn finish_pages(&mut self) {
        self.queued.clear();
        self.begin_pages();
    }
    pub(super) fn finish_preparing(&mut self) {
        self.queued.clear();
    }
}

impl BattleScreen {
    pub(super) fn activate_text_page(&mut self, page: usize) -> bool {
        let first_publication = self.presentation.activate(page, self.link_mode);
        if first_publication {
            if let Some(display) = self.presentation.pages.get(page)
                .and_then(Option::as_ref).and_then(|step| step.transform) {
                if display.player { self.player_species = display.species; }
                else { self.enemy_species = display.species; }
            }
            if let Some(display) = self.presentation.pages.get(page)
                .and_then(Option::as_ref).and_then(|step| step.level_up).filter(|display| display.active) {
                self.player_level = display.level;
                self.player_max_hp = display.max_hp;
                self.hp_bar_anim.set_target(
                    BattleSide::Player, display.hp, display.max_hp, self.player_species,
                    &mut self.player_hp,
                );
            }
        }
        first_publication
    }

    pub fn set_presentation_enabled(&mut self, enabled: bool) {
        if self.presentation.enabled == enabled {
            return;
        }
        self.presentation.enabled = enabled;
        if enabled {
            self.sync_display_from_state();
        }
    }

    /// Called once the frontend has finished the move AND its impact effects.
    pub fn complete_move_presentation(&mut self) {
        if !self.presentation.waiting {
            return;
        }
        self.presentation.waiting = false;
        let Some(step) = self
            .presentation
            .page
            .and_then(|page| self.presentation.pages.get(page))
            .and_then(Option::as_ref)
        else {
            return;
        };
        if let Some(cue) = step.faint {
            // Explicit primary calls, using ShowingText's existing VBlank wait:
            // PlayBattleVictoryMusic -> Delay3 (core.asm:959-967), and the
            // visible faint PrintText -> Delay3 (home/window.asm:283-292).
            // A silent remove does not run PrintText or acquire an ACK.
            if let super::BattlePhase::ShowingText { wait_frames, .. } = &mut self.phase {
                *wait_frames = if cue.sound == Some(super::FaintSound::WildVictory) { 3 } else { 0 }
                    + if cue.text.is_some() { 3 } else { 0 };
            }
            return;
        }
        let hp = step
            .hits
            .get(self.presentation.hit_index)
            .copied()
            .unwrap_or(step.hp);
        self.presentation.hit_index += 1;
        self.hp_bar_anim.set_target(
            BattleSide::Player,
            hp[0],
            self.player_max_hp,
            self.player_species,
            &mut self.player_hp,
        );
        self.hp_bar_anim.set_target(
            BattleSide::Enemy,
            hp[1],
            self.enemy_max_hp,
            self.enemy_species,
            &mut self.enemy_hp,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        battle::{BattleInput, BattlePhase},
        pokemon::stats::create_pokemon_with_moves,
    };
    use pokered_data::species::Species;

    fn rest_sleep_screen(enemy: bool, poisoned: bool) -> BattleScreen {
        use crate::{battle::state::StatusCondition, link::rng::LinkRng};
        let make = |species, mv| create_pokemon_with_moves(species, 100, [0x9a, 0x78],
            [mv, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        let mut actor = make(Species::Chansey, MoveId::Rest); actor.set_nickname("WARD"); actor.hp -= 30;
        if poisoned { actor.status = StatusCondition::Poison; }
        let other = make(Species::Snorlax, MoveId::Splash);
        let (player, foe) = if enemy { (other, actor) } else { (actor, other) };
        let mut b = BattleScreen::from_parties(true, &[player], &[foe], None);
        b.link_rng = Some(LinkRng::new([255; 10])); b.link_enemy_move_override = Some((foe.moves[0], 0));
        let bs = b.battle_state.as_mut().unwrap();
        bs.player.active_mon_mut().speed = if enemy { 100 } else { 800 };
        bs.enemy.active_mon_mut().speed = if enemy { 800 } else { 100 };
        b.set_presentation_enabled(true); b.phase = BattlePhase::PlayerMenu;
        let a = BattleInput { a: true, ..BattleInput::none() }; b.update_frame(a); b.update_frame(a);
        for frame in 0..5000 {
            if b.presentation.waiting && b.presentation.current_move().is_some_and(|(_, mv, _)| mv == MoveId::Rest) { return b; }
            if b.presentation.waiting { b.complete_move_presentation(); }
            b.update_frame(if frame % 2 == 0 { a } else { BattleInput::none() });
        }
        panic!("missing Rest animation");
    }

    fn roundtrip_rest(b: &BattleScreen) -> BattleScreen {
        use crate::snapshot::BattleSnapshot;
        let json = serde_json::to_value(BattleSnapshot::capture(b)).unwrap();
        let saved: BattleSnapshot = serde_json::from_value(json.clone()).unwrap();
        let mut restored = BattleScreen::new(true); saved.restore_into(&mut restored);
        assert_eq!(serde_json::to_value(BattleSnapshot::capture(&restored)).unwrap(), json);
        restored
    }

    fn compare_rest_without_input(b: &mut BattleScreen, restored: &mut BattleScreen) {
        for _ in 0..200 {
            b.update_frame(BattleInput::none()); restored.update_frame(BattleInput::none());
            assert_eq!((b.player_hp, b.enemy_hp), (restored.player_hp, restored.enemy_hp));
            assert_eq!(b.current_message, restored.current_message);
        }
        assert!(b.current_message.as_ref().is_some_and(|m| m.contains("regained health!")));
        assert!(!b.hp_bar_anim.is_active() && !restored.hp_bar_anim.is_active());
    }

    #[test]
    fn rest_snapshot_keeps_pending_animation_and_its_hp_completion() {
        for enemy in [false, true] { for poisoned in [false, true] {
            let mut b = rest_sleep_screen(enemy, poisoned); let mut restored = roundtrip_rest(&b);
            assert_eq!(restored.presentation.current_move(), b.presentation.current_move());
            assert!(restored.presentation.waiting);
            let message = restored.current_message.clone();
            for _ in 0..50 { restored.update_frame(BattleInput::none()); }
            assert_eq!(restored.current_message, message); assert!(restored.presentation.waiting);
            b.complete_move_presentation(); restored.complete_move_presentation();
            compare_rest_without_input(&mut b, &mut restored);
        }}
    }

    #[test]
    fn rest_snapshot_during_refill_or_result_does_not_replay_animation() {
        for enemy in [false, true] { for poisoned in [false, true] { for elapsed in [0, 5, 100] {
            let mut b = rest_sleep_screen(enemy, poisoned); b.complete_move_presentation();
            for _ in 0..elapsed { b.update_frame(BattleInput::none()); }
            let mut restored = roundtrip_rest(&b);
            assert!(!restored.presentation.waiting); assert!(restored.presentation.current_move().is_none());
            compare_rest_without_input(&mut b, &mut restored);
        }}}
    }

    #[test]
    fn snapshots_without_rest_metadata_retain_the_old_current_page_ack() {
        use crate::snapshot::BattleSnapshot;
        for enemy in [false, true] {
            let mut b = rest_sleep_screen(enemy, false); b.complete_move_presentation();
            let message = b.current_message.clone();
            let mut json = serde_json::to_value(BattleSnapshot::capture(&b)).unwrap();
            let publication = json["text_publication"].as_object_mut().unwrap();
            publication.remove("rests"); publication.remove("active_rest");
            let saved: BattleSnapshot = serde_json::from_value(json).unwrap(); saved.restore_into(&mut b);
            for _ in 0..600 { b.update_frame(BattleInput::none()); }
            assert_eq!(b.current_message, message); assert!(!b.presentation.waiting);
            b.update_frame(BattleInput { a: true, ..BattleInput::none() });
            for _ in 0..20 { b.update_frame(BattleInput::none()); }
            assert!(b.current_message.as_ref().is_some_and(|m| m.contains("regained health!")));
        }
    }

    fn growth_screen(level: u8, active: usize, transformed: bool) -> BattleScreen {
        let make = |species, level| create_pokemon_with_moves(species, level, [0x9a, 0x78],
            [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
        let mut mon = make(Species::Mewtwo, level); mon.hp -= 7; mon.set_nickname("ACE");
        let party = if level == 5 { vec![mon, mon] } else { vec![mon] };
        let mut enemy = make(Species::Snorlax, 100); enemy.hp = 0;
        let mut battle = BattleScreen::from_parties(true, &party, &[enemy], None);
        let bs = battle.battle_state.as_mut().unwrap();
        bs.player.active_pokemon_index = active;
        bs.party_gain_exp_flags[0] = true;
        if level == 5 { bs.party_gain_exp_flags[1] = true; }
        if transformed {
            bs.player.preserve_original_identity();
            bs.player.active_mon_mut().species = Species::Snorlax;
            bs.player.set_status3(super::super::state::status3::TRANSFORMED);
        }
        battle.set_presentation_enabled(true);
        battle
    }

    #[test]
    fn matching_bench_and_active_growth_captions_keep_party_order() {
        for active in [0, 1] {
            let mut battle = growth_screen(5, active, false);
            let messages = battle.process_exp_gain();
            assert_eq!(messages.iter().filter(|m| *m == "ACE grew to level 10!").count(), 2);
            battle.show_text_then(messages, BattlePhase::PlayerMenu);
            let mut previous = None; let mut growth_count = 0;
            for _ in 0..1000 {
                if battle.current_message != previous {
                    if battle.current_message.as_deref() == Some("ACE grew to level 10!") {
                        growth_count += 1;
                        let expected = if growth_count > active { (10, 36, 43) } else { (5, 19, 26) };
                        assert_eq!((battle.player_level, battle.player_hp, battle.player_max_hp), expected);
                        assert!(!battle.presentation.waiting);
                    }
                    previous = battle.current_message.clone();
                }
                if battle.phase == BattlePhase::PlayerMenu { break; }
                battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
            }
            assert_eq!(growth_count, 2);
            assert_eq!(battle.phase, BattlePhase::PlayerMenu);
        }
    }

    #[test]
    fn transformed_growth_updates_hp_and_level_without_replacing_battle_species() {
        let mut battle = growth_screen(20, 0, true);
        let messages = battle.process_exp_gain();
        battle.show_text_then(messages, BattlePhase::PlayerMenu);
        assert_eq!((battle.player_level, battle.player_hp, battle.player_max_hp), (20, 69, 76));
        for _ in 0..1000 {
            if battle.current_message.as_deref() == Some("ACE grew to level 21!") {
                assert_eq!((battle.player_level, battle.player_hp, battle.player_max_hp), (21, 73, 80));
                assert_eq!(battle.player_species, Species::Snorlax);
                return;
            }
            battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        }
        panic!("missing transformed growth notice");
    }

    #[test]
    fn growth_first_page_and_snapshot_roundtrip_publish_one_hud_update() {
        use crate::snapshot::BattleSnapshot;
        let mut battle = growth_screen(20, 0, false);
        let text = "ACE grew to level 21! This prepared caption spans several pages to exercise the first publication.".to_string();
        battle.presentation.queue_level_up(text.clone(), LevelUpDisplay { active: true, level: 21, hp: 73, max_hp: 80 });
        battle.show_text_then(vec!["Gained experience".to_string(), text.clone()], BattlePhase::PlayerMenu);
        for _ in 0..1000 {
            let json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
            let saved: BattleSnapshot = serde_json::from_value(json.clone()).unwrap();
            saved.restore_into(&mut battle);
            assert_eq!(serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap(), json);
            if battle.current_message.as_ref().is_some_and(|m| m.starts_with("ACE grew")) {
                assert_ne!(battle.current_message.as_ref().unwrap(), &text);
                assert_eq!((battle.player_level, battle.player_hp, battle.player_max_hp), (21, 73, 80));
                assert!(!battle.hp_bar_anim.is_active());
                assert!(!battle.presentation.waiting);
                battle.update_frame(BattleInput::none());
                assert_eq!((battle.player_level, battle.player_hp, battle.player_max_hp), (21, 73, 80));
                return;
            }
            battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        }
        panic!("missing first growth page");
    }

    #[test]
    fn publication_snapshots_without_growth_metadata_keep_existing_pages() {
        use crate::snapshot::BattleSnapshot;
        let battle = second_plain_text_page(true);
        let mut json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
        json["text_publication"].as_object_mut().unwrap().remove("level_ups");
        let saved: BattleSnapshot = serde_json::from_value(json).unwrap();
        let mut restored = BattleScreen::new(true); saved.restore_into(&mut restored);
        restored.update_frame(BattleInput::none());
        assert_eq!(restored.current_message.as_deref(), Some("Second page"));
        assert!(!restored.presentation.waiting);
    }

    #[test]
    fn transform_first_result_page_and_snapshot_roundtrip_publish_identity() {
        use crate::snapshot::BattleSnapshot;
        for player in [false, true] {
            let make = |species| create_pokemon_with_moves(species, 40, [0x9a, 0x78],
                [MoveId::Splash, MoveId::None, MoveId::None, MoveId::None]).unwrap();
            let mut battle = BattleScreen::from_parties(true, &[make(Species::Mewtwo)], &[make(Species::Mewtwo)], None);
            battle.set_presentation_enabled(true);
            let bs = battle.battle_state.as_mut().unwrap();
            let side = if player { &mut bs.player } else { &mut bs.enemy };
            side.active_mon_mut().species = Species::Snorlax;
            let caption = "COPYMASTER transformed into SNORLAX! This prepared result spans several text pages.".to_string();
            battle.presentation.queued.push(MoveStep::transform(caption.clone(), TransformDisplay { player, species: Species::Snorlax }));
            battle.sync_display_from_state();
            assert_eq!(if player { battle.player_species } else { battle.enemy_species }, Species::Mewtwo);
            battle.show_text_then(vec!["COPYMASTER used TRANSFORM!".to_string(), caption.clone()], BattlePhase::PlayerMenu);
            let mut seen = false;
            for _ in 0..1000 {
                let json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
                let saved: BattleSnapshot = serde_json::from_value(json.clone()).unwrap();
                saved.restore_into(&mut battle);
                assert_eq!(serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap(), json);
                if battle.current_message.as_ref().is_some_and(|m| m.contains("transformed")) {
                    assert_ne!(battle.current_message.as_ref().unwrap(), &caption);
                    assert_eq!(if player { battle.player_species } else { battle.enemy_species }, Species::Snorlax);
                    assert!(!battle.presentation.waiting);
                    seen = true; break;
                }
                battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
            }
            assert!(seen, "missing first result page");
        }
    }

    #[test]
    fn publication_snapshots_without_transform_metadata_keep_existing_pages() {
        use crate::snapshot::BattleSnapshot;
        let battle = second_plain_text_page(true);
        let mut json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
        json["text_publication"].as_object_mut().unwrap().remove("transforms");
        let saved: BattleSnapshot = serde_json::from_value(json).unwrap();
        let mut restored = BattleScreen::new(true); saved.restore_into(&mut restored);
        restored.update_frame(BattleInput::none());
        assert_eq!(restored.current_message.as_deref(), Some("Second page"));
        assert!(!restored.presentation.waiting);
    }

    fn second_plain_text_page(presentation_enabled: bool) -> BattleScreen {
        let mut battle = BattleScreen::new(true);
        battle.set_presentation_enabled(presentation_enabled);
        battle.show_text_then(
            vec!["First page".to_string(), "Second page".to_string()],
            BattlePhase::PlayerMenu,
        );
        for _ in 0..crate::battle::BATTLE_TEXT_PAGE_WAIT_FRAMES {
            battle.update_frame(BattleInput::none());
        }
        battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert!(matches!(battle.phase, BattlePhase::ShowingText { current: 1, .. }));
        for _ in 0..crate::battle::BATTLE_TEXT_PAGE_WAIT_FRAMES {
            battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        }
        assert!(matches!(battle.phase, BattlePhase::ShowingText { current: 1, wait_frames: 0, .. }));
        assert_eq!(battle.current_message.as_deref(), Some("First page"));
        battle
    }

    #[test]
    fn frontend_page_is_published_before_ack_and_needs_no_extra_wait() {
        let mut battle = second_plain_text_page(true);
        battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(battle.current_message.as_deref(), Some("Second page"));
        assert!(matches!(battle.phase, BattlePhase::ShowingText { current: 1, wait_frames: 0, .. }));
        assert!(!battle.presentation.waiting, "plain text has no animation or sound wait");
        battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(battle.phase, BattlePhase::PlayerMenu);
    }

    #[test]
    fn core_only_text_ack_does_not_wait_for_frontend_publication() {
        let mut battle = second_plain_text_page(false);
        battle.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(battle.phase, BattlePhase::PlayerMenu);
    }

    #[test]
    fn publication_snapshot_distinguishes_pending_and_published_second_pages() {
        use crate::snapshot::BattleSnapshot;
        for page in [None, Some(0), Some(1)] {
            let mut battle = second_plain_text_page(page.is_some());
            if page.is_none() {
                // A core-driven text caller can enable its frontend later.
                // Its existing publication owner has no active page yet.
                battle.set_presentation_enabled(true);
            } else if page == Some(1) {
                battle.update_frame(BattleInput::none());
            }
            let json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
            assert_eq!(json["text_publication"]["enabled"], true);
            assert_eq!(json["text_publication"]["page"], serde_json::to_value(page).unwrap());
            let saved: BattleSnapshot = serde_json::from_value(json).unwrap();
            assert!(saved.text_publication.is_some(), "Some(page=None) is not a legacy snapshot");
            let mut restored = BattleScreen::new(true);
            saved.restore_into(&mut restored);
            restored.update_frame(BattleInput { a: true, ..BattleInput::none() });
            if page == Some(1) {
                assert_eq!(restored.phase, BattlePhase::PlayerMenu, "published page needs only one ACK");
            } else {
                assert_eq!(restored.current_message.as_deref(), Some("Second page"));
                assert!(matches!(restored.phase, BattlePhase::ShowingText { current: 1, wait_frames: 0, .. }));
                restored.update_frame(BattleInput { a: true, ..BattleInput::none() });
                assert_eq!(restored.phase, BattlePhase::PlayerMenu);
            }
        }
    }

    #[test]
    fn publication_snapshot_keeps_core_only_ack_without_frontend_wait() {
        use crate::snapshot::BattleSnapshot;
        let battle = second_plain_text_page(false);
        let json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
        let saved: BattleSnapshot = serde_json::from_value(json).unwrap();
        let mut restored = BattleScreen::new(true);
        saved.restore_into(&mut restored);
        assert!(!restored.presentation.enabled);
        restored.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(restored.phase, BattlePhase::PlayerMenu);
    }

    #[test]
    fn legacy_snapshot_keeps_current_ack_and_future_page_publication_barrier() {
        use crate::snapshot::BattleSnapshot;
        let mut battle = BattleScreen::new(true);
        battle.set_presentation_enabled(true);
        battle.show_text_then(
            vec!["First page".to_string(), "Second page".to_string()],
            BattlePhase::PlayerMenu,
        );
        for _ in 0..crate::battle::BATTLE_TEXT_PAGE_WAIT_FRAMES {
            battle.update_frame(BattleInput::none());
        }
        let mut json = serde_json::to_value(BattleSnapshot::capture(&battle)).unwrap();
        json.as_object_mut().unwrap().remove("text_publication");
        let saved: BattleSnapshot = serde_json::from_value(json).unwrap();
        assert!(saved.text_publication.is_none());
        let mut restored = BattleScreen::new(true);
        saved.restore_into(&mut restored);
        restored.set_presentation_enabled(true);
        restored.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert!(matches!(restored.phase, BattlePhase::ShowingText { current: 1, .. }),
            "legacy current page keeps its old one-ACK behavior");
        for _ in 0..crate::battle::BATTLE_TEXT_PAGE_WAIT_FRAMES {
            restored.update_frame(BattleInput::none());
        }
        restored.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(restored.current_message.as_deref(), Some("Second page"));
        assert!(matches!(restored.phase, BattlePhase::ShowingText { current: 1, wait_frames: 0, .. }),
            "a future page must still be published before its ACK");
        restored.update_frame(BattleInput { a: true, ..BattleInput::none() });
        assert_eq!(restored.phase, BattlePhase::PlayerMenu);
    }

    #[test]
    fn both_speed_orders_wait_for_each_animation_then_drain_only_its_damage() {
        for player_first in [true, false] {
            for chinese in [false, true] {
                let make = |species| {
                    create_pokemon_with_moves(
                        species,
                        30,
                        [0xff; 2],
                        [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
                    )
                    .unwrap()
                };
                let mut battle = BattleScreen::from_parties(
                    true,
                    &[make(Species::Charmander)],
                    &[make(Species::Bulbasaur)],
                    None,
                );
                battle.rng = runtime::StdBattleRng::from_seed(42);
                let bs = battle.battle_state.as_mut().unwrap();
                bs.player.active_mon_mut().speed = if player_first { 200 } else { 10 };
                bs.enemy.active_mon_mut().speed = if player_first { 10 } else { 200 };
                battle.is_zh = chinese;
                battle.set_presentation_enabled(true);
                let original = [battle.player_hp, battle.enemy_hp];
                battle.execute_turn_with_move(0);
                assert_eq!(
                    [battle.player_hp, battle.enemy_hp],
                    original,
                    "resolution must not expose final HP"
                );
                let mut sides = Vec::new();
                for _ in 0..1000 {
                    if battle.presentation.waiting {
                        let action = battle.presentation.current_move().unwrap();
                        sides.push(action.2);
                        let hp = [battle.player_hp, battle.enemy_hp];
                        for _ in 0..40 {
                            battle.update_frame(BattleInput {
                                a: true,
                                ..BattleInput::none()
                            });
                        }
                        assert_eq!(battle.presentation.current_move(), Some(action));
                        assert_eq!(
                            [battle.player_hp, battle.enemy_hp],
                            hp,
                            "even mashing A must wait for animation"
                        );
                        battle.complete_move_presentation();
                        for _ in 0..150 {
                            battle.update_frame(BattleInput::none());
                        }
                        if sides.len() == 1 {
                            let attacker = if player_first { 0 } else { 1 };
                            assert_eq!(
                                [battle.player_hp, battle.enemy_hp][attacker],
                                original[attacker],
                                "opponent's damage must still wait"
                            );
                        }
                    }
                    battle.update_frame(BattleInput {
                        a: true,
                        ..BattleInput::none()
                    });
                    if matches!(battle.phase, BattlePhase::PlayerMenu) {
                        break;
                    }
                }
                assert_eq!(sides, vec![player_first, !player_first]);
                let bs = battle.battle_state.as_ref().unwrap();
                assert_eq!(
                    [battle.player_hp, battle.enemy_hp],
                    [bs.player.active_mon().hp, bs.enemy.active_mon().hp]
                );
            }
        }
    }
    #[test]
    fn enemy_free_turn_stages_damage_and_poison_at_distinct_pages() {
        use crate::battle::state::StatusCondition;
        let make = |species| {
            create_pokemon_with_moves(
                species,
                30,
                [0xff; 2],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(Species::Charmander)],
            &[make(Species::Bulbasaur)],
            None,
        );
        battle
            .battle_state
            .as_mut()
            .unwrap()
            .enemy
            .active_mon_mut()
            .status = StatusCondition::Poison;
        battle.rng = runtime::StdBattleRng::from_seed(42);
        battle.set_presentation_enabled(true);
        let original = [battle.player_hp, battle.enemy_hp];
        battle.run_enemy_free_turn_stack(Vec::new(), None);
        assert_eq!([battle.player_hp, battle.enemy_hp], original);
        let steps: Vec<_> = battle.presentation.pages.iter().flatten().collect();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].move_id, Some(MoveId::Tackle));
        assert!(!steps[0].player);
        assert_eq!(
            steps[0].hp[1], original[1],
            "poison must not be folded into the move"
        );
        assert!(steps[0].hp[0] < original[0]);
        assert_eq!(steps[1].move_id, None);
        assert!(steps[1].announcement.contains("POISON"));
        assert!(steps[1].hp[1] < original[1]);
    }
    #[test]
    fn knockout_has_one_animation_and_hides_damage_until_it_finishes() {
        let make = |species, level| {
            create_pokemon_with_moves(
                species,
                level,
                [0xff; 2],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(Species::Charizard, 80)],
            &[make(Species::Bulbasaur, 5)],
            None,
        );
        battle.rng = runtime::StdBattleRng::from_seed(42);
        battle.set_presentation_enabled(true);
        let hp = battle.enemy_hp;
        battle.execute_turn_with_move(0);
        assert_eq!(battle.enemy_hp, hp);
        let actions: Vec<_> = battle
            .presentation
            .pages
            .iter()
            .flatten()
            .filter(|step| step.move_id.is_some())
            .collect();
        assert_eq!(actions.len(), 1);
        assert!(actions[0].player);
        assert_eq!(actions[0].hp[1], 0);
        assert!(battle.presentation.waiting);
        battle.complete_move_presentation();
        for _ in 0..200 {
            battle.update_frame(BattleInput::none());
        }
        assert_eq!(battle.enemy_hp, 0);
    }

    #[test]
    fn charge_turn_is_not_a_strike_and_stays_before_the_other_move() {
        for player in [true, false] {
            for mv in [
                MoveId::Fly,
                MoveId::Dig,
                MoveId::Solarbeam,
                MoveId::RazorWind,
                MoveId::SkullBash,
                MoveId::SkyAttack,
            ] {
                let make = |id| {
                    create_pokemon_with_moves(
                        Species::Mew,
                        50,
                        [0xff; 2],
                        [id, MoveId::None, MoveId::None, MoveId::None],
                    )
                    .unwrap()
                };
                let mut battle = BattleScreen::from_parties(
                    true,
                    &[make(if player { mv } else { MoveId::Splash })],
                    &[make(if player { MoveId::Splash } else { mv })],
                    None,
                );
                let bs = battle.battle_state.as_mut().unwrap();
                bs.player.active_mon_mut().speed = if player { 200 } else { 10 };
                bs.enemy.active_mon_mut().speed = if player { 10 } else { 200 };
                battle.set_presentation_enabled(true);
                battle.execute_turn_with_move(0);
                let first = battle.presentation.pages.iter().flatten().next().unwrap();
                assert_eq!(first.charge, Some(mv));
                assert_eq!(first.move_id, None);
                assert_eq!(first.player, player);
                assert!(!first.announcement.contains(" used "));
            }
        }
    }

    #[test]
    fn misses_are_known_before_the_move_presentation_starts() {
        let make = |species| {
            create_pokemon_with_moves(
                species,
                30,
                [0xff; 2],
                [MoveId::Tackle, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(Species::Charmander)],
            &[make(Species::Gastly)],
            None,
        );
        battle.set_presentation_enabled(true);
        battle.execute_turn_with_move(0);
        let steps: Vec<_> = battle
            .presentation
            .pages
            .iter()
            .flatten()
            .filter(|step| step.move_id.is_some())
            .collect();
        assert!(
            steps.iter().any(|step| step.player && step.missed),
            "Normal attack into Ghost is a miss"
        );
    }

    #[test]
    fn called_move_plays_before_its_strike_and_after_the_faster_opponent() {
        for player in [true, false] {
            let make = |mv| {
                create_pokemon_with_moves(
                    Species::Mew,
                    50,
                    [0xff; 2],
                    [mv, MoveId::None, MoveId::None, MoveId::None],
                )
                .unwrap()
            };
            let mut battle = BattleScreen::from_parties(
                true,
                &[make(if player {
                    MoveId::MirrorMove
                } else {
                    MoveId::Tackle
                })],
                &[make(if player {
                    MoveId::Tackle
                } else {
                    MoveId::MirrorMove
                })],
                None,
            );
            let bs = battle.battle_state.as_mut().unwrap();
            bs.player.last_move_used = MoveId::Tackle;
            bs.enemy.last_move_used = MoveId::Tackle;
            bs.player.active_mon_mut().speed = if player { 10 } else { 200 };
            bs.enemy.active_mon_mut().speed = if player { 200 } else { 10 };
            battle.rng = runtime::StdBattleRng::from_seed(42);
            battle.set_presentation_enabled(true);
            battle.execute_turn_with_move(0);
            let steps: Vec<_> = battle.presentation.pages.iter().flatten().collect();
            assert_eq!(steps.len(), 3);
            assert_eq!(steps[0].player, !player);
            assert_eq!(steps[1].player, player);
            assert_eq!(steps[1].move_id, Some(MoveId::MirrorMove));
            assert_eq!(steps[2].move_id, Some(MoveId::Tackle));
            assert_eq!(
                steps[1].hp, steps[0].hp,
                "calling animation cannot apply strike damage"
            );
        }
    }

    #[test]
    fn multi_hit_repeats_animation_and_drains_each_hit_before_next() {
        let make = |mv| {
            create_pokemon_with_moves(
                Species::Mew,
                50,
                [0xff; 2],
                [mv, MoveId::None, MoveId::None, MoveId::None],
            )
            .unwrap()
        };
        let mut battle = BattleScreen::from_parties(
            true,
            &[make(MoveId::DoubleKick)],
            &[make(MoveId::Splash)],
            None,
        );
        battle.rng = runtime::StdBattleRng::from_seed(42);
        battle
            .battle_state
            .as_mut()
            .unwrap()
            .player
            .active_mon_mut()
            .speed = 200;
        battle.set_presentation_enabled(true);
        let start = battle.enemy_hp;
        battle.execute_turn_with_move(0);
        let step = battle
            .presentation
            .pages
            .iter()
            .flatten()
            .find(|step| step.player)
            .unwrap();
        assert_eq!(step.hits.len(), 2);
        let hp = step.hits.clone();
        assert!(hp[0][1] < start && hp[1][1] < hp[0][1]);
        let first = battle.presentation.current_move().unwrap().0;
        battle.complete_move_presentation();
        for _ in 0..200 {
            battle.update_frame(BattleInput::none());
            if battle.presentation.waiting {
                break;
            }
        }
        assert_eq!(battle.enemy_hp, hp[0][1]);
        assert!(battle.presentation.waiting);
        assert_ne!(battle.presentation.current_move().unwrap().0, first);
        battle.complete_move_presentation();
        for _ in 0..200 {
            battle.update_frame(BattleInput::none());
        }
        assert_eq!(battle.enemy_hp, hp[1][1]);
    }
}
