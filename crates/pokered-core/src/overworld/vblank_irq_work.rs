//! A single bounded VBlank continuation, from accepted video entry to RETI.
//!
//! Audio1/3 is supplied by the existing native sequencer owner at its actual
//! call boundary. Its decoded instruction work advances this same device clock;
//! no captured duration or second channel player lives here. Register-phase
//! APU writes and unsupported fade/sound-bank children remain outside this port.
use super::{
    display_work_time::{DisplayWorkError, DisplayWorkTime},
    fade_audio_work::{FadeAudioState, FadeAudioWork},
    field_restore::{PatternAssets, PatternMemory},
    track_play_time_work::TrackTimeState,
    vblank_after_music_work::VBlankAfterMusicWork,
    vblank_audio_dispatch_work::{SourceAudioEngine, VBlankAudioDispatchWork},
    vblank_pre_audio_work::VBlankPreAudioWork,
    vblank_video_work::VBlankVideoWork,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqProgress {
    Instruction,
    /// The caller must tick its existing audio owner once, then pass only the
    /// successfully derived work back. Re-observing this boundary does not tick.
    NeedsAudio,
    Returned,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrqError {
    Display(DisplayWorkError),
    UnsupportedFadeChild,
    UnsupportedAudioBank(u8),
    InvalidAudioReceipt,
}
impl From<DisplayWorkError> for IrqError {
    fn from(error: DisplayWorkError) -> Self {
        Self::Display(error)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Phase {
    Video,
    PreAudio,
    Fade,
    Dispatch,
    AwaitAudio,
    AudioWork,
    AfterAudioDispatch,
    AfterMusic,
    Done,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VBlankIrqWork {
    phase: Phase,
    video: VBlankVideoWork,
    pre_audio: Option<VBlankPreAudioWork>,
    fade: FadeAudioWork,
    dispatch: Option<VBlankAudioDispatchWork>,
    after_music: Option<VBlankAfterMusicWork>,
    timer_input: TrackTimeState,
    remaining_audio_work: u32,
}
impl VBlankIrqWork {
    pub fn new(video: VBlankVideoWork, fade: FadeAudioState, timer: TrackTimeState) -> Self {
        Self {
            phase: Phase::Video,
            video,
            pre_audio: None,
            fade: FadeAudioWork::new(fade),
            dispatch: None,
            after_music: None,
            timer_input: timer,
            remaining_audio_work: 0,
        }
    }
    pub fn finished(&self) -> bool {
        self.phase == Phase::Done
    }
    pub fn video(&self) -> &VBlankVideoWork {
        &self.video
    }
    pub fn video_mut(&mut self) -> &mut VBlankVideoWork {
        &mut self.video
    }
    pub fn timer_state(&self) -> TrackTimeState {
        self.after_music
            .as_ref()
            .map_or(self.timer_input, |w| w.timer_state())
    }
    pub fn fade_state(&self) -> FadeAudioState {
        self.fade.state()
    }
    pub fn provide_audio_work(&mut self, cycles: u32) -> Result<(), IrqError> {
        if self.phase != Phase::AwaitAudio || cycles == 0 || cycles & 3 != 0 {
            return Err(IrqError::InvalidAudioReceipt);
        }
        self.remaining_audio_work = cycles;
        self.phase = Phase::AudioWork;
        Ok(())
    }
    /// A retained external audio instruction cursor has already consumed this
    /// owner's time and committed its one native sequencer update after RET.
    /// Unlike aggregate diagnostic work, these instructions must not run twice.
    pub fn audio_returned(&mut self, cycles: u32) -> Result<(), IrqError> {
        if self.phase != Phase::AwaitAudio || cycles == 0 || cycles & 3 != 0 {
            return Err(IrqError::InvalidAudioReceipt);
        }
        self.remaining_audio_work = 0;
        self.phase = Phase::AudioWork;
        Ok(())
    }
    /// Service a grant without releasing a video child's validated bus after
    /// each instruction. Audio remains a single explicit native-owner call.
    pub fn run_until_boundary<A: PatternAssets>(
        &mut self,
        time: &mut DisplayWorkTime,
        memory: &mut PatternMemory,
        assets: &A,
    ) -> Result<IrqProgress, IrqError> {
        loop {
            if self.phase == Phase::Video {
                self.video.run_until_body_ready(time, memory, assets)?;
            }
            match self.step(time, memory, assets)? {
                IrqProgress::Instruction => (),
                boundary => return Ok(boundary),
            }
        }
    }
    pub fn step<A: PatternAssets>(
        &mut self,
        time: &mut DisplayWorkTime,
        memory: &mut PatternMemory,
        assets: &A,
    ) -> Result<IrqProgress, IrqError> {
        loop {
            match self.phase {
                Phase::Video => {
                    if self.video.step(time, memory, assets)? {
                        return Ok(IrqProgress::Instruction);
                    }
                    self.pre_audio = Some(VBlankPreAudioWork::new(
                        self.video
                            .random_entry_carry()
                            .expect("returned PrepareOAM carry"),
                    ));
                    self.phase = Phase::PreAudio;
                }
                Phase::PreAudio => {
                    if self.pre_audio.as_mut().unwrap().step(time)? {
                        return Ok(IrqProgress::Instruction);
                    }
                    self.phase = Phase::Fade;
                }
                Phase::Fade => {
                    if self.fade.step(time)? {
                        return Ok(IrqProgress::Instruction);
                    }
                    if !self.fade.finished() {
                        return Err(IrqError::UnsupportedFadeChild);
                    }
                    self.dispatch =
                        Some(VBlankAudioDispatchWork::new(self.fade.state().audio_bank));
                    self.phase = Phase::Dispatch;
                }
                Phase::Dispatch => {
                    let dispatch = self.dispatch.as_mut().unwrap();
                    if dispatch.step(time)? {
                        return Ok(IrqProgress::Instruction);
                    }
                    if !matches!(
                        (dispatch.bank(), dispatch.engine()),
                        (2, Some(SourceAudioEngine::One)) | (31, Some(SourceAudioEngine::Three))
                    ) || !dispatch.music_ready()
                    {
                        return Err(IrqError::UnsupportedAudioBank(dispatch.bank()));
                    }
                    self.phase = Phase::AwaitAudio;
                }
                Phase::AwaitAudio => return Ok(IrqProgress::NeedsAudio),
                Phase::AudioWork => {
                    if self.remaining_audio_work != 0 {
                        time.consume_vblank_body_work(&mut self.remaining_audio_work)?;
                        return Ok(IrqProgress::Instruction);
                    }
                    assert!(self.dispatch.as_mut().unwrap().music_returned());
                    self.phase = Phase::AfterAudioDispatch;
                }
                Phase::AfterAudioDispatch => {
                    // Audio1 returns through JR .afterMusic; Audio3 falls through.
                    // Preserve that instruction and any budget suspension before
                    // starting the shared timer/input/RETI suffix.
                    let dispatch = self.dispatch.as_mut().unwrap();
                    if dispatch.step(time)? {
                        return Ok(IrqProgress::Instruction);
                    }
                    assert!(dispatch.after_music_ready());
                    self.after_music = Some(VBlankAfterMusicWork::new(
                        self.timer_input,
                        self.video.saved_bank().expect("VBlank saved bank"),
                    ));
                    self.phase = Phase::AfterMusic;
                }
                Phase::AfterMusic => {
                    if self.after_music.as_mut().unwrap().step(time)? {
                        return Ok(IrqProgress::Instruction);
                    }
                    self.phase = Phase::Done;
                }
                Phase::Done => return Ok(IrqProgress::Returned),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        alloc_prelude::*,
        overworld::{display_delay_work::DelayFrameBus, field_restore::PatternSource},
        save::game_data::PlayTime,
    };
    struct NoAssets;
    impl PatternAssets for NoAssets {
        fn read_pattern_byte(&self, _: PatternSource, _: u16) -> Option<u8> {
            None
        }
    }
    fn build(bank: u8) -> (DisplayWorkTime, PatternMemory, VBlankIrqWork) {
        let (mut time, memory, video, _, _) = super::super::vblank_video_work::tests::build();
        time.set_vblank_dispatch_enabled(true);
        let fade = FadeAudioState {
            control: 0,
            counter: 0,
            reload: 0,
            flags2: 0,
            nr50: 0x77,
            audio_bank: bank,
            saved_bank: bank,
            new_sound: 0,
        };
        let timer = TrackTimeState {
            ignore_counter: 0,
            flags5: 0,
            flags6: 3,
            joy_pressed: 0,
            joy_held: 0,
            time: PlayTime::default(),
        };
        (time, memory, VBlankIrqWork::new(video, fade, timer))
    }
    #[test]
    fn leased_irq_runner_matches_instruction_owner_at_audio_budget_error_and_reti_boundaries() {
        for bank in [2, 8, 31] {
            for budget in [0, 4, 120, 8192, 70224] {
                let (time, memory, work) = build(bank);
                let mut state = serde_json::to_value(&time).unwrap();
                state["deadline"] = (time.elapsed() + budget).into();
                let mut time: DisplayWorkTime = serde_json::from_value(state).unwrap();
                let (mut reference, mut expected_memory, mut expected_work) =
                    (time.clone(), memory.clone(), work.clone());
                let (mut memory, mut work) = (memory, work);
                loop {
                    let actual = work.run_until_boundary(&mut time, &mut memory, &NoAssets);
                    let expected = loop {
                        match expected_work.step(&mut reference, &mut expected_memory, &NoAssets) {
                            Ok(IrqProgress::Instruction) => (),
                            result => break result,
                        }
                    };
                    assert_eq!(actual, expected);
                    assert_eq!(
                        serde_json::to_value((&time, &memory, &work)).unwrap(),
                        serde_json::to_value((&reference, &expected_memory, &expected_work))
                            .unwrap()
                    );
                    match actual {
                        Ok(IrqProgress::NeedsAudio) => {
                            work.provide_audio_work(70228).unwrap();
                            expected_work.provide_audio_work(70228).unwrap();
                        }
                        Err(IrqError::Display(DisplayWorkError::Budget)) => {
                            (time, memory, work) = serde_json::from_value(
                                serde_json::to_value((&time, &memory, &work)).unwrap(),
                            )
                            .unwrap();
                            (reference, expected_memory, expected_work) = serde_json::from_value(
                                serde_json::to_value((
                                    &reference,
                                    &expected_memory,
                                    &expected_work,
                                ))
                                .unwrap(),
                            )
                            .unwrap();
                            time.begin_native_update();
                            reference.begin_native_update();
                        }
                        Ok(IrqProgress::Returned) => {
                            assert_eq!(time.interrupt_returns(), 1);
                            assert!(!time.active_vblank());
                            break;
                        }
                        Err(IrqError::UnsupportedAudioBank(value)) => {
                            assert_eq!(value, bank);
                            break;
                        }
                        other => panic!("unexpected runner boundary: {other:?}"),
                    }
                }
            }
        }
    }
    #[test]
    fn only_complete_video_random_audio_timer_input_and_reti_can_return_foreground() {
        let mut ends = Vec::new();
        for bank in [2, 31] {
            for cycles in [4, 70224 + 4] {
                // Synthetic bridge durations, not original audio captures.
                let (mut time, mut memory, mut work) = build(bank);
                let mut supplied = 0;
                let mut snapshot_checked = false;
                loop {
                    match work.step(&mut time, &mut memory, &NoAssets) {
                        Ok(IrqProgress::NeedsAudio) => {
                            assert!(time.active_vblank());
                            assert_eq!(time.interrupt_returns(), 0);
                            assert_eq!(
                                work.step(&mut time, &mut memory, &NoAssets),
                                Ok(IrqProgress::NeedsAudio)
                            );
                            assert_eq!(
                                work.provide_audio_work(0),
                                Err(IrqError::InvalidAudioReceipt)
                            );
                            work.provide_audio_work(cycles).unwrap();
                            assert_eq!(
                                work.provide_audio_work(cycles),
                                Err(IrqError::InvalidAudioReceipt)
                            );
                            supplied += 1;
                        }
                        Ok(IrqProgress::Returned) => break,
                        Ok(IrqProgress::Instruction) => (),
                        Err(IrqError::Display(DisplayWorkError::Budget)) => {
                            assert!(time.active_vblank());
                            assert_eq!(time.interrupt_returns(), 0);
                            let json = serde_json::to_vec(&(&time, &memory, &work)).unwrap();
                            (time, memory, work) = serde_json::from_slice(&json).unwrap();
                            snapshot_checked = true;
                            time.begin_native_update();
                        }
                        Err(error) => panic!("unexpected IRQ error {error:?}"),
                    }
                }
                assert_eq!(supplied, 1);
                assert!(work.finished());
                assert!(!time.active_vblank());
                assert_eq!(time.interrupt_returns(), 1);
                assert_eq!(work.timer_state().time.frames, 1);
                assert_eq!(
                    time.source_loaded_rom_bank(),
                    work.video().saved_bank().unwrap()
                );
                let end = time.elapsed();
                assert_eq!(
                    work.step(&mut time, &mut memory, &NoAssets),
                    Ok(IrqProgress::Returned)
                );
                assert_eq!(time.elapsed(), end);
                if cycles > 70224 {
                    assert!(snapshot_checked);
                }
                ends.push(end);
            }
        }
        assert_eq!(ends[1] - ends[0], 70224);
        assert_eq!(ends[3] - ends[2], 70224);
        // Audio1 skips a comparison but executes the final JR: net 12 T fewer.
        assert_eq!(ends[2] - ends[0], 12);
        assert_eq!(ends[3] - ends[1], 12);
    }
    #[test]
    fn unsupported_bank_cannot_borrow_the_audio3_duration_or_return_the_irq() {
        let (mut time, mut memory, mut work) = build(8);
        loop {
            match work.step(&mut time, &mut memory, &NoAssets) {
                Ok(IrqProgress::Instruction) => (),
                Err(IrqError::Display(DisplayWorkError::Budget)) => time.begin_native_update(),
                Err(IrqError::UnsupportedAudioBank(8)) => break,
                other => panic!("unexpected result {other:?}"),
            }
        }
        assert!(time.active_vblank());
        assert_eq!(time.interrupt_returns(), 0);
        assert_eq!(
            work.provide_audio_work(2004),
            Err(IrqError::InvalidAudioReceipt)
        );
    }
}
