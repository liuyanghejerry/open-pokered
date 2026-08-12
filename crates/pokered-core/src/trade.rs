//! In-game NPC trade support — port of `engine/events/in_game_trades.asm`.
//!
//! Two pieces:
//!
//! - [`assemble_npc_trade_mon`] builds the received Pokémon with the original's
//!   semantics: fixed nickname from the `TradeMons` table
//!   (`pokered_data::trades`), OT name `<TRAINER>`, random OT ID and random DVs
//!   (the original rolls both at trade time — see the data module's docs), and
//!   the level of the mon the player gave.
//! - [`TradeAnim`] is the frame-stepped state machine behind the trade
//!   cutscene (`engine/movie/trade.asm`, `InternalClockTradeFuncSequence`).
//!   Frontends render it and play its SFX events; the party mutation is
//!   applied only after the animation completes, matching the original order
//!   (anim → `RemovePokemon`/`AddPartyMon`).

use pokered_data::species::Species;
use pokered_data::trades::NPC_TRADE_OT_NAME;
use rand::Rng;

use crate::battle::obedience::is_traded_for;
use crate::battle::state::Pokemon;

/// Build the Pokémon received from an NPC in-game trade.
///
/// asm parity (`InGameTrade_DoTrade` → `AddPartyMon` with
/// `wMonDataLocation = $80` → `InGameTrade_CopyDataToReceivedMon`):
/// - `level` is the level of the mon the player gave (`wCurEnemyLevel`).
/// - `dv_bytes` are the two `call Random` bytes from `AddPartyMon`
///   (NOT the fixed $98/$88 enemy-trainer DVs).
/// - `ot_id` is the random 16-bit `wTradedEnemyMonOTID`.
/// - OT name is the literal `<TRAINER>`; nickname comes from the table.
/// - `is_traded` follows the original obedience rule: traded iff
///   `ot_id != 0 && ot_id != player_id` (a freak matching/zero roll means the
///   mon counts as the player's own, exactly like the original).
pub fn assemble_npc_trade_mon(
    species: Species,
    level: u8,
    nickname: &str,
    dv_bytes: [u8; 2],
    ot_id: u16,
    player_id: u16,
) -> Option<Pokemon> {
    let mut mon = crate::pokemon::stats::create_pokemon(species, level, dv_bytes)?;
    mon.set_nickname(nickname);
    // The OT name is stored as the literal "TRAINER" — the angle brackets of
    // the `<TRAINER>` table text are script markup (the CHAR_TRAINER control
    // code), not part of the name, and '<' has no charmap glyph.
    mon.ot_name = crate::battle::state::encode_name("TRAINER");
    mon.ot_id = ot_id;
    mon.is_traded = is_traded_for(ot_id, player_id);
    Some(mon)
}

/// Roll the trade-time randoms: two DV bytes (`AddPartyMon`: `call Random`
/// twice) and the 16-bit OT ID (`InGameTrade_PrepareTradeData`:
/// `hRandomAdd` → `wTradedEnemyMonOTID`).
pub fn roll_npc_trade_randoms(rng: &mut impl Rng) -> ([u8; 2], u16) {
    ([rng.gen(), rng.gen()], rng.gen())
}

/// [`roll_npc_trade_randoms`] on the thread RNG — what frontends use at trade
/// completion (deterministic tests inject a seeded RNG instead).
pub fn roll_npc_trade_randoms_thread() -> ([u8; 2], u16) {
    roll_npc_trade_randoms(&mut rand::thread_rng())
}

// ---------------------------------------------------------------------------
// Trade animation state machine
// ---------------------------------------------------------------------------

/// Sound effects the animation asks the frontend to play
/// (`engine/movie/trade.asm`: `SFX_HEAL_HP` when the cable connects,
/// `SFX_TINK` while the ball travels, `PlayCry` for each mon).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSfx {
    /// SFX_HEAL_HP — cable appears (`Trade_DrawOpenEndOfLinkCable`).
    CableConnect,
    /// SFX_TINK — ball moving through the link cable.
    BallTravel,
    /// Cry of the mon the player gives away (`Trade_ShowPlayerMon`).
    GiveMonCry,
    /// Cry of the mon the player receives (`Trade_ShowEnemyMon`).
    ReceiveMonCry,
}

/// Phases of the trade cutscene, mirroring `InternalClockTradeFuncSequence`.
/// The frame counts below are taken from the original routines: the window
/// slide-in loop, `Trade_Delay80`/`Trade_Delay100`, the sub-animation frame
/// counts × their `wSubAnimFrameDelay` (`data/moves/animations.asm`:
/// TradeBallPoof/Drop delay 6, TradeBallShake delay 4), the ball's
/// 32 × `Delay3` cable run, and `Trade_SlideTextBoxOffScreen`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeAnimPhase {
    /// `Trade_ShowPlayerMon` slide loop — the mon panel slides in from the
    /// right edge, rWX/hSCX $7e → 0 in 63 steps of 2px (63 frames).
    SlideInGiveMon,
    /// `Trade_Delay80` — the given mon's pic at rest.
    ShowGiveMon,
    /// `TRADE_BALL_POOF_ANIM` (Subanim_2TradeBallPoof: 3 frames × delay 6).
    GiveMonPoof,
    /// `TRADE_BALL_DROP_ANIM` (Subanim_2TradeBallDrop: 6 frames × delay 6) —
    /// clears the mon pic; the give mon's cry plays as it ends
    /// (`Trade_ShowPlayerMon`: drop anim → `PlayCry`).
    GiveMonBallDrop,
    /// `Trade_DrawOpenEndOfLinkCable` (SFX_HEAL_HP + 20-frame cable scroll)
    /// + `Trade_AnimateBallEnteringLinkCable`'s `TRADE_BALL_SHAKE_ANIM`
    /// (Subanim_2TradeBallShake: 4 frames × delay 4) + `DelayFrames 10`.
    BallEnterCable,
    /// The ball's run through the link cable: x $20 → $a0 in 32 steps of
    /// 4px, `Delay3` per step, SFX_TINK per step except the last
    /// (`Trade_AnimateBallEnteringLinkCable`).
    SlideOut,
    /// `_TradeWentToText` — "{GIVE} went to <TRAINER>."
    TextWentTo,
    /// `_TradeForText` + `_TradeSendsText`.
    TextForSends,
    /// `_TradeWavesFarewellText` + `_TradeTransferredText`.
    TextFarewell,
    /// `Trade_AnimRightToLeft` — the received mon travels back (modelled as
    /// the same ball-through-cable run in reverse).
    SlideBack,
    /// `Trade_ShowEnemyMon`'s `TRADE_BALL_TILT_ANIM`
    /// (Subanim_2TradeBallAppear: 1 frame × delay 6).
    ReceiveBallTilt,
    /// `Trade_ShowEnemyMon`'s `TRADE_BALL_POOF_ANIM` — the received mon's
    /// pic is revealed under the poof cloud; its cry plays as the poof ends
    /// (`Trade_ShowEnemyMon`: poof anim → `PlayCry`).
    ReceiveMonPoof,
    /// `Trade_Delay100` — the received mon's pic at rest.
    ShowReceiveMon,
    /// `_TradeTakeCareText` — "Take good care of {RECEIVE}." (+ Trade_Delay80).
    TextTakeCare,
    /// `Trade_SlideTextBoxOffScreen` — 50-frame hold, then the text box
    /// slides right 2px/frame until WX reaches $a1 (77 frames), then a
    /// 10-frame clear.
    SlideTextBoxOff,
    /// `Trade_Cleanup` — animation over; apply the party mutation.
    Done,
}

impl TradeAnimPhase {
    /// Frames (60 Hz) each phase lasts.
    fn duration(self) -> u16 {
        match self {
            Self::SlideInGiveMon => 63,
            Self::ShowGiveMon => 80,
            Self::GiveMonPoof => 18,
            Self::GiveMonBallDrop => 36,
            Self::BallEnterCable => 46,
            Self::SlideOut | Self::SlideBack => 96,
            Self::TextWentTo | Self::TextForSends | Self::TextFarewell => 80,
            Self::ReceiveBallTilt => 6,
            Self::ReceiveMonPoof => 18,
            Self::ShowReceiveMon => 100,
            Self::TextTakeCare => 80,
            Self::SlideTextBoxOff => 137,
            Self::Done => 0,
        }
    }

    fn next(self) -> Self {
        use TradeAnimPhase::*;
        match self {
            SlideInGiveMon => ShowGiveMon,
            ShowGiveMon => GiveMonPoof,
            GiveMonPoof => GiveMonBallDrop,
            GiveMonBallDrop => BallEnterCable,
            BallEnterCable => SlideOut,
            SlideOut => TextWentTo,
            TextWentTo => TextForSends,
            TextForSends => TextFarewell,
            TextFarewell => SlideBack,
            SlideBack => ReceiveBallTilt,
            ReceiveBallTilt => ReceiveMonPoof,
            ReceiveMonPoof => ShowReceiveMon,
            ShowReceiveMon => TextTakeCare,
            TextTakeCare => SlideTextBoxOff,
            SlideTextBoxOff | Done => Done,
        }
    }
}

/// Horizontal pixel bounds of the ball's cable run on the 160px screen:
/// OAM x $20 → $a0 (`Trade_AnimateBallEnteringLinkCable`), i.e. screen
/// x 24 → 152 after the +8 OAM bias.
pub const TRADE_BALL_MIN_X: i32 = 24;
pub const TRADE_BALL_MAX_X: i32 = 152;
/// Vertical pixel center of the link cable.
pub const TRADE_CABLE_Y: i32 = 56;
/// Frames each 4px cable step takes (`Delay3`).
pub const TRADE_BALL_STEP_FRAMES: u16 = 3;

/// The trade ball sub-animations (`data/battle_anims/subanimations.asm`
/// 0x48-0x4B), exposed so frontends can render the exact frame blocks via
/// the shared battle-animation data instead of hand-drawn stand-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeBallSubAnim {
    /// Subanim_2TradeBallPoof (0x4B): 3-frame poof cloud.
    Poof,
    /// Subanim_2TradeBallDrop (0x48): 6-frame drop-and-tilt.
    Drop,
    /// Subanim_2TradeBallShake (0x49): 4-frame wobble.
    Shake,
    /// Subanim_2TradeBallAppear (0x4A): single tilted-ball frame.
    Appear,
}

impl TradeBallSubAnim {
    /// Number of frame blocks in the sub-animation.
    pub fn num_frames(self) -> u8 {
        match self {
            Self::Poof => 3,
            Self::Drop => 6,
            Self::Shake => 4,
            Self::Appear => 1,
        }
    }
}

/// Frame-stepped trade cutscene state. The frontend calls [`TradeAnim::tick`]
/// once per frame, drains [`TradeAnim::pending_sfx`], and renders from the
/// accessor methods. No input: the original's interstitial texts use
/// `BIT_NO_TEXT_DELAY` and auto-advance.
#[derive(Debug, Clone)]
pub struct TradeAnim {
    /// Species the player gives (travels left → right).
    pub give: Species,
    /// Species the player receives (travels right → left).
    pub receive: Species,
    /// Player name, for the "_TradeForText_" line.
    pub player_name: String,
    /// Chinese text when true, English otherwise.
    pub is_zh: bool,
    /// Partner trainer name in the text lines. NPC trades use the literal
    /// `<TRAINER>` (`NPC_TRADE_OT_NAME`); link trades use the remote
    /// trainer's real name (`wLinkEnemyTrainerName` in text_2.asm).
    partner_name: String,
    phase: TradeAnimPhase,
    frame: u16,
    /// SFX queued since the last tick; drained by the frontend.
    pub pending_sfx: Vec<TradeSfx>,
}

impl TradeAnim {
    pub fn new(give: Species, receive: Species, player_name: String, is_zh: bool) -> Self {
        Self {
            give,
            receive,
            player_name,
            is_zh,
            partner_name: NPC_TRADE_OT_NAME.to_string(),
            phase: TradeAnimPhase::SlideInGiveMon,
            frame: 0,
            pending_sfx: Vec::new(),
        }
    }

    /// Set the partner trainer name for the text lines — link trades print
    /// the remote trainer's name (`wLinkEnemyTrainerName`) instead of the NPC
    /// `<TRAINER>` placeholder.
    pub fn with_partner_name(mut self, name: String) -> Self {
        self.partner_name = name;
        self
    }

    pub fn phase(&self) -> TradeAnimPhase {
        self.phase
    }

    pub fn is_done(&self) -> bool {
        self.phase == TradeAnimPhase::Done
    }

    /// Advance one frame. Returns `true` on the frame the animation completes.
    pub fn tick(&mut self) -> bool {
        if self.is_done() {
            return true;
        }
        // Mid-phase SFX beats.
        match self.phase {
            // `Trade_ShowPlayerMon`: PlayCry right after the drop anim.
            TradeAnimPhase::GiveMonBallDrop
                if self.frame == TradeAnimPhase::GiveMonBallDrop.duration() - 1 =>
            {
                self.pending_sfx.push(TradeSfx::GiveMonCry);
            }
            // `Trade_ShowEnemyMon`: PlayCry right after the poof anim.
            TradeAnimPhase::ReceiveMonPoof
                if self.frame == TradeAnimPhase::ReceiveMonPoof.duration() - 1 =>
            {
                self.pending_sfx.push(TradeSfx::ReceiveMonCry);
            }
            TradeAnimPhase::BallEnterCable if self.frame == 0 => {
                self.pending_sfx.push(TradeSfx::CableConnect);
            }
            // SFX_TINK after each 4px step except the one that reaches the
            // edge of the screen (31 tinks per run, like the original).
            TradeAnimPhase::SlideOut | TradeAnimPhase::SlideBack
                if self.frame % TRADE_BALL_STEP_FRAMES == 0 && self.frame > 0 =>
            {
                self.pending_sfx.push(TradeSfx::BallTravel);
            }
            _ => {}
        }
        self.frame += 1;
        if self.frame >= self.phase.duration() {
            self.frame = 0;
            self.phase = self.phase.next();
        }
        self.is_done()
    }

    /// Frames (60 Hz) the current phase lasts.
    pub fn phase_duration(&self) -> u16 {
        self.phase.duration()
    }

    /// Progress (0.0–1.0) within the current phase.
    pub fn phase_progress(&self) -> f32 {
        let d = self.phase.duration();
        if d == 0 {
            1.0
        } else {
            (self.frame as f32 / d as f32).min(1.0)
        }
    }

    /// Species whose front pic is currently on screen, if any.
    /// The given mon stays up through the poof cloud (the drop anim is what
    /// clears the pic in the original); the received mon is revealed under
    /// its poof and stays until the text box finishes sliding off.
    pub fn visible_mon(&self) -> Option<Species> {
        use TradeAnimPhase::*;
        match self.phase {
            SlideInGiveMon | ShowGiveMon | GiveMonPoof => Some(self.give),
            ReceiveMonPoof | ShowReceiveMon | TextTakeCare => Some(self.receive),
            // Trade_SlideTextBoxOffScreen clears the tile map once the box is
            // fully off-screen (last 10 frames of the phase).
            SlideTextBoxOff if self.frame < 127 => Some(self.receive),
            _ => None,
        }
    }

    /// Horizontal pixel offset of the mon pic panel during the
    /// `Trade_ShowPlayerMon` window slide-in (rWX/hSCX $7e → 0, −2 per
    /// frame); 0 at all other times. Add to the pic's resting x.
    pub fn mon_panel_offset_x(&self) -> i32 {
        if self.phase == TradeAnimPhase::SlideInGiveMon {
            126 - 2 * self.frame as i32
        } else {
            0
        }
    }

    /// The ball sub-animation frame currently on screen, if any
    /// (`Trade_ShowAnimation` beats): `(which, frame_block_index)`.
    pub fn ball_sub_anim(&self) -> Option<(TradeBallSubAnim, u8)> {
        use TradeAnimPhase::*;
        match self.phase {
            GiveMonPoof | ReceiveMonPoof => Some((TradeBallSubAnim::Poof, (self.frame / 6) as u8)),
            GiveMonBallDrop => Some((TradeBallSubAnim::Drop, (self.frame / 6) as u8)),
            // The shake plays after the cable-open scroll (20 frames in).
            BallEnterCable if (20..36).contains(&self.frame) => {
                Some((TradeBallSubAnim::Shake, ((self.frame - 20) / 4) as u8))
            }
            ReceiveBallTilt => Some((TradeBallSubAnim::Appear, 0)),
            _ => None,
        }
    }

    /// Horizontal pixel offset of the text box during
    /// `Trade_SlideTextBoxOffScreen` (50-frame hold, then WX += 2 per frame
    /// until $a1 — 77 slide frames, offset 2..=154); 0 at all other times.
    pub fn text_box_offset_x(&self) -> i32 {
        if self.phase == TradeAnimPhase::SlideTextBoxOff && self.frame >= 50 {
            (2 * (self.frame as i32 - 49)).min(154)
        } else {
            0
        }
    }

    /// Cable (and Game Boy pair) is on screen from the cable-connect phase
    /// until the received mon is revealed.
    pub fn cable_visible(&self) -> bool {
        use TradeAnimPhase::*;
        matches!(
            self.phase,
            BallEnterCable | SlideOut | TextWentTo | TextForSends | TextFarewell | SlideBack
                | ReceiveBallTilt
        )
    }

    /// Ball position in screen pixels while it travels the cable.
    /// Includes the original's per-step cable "bulge" (2px vertical toggle).
    pub fn ball_pos(&self) -> Option<(i32, i32)> {
        let bulge = if (self.frame / TRADE_BALL_STEP_FRAMES) % 2 == 0 {
            0
        } else {
            2
        };
        let y = TRADE_CABLE_Y + bulge;
        match self.phase {
            // Parked at the cable mouth after the shake sub-animation (which
            // carries its own on-screen position).
            TradeAnimPhase::BallEnterCable if self.frame >= 36 => Some((TRADE_BALL_MIN_X, y)),
            TradeAnimPhase::SlideOut => {
                let steps = (self.frame / TRADE_BALL_STEP_FRAMES) as i32;
                Some(((TRADE_BALL_MIN_X + 4 * steps).min(TRADE_BALL_MAX_X), y))
            }
            TradeAnimPhase::SlideBack => {
                let steps = (self.frame / TRADE_BALL_STEP_FRAMES) as i32;
                Some(((TRADE_BALL_MAX_X - 4 * steps).max(TRADE_BALL_MIN_X), y))
            }
            _ => None,
        }
    }

    /// The current two text-box lines, if a text phase is active.
    pub fn text_lines(&self) -> Option<(String, String)> {
        let give = pokered_data::lang_data::species_name(self.give, self.is_zh);
        let receive = pokered_data::lang_data::species_name(self.receive, self.is_zh);
        let player = &self.player_name;
        let partner = &self.partner_name;
        let lines = match self.phase {
            // _TradeWentToText: "{GIVE} went / to <TRAINER>."
            TradeAnimPhase::TextWentTo if !self.is_zh => {
                (format!("{} went", give), format!("to {}.", partner))
            }
            TradeAnimPhase::TextWentTo => {
                (format!("{}传给了", give), format!("{}。", partner))
            }
            // _TradeForText + _TradeSendsText:
            // "For {PLAYER}'s {GIVE}," / "<TRAINER> sends {RECEIVE}."
            TradeAnimPhase::TextForSends if !self.is_zh => (
                format!("For {}'s {},", player, give),
                format!("{} sends {}.", partner, receive),
            ),
            TradeAnimPhase::TextForSends => (
                format!("用{}的{}", player, give),
                format!("换来{}的{}。", partner, receive),
            ),
            // _TradeWavesFarewellText + _TradeTransferredText.
            TradeAnimPhase::TextFarewell if !self.is_zh => (
                format!("{} waves farewell", partner),
                format!("as {} is transferred.", receive),
            ),
            TradeAnimPhase::TextFarewell => (
                format!("{}挥手告别，", partner),
                format!("{}被传送走了。", receive),
            ),
            // _TradeTakeCareText: "Take good care of / {RECEIVE}."
            TradeAnimPhase::TextTakeCare if !self.is_zh => {
                ("Take good care of".to_string(), format!("{}.", receive))
            }
            TradeAnimPhase::TextTakeCare => ("好好照顾".to_string(), format!("{}。", receive)),
            // The TakeCare text stays up while it slides off-screen; the
            // original clears the tile map for the phase's last 10 frames.
            TradeAnimPhase::SlideTextBoxOff if self.frame >= 127 => return None,
            TradeAnimPhase::SlideTextBoxOff if !self.is_zh => {
                ("Take good care of".to_string(), format!("{}.", receive))
            }
            TradeAnimPhase::SlideTextBoxOff => ("好好照顾".to_string(), format!("{}。", receive)),
            _ => return None,
        };
        Some(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::trades::{find_npc_trade, NPC_TRADES};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn received_mon_uses_table_data_and_given_level() {
        let trade = find_npc_trade(Species::Abra, Species::MrMime).unwrap();
        let mon = assemble_npc_trade_mon(
            trade.receive,
            17, // level of the given Abra
            trade.nickname,
            [0x12, 0x34],
            40001,
            12345,
        )
        .expect("MrMime has base stats");
        assert_eq!(mon.species, Species::MrMime);
        assert_eq!(mon.level, 17, "received level = given mon's level");
        assert_eq!(mon.dv_bytes, [0x12, 0x34]);
        let mut buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
        assert_eq!(crate::battle::state::decode_name(&mon.nickname, &mut buf), "MARCEL");
        assert_eq!(crate::battle::state::decode_name(&mon.ot_name, &mut buf), "TRAINER");
        assert_eq!(mon.ot_id, 40001);
        assert!(mon.is_traded, "OT ID mismatch → traded (1.5x EXP)");
    }

    #[test]
    fn received_mon_traded_flag_follows_obedience_rule() {
        // ot_id == 0 → "unknown" → own mon (legacy-save rule in obedience.rs).
        let mon =
            assemble_npc_trade_mon(Species::Jynx, 20, "LOLA", [0, 0], 0, 12345).unwrap();
        assert!(!mon.is_traded);
        // ot_id == player_id → own mon (freak matching roll, faithful).
        let mon =
            assemble_npc_trade_mon(Species::Jynx, 20, "LOLA", [0, 0], 12345, 12345).unwrap();
        assert!(!mon.is_traded);
    }

    #[test]
    fn random_rolls_cover_full_byte_range() {
        let mut rng = StdRng::seed_from_u64(0x9A78);
        let (dvs, ot_id) = roll_npc_trade_randoms(&mut rng);
        // Just exercise the API deterministically; values come from the RNG.
        let (dvs2, ot_id2) = roll_npc_trade_randoms(&mut rng);
        assert_ne!((dvs, ot_id), (dvs2, ot_id2), "consecutive rolls differ");
    }

    #[test]
    fn every_table_nickname_survives_construction() {
        for trade in NPC_TRADES.iter() {
            let mon =
                assemble_npc_trade_mon(trade.receive, 5, trade.nickname, [0xAB, 0xCD], 1, 2)
                    .unwrap();
            let mut buf = [0u8; crate::battle::state::NAME_TEXT_BUF];
            assert_eq!(
                crate::battle::state::decode_name(&mon.nickname, &mut buf),
                trade.nickname
            );
            assert_eq!(crate::battle::state::decode_name(&mon.ot_name, &mut buf), "TRAINER");
        }
    }

    #[test]
    fn anim_runs_full_sequence_to_done() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        let mut phases = vec![anim.phase()];
        let mut ticks = 0;
        while !anim.tick() {
            ticks += 1;
            assert!(ticks < 2000, "animation must terminate");
            if anim.phase() != *phases.last().unwrap() {
                phases.push(anim.phase());
            }
        }
        if anim.phase() != *phases.last().unwrap() {
            phases.push(anim.phase());
        }
        use TradeAnimPhase::*;
        assert_eq!(
            phases,
            vec![
                SlideInGiveMon,
                ShowGiveMon,
                GiveMonPoof,
                GiveMonBallDrop,
                BallEnterCable,
                SlideOut,
                TextWentTo,
                TextForSends,
                TextFarewell,
                SlideBack,
                ReceiveBallTilt,
                ReceiveMonPoof,
                ShowReceiveMon,
                TextTakeCare,
                SlideTextBoxOff,
                Done,
            ]
        );
    }

    /// `Trade_ShowPlayerMon` slide loop: rWX/hSCX start at $7e and decrease
    /// 2px/frame for 63 frames before the mon rests (Trade_Delay80).
    #[test]
    fn give_mon_panel_slides_in_63_frames() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        assert_eq!(anim.phase(), TradeAnimPhase::SlideInGiveMon);
        assert_eq!(anim.mon_panel_offset_x(), 126, "slide starts at rWX $7e");
        let mut last = 126;
        let mut frames = 0;
        while anim.phase() == TradeAnimPhase::SlideInGiveMon {
            let off = anim.mon_panel_offset_x();
            assert_eq!(off % 2, 0, "2px steps");
            assert!(off < last || frames == 0, "strictly decreasing");
            last = off;
            frames += 1;
            anim.tick();
        }
        assert_eq!(frames, 63, "63 iterations like the asm loop");
        assert_eq!(last, 2, "last written rWX is 2");
        assert_eq!(anim.phase(), TradeAnimPhase::ShowGiveMon);
        assert_eq!(anim.mon_panel_offset_x(), 0, "at rest after slide-in");
        assert_eq!(anim.visible_mon(), Some(Species::Abra));
    }

    /// The give mon's pic survives the poof cloud and is cleared by the drop
    /// anim; the cry plays as the drop ends (`Trade_ShowPlayerMon`).
    #[test]
    fn poof_then_drop_then_cry() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::GiveMonPoof {
            anim.tick();
        }
        // Subanim_2TradeBallPoof: 3 frame blocks × delay 6.
        for f in 0..18 {
            assert_eq!(
                anim.ball_sub_anim(),
                Some((TradeBallSubAnim::Poof, f / 6)),
                "poof frame at tick {f}"
            );
            assert_eq!(anim.visible_mon(), Some(Species::Abra), "pic under poof");
            anim.tick();
        }
        assert_eq!(anim.phase(), TradeAnimPhase::GiveMonBallDrop);
        // Subanim_2TradeBallDrop: 6 frame blocks × delay 6; pic cleared.
        for f in 0..36 {
            assert_eq!(anim.ball_sub_anim(), Some((TradeBallSubAnim::Drop, f / 6)));
            assert_eq!(anim.visible_mon(), None, "drop anim clears the pic");
            anim.tick();
            let cries: Vec<_> = anim.pending_sfx.drain(..).collect();
            if f < 35 {
                assert!(!cries.contains(&TradeSfx::GiveMonCry));
            } else {
                assert!(cries.contains(&TradeSfx::GiveMonCry), "cry as drop ends");
            }
        }
        assert_eq!(anim.phase(), TradeAnimPhase::BallEnterCable);
    }

    /// Ball-entering phase: cable-connect SFX, then the shake sub-anim
    /// (Subanim_2TradeBallShake: 4 frames × delay 4) after the 20-frame
    /// cable scroll, then a 10-frame delay with the ball parked.
    #[test]
    fn ball_enter_cable_beats() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::BallEnterCable {
            anim.tick();
        }
        assert_eq!(anim.phase_duration(), 46);
        for f in 0..46 {
            let expect = if (20..36).contains(&f) {
                Some((TradeBallSubAnim::Shake, (f - 20) / 4))
            } else {
                None
            };
            assert_eq!(anim.ball_sub_anim(), expect, "shake frame at tick {f}");
            assert_eq!(anim.ball_pos().is_some(), f >= 36, "parked after delay");
            anim.tick();
        }
        assert_eq!(anim.phase(), TradeAnimPhase::SlideOut);
    }

    /// Cable run: 32 steps of 4px with Delay3 (96 frames), SFX_TINK per
    /// step except the last (`Trade_AnimateBallEnteringLinkCable`).
    #[test]
    fn cable_run_timing_matches_original() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::SlideOut {
            anim.tick();
        }
        let mut tinks = 0;
        for f in 0..96 {
            let (x, _y) = anim.ball_pos().unwrap();
            assert_eq!(x, 24 + 4 * (f as i32 / 3), "OAM x $20 + 4px per Delay3");
            anim.tick();
            tinks += anim.pending_sfx.drain(..).filter(|s| *s == TradeSfx::BallTravel).count();
        }
        assert_eq!(tinks, 31, "tink after each step except the last");
        assert_eq!(anim.phase(), TradeAnimPhase::TextWentTo);
    }

    /// Receive side (`Trade_ShowEnemyMon`): tilt anim → poof revealing the
    /// mon → cry → Trade_Delay100.
    #[test]
    fn receive_side_tilt_poof_cry() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::ReceiveBallTilt {
            anim.tick();
        }
        // Subanim_2TradeBallAppear: single tilted frame × delay 6.
        for _ in 0..6 {
            assert_eq!(anim.ball_sub_anim(), Some((TradeBallSubAnim::Appear, 0)));
            assert_eq!(anim.visible_mon(), None);
            anim.tick();
        }
        assert_eq!(anim.phase(), TradeAnimPhase::ReceiveMonPoof);
        for f in 0..18 {
            assert_eq!(anim.ball_sub_anim(), Some((TradeBallSubAnim::Poof, f / 6)));
            assert_eq!(anim.visible_mon(), Some(Species::MrMime), "revealed under poof");
            anim.tick();
            let cries: Vec<_> = anim.pending_sfx.drain(..).collect();
            if f < 17 {
                assert!(!cries.contains(&TradeSfx::ReceiveMonCry));
            } else {
                assert!(cries.contains(&TradeSfx::ReceiveMonCry), "cry as poof ends");
            }
        }
        assert_eq!(anim.phase(), TradeAnimPhase::ShowReceiveMon);
        assert_eq!(anim.phase_duration(), 100, "Trade_Delay100");
    }

    /// `Trade_SlideTextBoxOffScreen`: 50-frame hold, then the text box
    /// slides right 2px/frame for 77 frames, then a 10-frame clear.
    #[test]
    fn text_box_slides_off_screen() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::SlideTextBoxOff {
            anim.tick();
        }
        assert_eq!(anim.phase_duration(), 137);
        for f in 0..137 {
            let off = anim.text_box_offset_x();
            if f < 50 {
                assert_eq!(off, 0, "50-frame hold");
                assert!(anim.text_lines().is_some());
            } else if f < 127 {
                assert_eq!(off, 2 * (f as i32 - 49), "WX += 2 per frame");
                assert!(anim.text_lines().is_some(), "text rides the box");
            } else {
                assert_eq!(off, 154, "fully off (WX $a1)");
                assert!(anim.text_lines().is_none(), "tile map cleared");
            }
            anim.tick();
        }
        assert!(anim.is_done());
    }

    #[test]
    fn anim_emits_expected_sfx() {
        let mut anim = TradeAnim::new(Species::Spearow, Species::Farfetchd, "RED".to_string(), false);
        let mut seen = Vec::new();
        while !anim.is_done() {
            anim.tick();
            seen.extend(anim.pending_sfx.drain(..));
        }
        assert!(seen.contains(&TradeSfx::CableConnect));
        assert!(seen.contains(&TradeSfx::GiveMonCry));
        assert!(seen.contains(&TradeSfx::ReceiveMonCry));
        assert!(seen.iter().filter(|s| **s == TradeSfx::BallTravel).count() >= 10);
    }

    #[test]
    fn ball_crosses_screen_during_slides() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        // Fast-forward to SlideOut.
        while anim.phase() != TradeAnimPhase::SlideOut {
            anim.tick();
        }
        let first = anim.ball_pos().unwrap().0;
        while anim.phase() == TradeAnimPhase::SlideOut && anim.phase_progress() < 0.95 {
            anim.tick();
        }
        let last = anim.ball_pos().unwrap().0;
        assert!(first <= TRADE_BALL_MIN_X + 8);
        assert!(last >= TRADE_BALL_MAX_X - 16, "ball crossed L→R");
        // SlideBack travels R→L.
        while anim.phase() != TradeAnimPhase::SlideBack {
            anim.tick();
        }
        let first = anim.ball_pos().unwrap().0;
        while anim.phase() == TradeAnimPhase::SlideBack && anim.phase_progress() < 0.95 {
            anim.tick();
        }
        let last = anim.ball_pos().unwrap().0;
        assert!(first >= TRADE_BALL_MAX_X - 8);
        assert!(last <= TRADE_BALL_MIN_X + 16, "ball crossed R→L");
    }

    #[test]
    fn text_lines_match_original_strings() {
        let mut anim = TradeAnim::new(Species::Abra, Species::MrMime, "RED".to_string(), false);
        while anim.phase() != TradeAnimPhase::TextWentTo {
            anim.tick();
        }
        assert_eq!(
            anim.text_lines(),
            Some(("ABRA went".to_string(), "to <TRAINER>.".to_string()))
        );
        while anim.phase() != TradeAnimPhase::TextForSends {
            anim.tick();
        }
        assert_eq!(
            anim.text_lines(),
            Some((
                "For RED's ABRA,".to_string(),
                "<TRAINER> sends MR.MIME.".to_string()
            ))
        );
        while anim.phase() != TradeAnimPhase::TextTakeCare {
            anim.tick();
        }
        assert_eq!(
            anim.text_lines(),
            Some(("Take good care of".to_string(), "MR.MIME.".to_string()))
        );
    }

    /// Link trades print the remote trainer's name in the text lines
    /// (`wLinkEnemyTrainerName`, text_2.asm:_TradeWentToText) instead of the
    /// NPC `<TRAINER>` placeholder.
    #[test]
    fn link_anim_uses_partner_trainer_name() {
        let mut anim = TradeAnim::new(Species::Pikachu, Species::Charmander, "RED".to_string(), false)
            .with_partner_name("GREEN".to_string());
        while anim.phase() != TradeAnimPhase::TextWentTo {
            anim.tick();
        }
        assert_eq!(
            anim.text_lines(),
            Some(("PIKACHU went".to_string(), "to GREEN.".to_string()))
        );
        while anim.phase() != TradeAnimPhase::TextForSends {
            anim.tick();
        }
        assert_eq!(
            anim.text_lines(),
            Some((
                "For RED's PIKACHU,".to_string(),
                "GREEN sends CHARMANDER.".to_string()
            ))
        );
        while anim.phase() != TradeAnimPhase::TextFarewell {
            anim.tick();
        }
        assert_eq!(
            anim.text_lines(),
            Some((
                "GREEN waves farewell".to_string(),
                "as CHARMANDER is transferred.".to_string()
            ))
        );
    }
}
