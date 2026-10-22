//! Retained source music-command work through note-length entry. Converted native
//! stream offsets are used only for reading the one owner's stream; no ROM code
//! is interpreted at runtime and no native channel or APU is updated here.
pub use super::audio_note_dispatch_work::MusicCommandState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct StreamInput {
    pub channel: u8,
    pub ptr: usize,
    pub return_ptr: usize,
    pub loop_counter: u8,
    pub state: MusicCommandState,
    pub vibrato_reload: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct StreamResult {
    pub state: MusicCommandState,
    pub note: u8,
    pub next_ptr: usize,
    pub return_ptr: usize,
    pub loop_counter: u8,
    pub tempo: Option<u16>,
    pub vibrato_reload: u8,
    pub vibrato_extent: Option<u8>,
    pub vibrato_rate: Option<u8>,
    pub wave_instrument: Option<u8>,
    pub master_volume: Option<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum StreamInputError {
    UnsupportedChannel(u8),
    TruncatedStream(usize),
    UnsupportedCommand(u8),
    ChannelEndUnsupported,
    CommandLimit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamStepError<E> {
    Input(StreamInputError),
    Clock(E),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Fetch {
    Command,
    CallLow,
    CallHigh,
    LoopCount,
    LoopLow,
    LoopHigh,
    SkipLow,
    SkipHigh,
    NoteParameter,
    DutyParameter,
    VibratoDelay,
    VibratoDepth,
    TempoHigh,
    TempoLow,
    Volume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
enum Stage {
    Entry(u8),
    FetchCall(Fetch),
    FetchByte(Fetch, u8),
    CompareReturn(u8),
    ReturnCheck(u8),
    ReturnBody(u8),
    CompareCall(u8),
    CallPush,
    CallBody(u8),
    CompareLoop(u8),
    LoopCount(u8),
    LoopCheck(u8),
    LoopRepeat(u8),
    LoopReset(u8),
    LoopPush,
    LoopBody(u8),
    NoteType(u8),
    SetSpeed(u8),
    ParameterKind(u8),
    WaveVolume(u8),
    StoreVolume(u8),
    PerfectPitch(u8),
    TogglePitch(u8),
    Vibrato(u8),
    VibratoDelay(u8),
    VibratoDepth(u8),
    PitchSlide(u8),
    Duty(u8),
    SetDuty(u8),
    Tempo(u8),
    MusicTempo(u8),
    StoreTempoHigh,
    StoreTempoLow(u8),
    OtherCompare(u8, u8),
    MasterVolume(u8),
    Octave(u8),
    StoreOctave(u8),
    SfxNote(u8),
    MusicSfxGate(u8),
    PitchSweep(u8),
    Note(u8),
    NoiseNote(u8),
    Jump,
    NoteLength,
    Unsupported(StreamInputError),
}
/// CPU temporaries and actual branch progress. Results become available only at
/// note-length entry. The caller must retain exclusive ownership of stream data
/// and feed accepted instructions into its common clock; no native tick occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "source-clock-serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct StreamInstructions {
    channel: u8,
    stage: Stage,
    output: StreamResult,
    command: u8,
    last: u8,
    low: u8,
    high: u8,
    count: u8,
    remaining: u8,
    commands: u8,
    cycles: u32,
}
impl StreamInstructions {
    pub fn new(input: StreamInput) -> Result<Self, StreamInputError> {
        if input.channel >= 4 {
            return Err(StreamInputError::UnsupportedChannel(input.channel));
        }
        Ok(Self {
            channel: input.channel,
            stage: Stage::Entry(0),
            output: StreamResult {
                state: input.state,
                note: 0,
                next_ptr: input.ptr,
                return_ptr: input.return_ptr,
                loop_counter: input.loop_counter,
                tempo: None,
                vibrato_reload: input.vibrato_reload,
                vibrato_extent: None,
                vibrato_rate: None,
                wave_instrument: None,
                master_volume: None,
            },
            command: 0,
            last: 0,
            low: 0,
            high: 0,
            count: 0,
            remaining: 0,
            commands: 0,
            cycles: 0,
        })
    }
    pub fn finished(&self) -> bool {
        self.stage == Stage::NoteLength
    }
    pub fn result(&self) -> Option<StreamResult> {
        self.finished().then_some(self.output)
    }
    pub fn cycles(&self) -> u32 {
        self.cycles
    }
    pub fn try_step<E>(
        &mut self,
        data: &[u8],
        execute: impl FnOnce(u8) -> Result<(), E>,
    ) -> Result<bool, StreamStepError<E>> {
        if let Stage::Unsupported(error) = self.stage {
            return Err(StreamStepError::Input(error));
        }
        if self.finished() {
            return Ok(true);
        }
        let byte = if matches!(self.stage, Stage::FetchByte(_, 10)) {
            Some(
                *data
                    .get(self.output.next_ptr)
                    .ok_or(StreamStepError::Input(StreamInputError::TruncatedStream(
                        self.output.next_ptr,
                    )))?,
            )
        } else {
            None
        };
        let cycles = self.next_cycles().expect("live command instruction");
        execute(cycles).map_err(StreamStepError::Clock)?;
        self.advance(byte);
        self.cycles += u32::from(cycles);
        Ok(self.finished())
    }
    fn next_cycles(&self) -> Option<u8> {
        use Stage::*;
        let c = self.command;
        Some(match self.stage {
            Entry(n) => [12, 8, 8, 12, 8, 8, 12, 8, 16, 16, 24][n as usize],
            FetchCall(_) => 24,
            FetchByte(_, n) => [8, 4, 4, 4, 12, 8, 8, 4, 8, 4, 8, 8, 8, 8, 8, 16][n as usize],
            CompareReturn(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if c == 0xff {
                    12
                } else {
                    16
                }
            }
            ReturnCheck(n) => {
                if n < 4 {
                    [8, 12, 8, 12][n as usize]
                } else if self.output.state.flags1 & 2 != 0 {
                    12
                } else {
                    8
                }
            }
            ReturnBody(n) => [
                16, 8, 4, 4, 4, 12, 8, 16, 12, 8, 4, 4, 12, 8, 8, 8, 8, 8, 16,
            ][n as usize],
            CompareCall(n) => {
                if n == 0 {
                    8
                } else if c == 0xfd {
                    12
                } else {
                    16
                }
            }
            CallPush | LoopPush => 16,
            CallBody(n) => [
                4, 12, 4, 16, 8, 4, 4, 4, 12, 8, 16, 12, 8, 4, 4, 12, 8, 8, 8, 8, 8, 12, 8, 8, 8,
                8, 12, 8, 16, 16,
            ][n as usize],
            CompareLoop(n) => {
                if n == 0 {
                    8
                } else if c == 0xfe {
                    12
                } else {
                    16
                }
            }
            LoopCount(n) => {
                if n < 2 {
                    4
                } else if self.count == 0 {
                    12
                } else {
                    8
                }
            }
            LoopCheck(n) => {
                if n < 5 {
                    [8, 12, 8, 8, 4][n as usize]
                } else if self.remaining > 1 {
                    12
                } else {
                    8
                }
            }
            LoopRepeat(n) => [4, 8][n as usize],
            LoopReset(n) => [8, 8][n as usize],
            LoopBody(n) => [4, 8, 4, 4, 4, 12, 8, 12, 8, 8, 16][n as usize],
            NoteType(n) => {
                if n < 2 {
                    8
                } else if c & 0xf0 == 0xd0 {
                    12
                } else {
                    16
                }
            }
            SetSpeed(n) => {
                if n < 8 {
                    [4, 8, 8, 12, 8, 8, 4, 8][n as usize]
                } else if self.channel == 3 {
                    12
                } else {
                    8
                }
            }
            ParameterKind(n) => {
                if n < 3 {
                    [4, 4, 8][n as usize]
                } else if n == 3 {
                    if self.channel == 2 {
                        12
                    } else {
                        8
                    }
                } else if n == 4 {
                    8
                } else {
                    12
                }
            }
            WaveVolume(n) => [12, 4, 8, 8, 4, 8, 8, 4][n as usize],
            StoreVolume(n) => [8, 12, 8, 8, 16][n as usize],
            PerfectPitch(n) => {
                if n < 2 {
                    [4, 8][n as usize]
                } else if c == 0xe8 {
                    8
                } else {
                    12
                }
            }
            TogglePitch(n) => [8, 12, 8, 8, 8, 8, 16][n as usize],
            Vibrato(n) | PitchSlide(n) | Duty(n) | Tempo(n) => {
                if n == 0 {
                    8
                } else {
                    let command = match self.stage {
                        Vibrato(_) => 0xea,
                        PitchSlide(_) => 0xeb,
                        Duty(_) => 0xec,
                        _ => 0xed,
                    };
                    if c == command {
                        8
                    } else {
                        12
                    }
                }
            }
            VibratoDelay(n) => [8, 12, 8, 8, 12, 8, 8][n as usize],
            VibratoDepth(n) => [
                4, 8, 8, 8, 12, 8, 8, 4, 4, 8, 4, 8, 4, 8, 4, 12, 8, 8, 4, 8, 16,
            ][n as usize],
            SetDuty(n) => [4, 4, 8, 8, 12, 8, 8, 16][n as usize],
            MusicTempo(n) => [4, 8, 8][n as usize],
            StoreTempoHigh => 16,
            StoreTempoLow(n) => [16, 4, 16, 16, 16, 16, 12, 16][n as usize],
            OtherCompare(which, n) => {
                if n == 0 {
                    8
                } else if [0xee, 0xef, 0xfc, 0xf0, 0xf8][which as usize] == c {
                    8
                } else {
                    12
                }
            }
            MasterVolume(n) => [12, 16][n as usize],
            Octave(n) => {
                if n < 2 {
                    8
                } else if c & 0xf0 == 0xe0 {
                    8
                } else {
                    12
                }
            }
            StoreOctave(n) => [12, 8, 8, 4, 8, 8, 16][n as usize],
            SfxNote(n) => {
                if n == 0 {
                    8
                } else if c & 0xf0 == 0x20 {
                    8
                } else {
                    12
                }
            }
            MusicSfxGate(n) => [4, 8, 12][n as usize],
            PitchSweep(n) => [4, 8, 12][n as usize],
            Note(n) => [4, 8, if self.channel == 3 { 8 } else { 12 }][n as usize],
            NoiseNote(n) => {
                if n < 3 {
                    [4, 8, 8][n as usize]
                } else if n == 3 {
                    if c & 0xf0 == 0xb0 {
                        12
                    } else {
                        8
                    }
                } else if c & 0xf0 >= 0xb0 {
                    12
                } else {
                    8
                }
            }
            Jump => 16,
            NoteLength | Unsupported(_) => return None,
        })
    }
    fn fetch_finished(&mut self, kind: Fetch) -> Stage {
        use Fetch::*;
        match kind {
            Command => {
                self.command = self.last;
                self.commands += 1;
                Stage::CompareReturn(0)
            }
            CallLow => {
                self.low = self.last;
                Stage::CallPush
            }
            CallHigh => {
                self.high = self.last;
                Stage::CallBody(0)
            }
            LoopCount => {
                self.count = self.last;
                self.remaining = if self.output.loop_counter == 0 {
                    self.last
                } else {
                    self.output.loop_counter
                };
                Stage::LoopCount(0)
            }
            LoopLow => {
                self.low = self.last;
                Stage::LoopPush
            }
            LoopHigh => {
                self.high = self.last;
                Stage::LoopBody(0)
            }
            SkipLow => Stage::FetchCall(SkipHigh),
            SkipHigh => Stage::Jump,
            NoteParameter => Stage::ParameterKind(0),
            DutyParameter => Stage::SetDuty(0),
            VibratoDelay => Stage::VibratoDelay(0),
            VibratoDepth => Stage::VibratoDepth(0),
            TempoHigh => {
                self.high = self.last;
                Stage::StoreTempoHigh
            }
            TempoLow => Stage::StoreTempoLow(0),
            Volume => Stage::MasterVolume(0),
        }
    }
    fn restart(&self) -> Stage {
        if self.commands >= 100 {
            Stage::Unsupported(StreamInputError::CommandLimit)
        } else {
            Stage::FetchCall(Fetch::Command)
        }
    }
    fn advance(&mut self, byte: Option<u8>) {
        use Stage::*;
        self.stage = match self.stage {
            Entry(n) => {
                if n == 5 {
                    self.output.state.vibrato_delay = self.output.vibrato_reload;
                }
                if n == 8 {
                    self.output.state.flags1 &= !16;
                }
                if n == 9 {
                    self.output.state.flags1 &= !32;
                }
                if n == 10 {
                    self.restart()
                } else {
                    Entry(n + 1)
                }
            }
            FetchCall(f) => FetchByte(f, 0),
            FetchByte(f, n) => {
                if n == 10 {
                    self.last = byte.expect("checked music byte");
                }
                if n == 11 {
                    self.output.next_ptr += 1;
                }
                if n == 15 {
                    self.fetch_finished(f)
                } else {
                    FetchByte(f, n + 1)
                }
            }
            CompareReturn(n) => {
                if n == 2 {
                    if self.command == 0xff {
                        ReturnCheck(0)
                    } else {
                        CompareCall(0)
                    }
                } else {
                    CompareReturn(n + 1)
                }
            }
            ReturnCheck(n) => {
                if n == 4 {
                    if self.output.state.flags1 & 2 == 0 {
                        Unsupported(StreamInputError::ChannelEndUnsupported)
                    } else {
                        ReturnBody(0)
                    }
                } else {
                    ReturnCheck(n + 1)
                }
            }
            ReturnBody(n) => {
                if n == 0 {
                    self.output.state.flags1 &= !2;
                }
                if n == 17 {
                    self.output.next_ptr = self.output.return_ptr;
                }
                if n == 18 {
                    self.restart()
                } else {
                    ReturnBody(n + 1)
                }
            }
            CompareCall(n) => {
                if n == 1 {
                    if self.command == 0xfd {
                        FetchCall(Fetch::CallLow)
                    } else {
                        CompareLoop(0)
                    }
                } else {
                    CompareCall(1)
                }
            }
            CallPush => FetchCall(Fetch::CallHigh),
            CallBody(n) => {
                if n == 20 {
                    self.output.return_ptr = self.output.next_ptr;
                }
                if n == 24 {
                    self.output.next_ptr = usize::from(self.low) | usize::from(self.high) << 8;
                }
                if n == 28 {
                    self.output.state.flags1 |= 2;
                }
                if n == 29 {
                    self.restart()
                } else {
                    CallBody(n + 1)
                }
            }
            CompareLoop(n) => {
                if n == 1 {
                    if self.command == 0xfe {
                        FetchCall(Fetch::LoopCount)
                    } else {
                        NoteType(0)
                    }
                } else {
                    CompareLoop(1)
                }
            }
            LoopCount(n) => {
                if n == 2 {
                    if self.count == 0 {
                        FetchCall(Fetch::LoopLow)
                    } else {
                        LoopCheck(0)
                    }
                } else {
                    LoopCount(n + 1)
                }
            }
            LoopCheck(n) => {
                if n == 5 {
                    if self.remaining > 1 {
                        LoopRepeat(0)
                    } else {
                        LoopReset(0)
                    }
                } else {
                    LoopCheck(n + 1)
                }
            }
            LoopRepeat(n) => {
                if n == 1 {
                    self.output.loop_counter = self.remaining - 1;
                    FetchCall(Fetch::LoopLow)
                } else {
                    LoopRepeat(1)
                }
            }
            LoopReset(n) => {
                if n == 1 {
                    self.output.loop_counter = 0;
                    FetchCall(Fetch::SkipLow)
                } else {
                    LoopReset(1)
                }
            }
            LoopPush => FetchCall(Fetch::LoopHigh),
            LoopBody(n) => {
                if n == 9 {
                    self.output.next_ptr = usize::from(self.low) | usize::from(self.high) << 8;
                }
                if n == 10 {
                    self.restart()
                } else {
                    LoopBody(n + 1)
                }
            }
            NoteType(n) => {
                if n == 2 {
                    if self.command & 0xf0 == 0xd0 {
                        SetSpeed(0)
                    } else {
                        PerfectPitch(0)
                    }
                } else {
                    NoteType(n + 1)
                }
            }
            SetSpeed(n) => {
                if n == 5 {
                    self.output.state.speed = self.command & 15;
                }
                if n == 8 {
                    if self.channel == 3 {
                        Jump
                    } else {
                        FetchCall(Fetch::NoteParameter)
                    }
                } else {
                    SetSpeed(n + 1)
                }
            }
            ParameterKind(n) => {
                if n == 3 && self.channel == 2 {
                    WaveVolume(0)
                } else if n == 5 {
                    self.output.state.volume = self.last;
                    StoreVolume(0)
                } else {
                    ParameterKind(n + 1)
                }
            }
            WaveVolume(n) => {
                if n == 3 {
                    self.output.wave_instrument = Some(self.last & 15);
                }
                if n == 7 {
                    self.output.state.volume = (self.last & 0x30) << 1;
                    StoreVolume(0)
                } else {
                    WaveVolume(n + 1)
                }
            }
            StoreVolume(n) => {
                if n == 4 {
                    self.restart()
                } else {
                    StoreVolume(n + 1)
                }
            }
            PerfectPitch(n) => {
                if n == 2 {
                    if self.command == 0xe8 {
                        TogglePitch(0)
                    } else {
                        Vibrato(0)
                    }
                } else {
                    PerfectPitch(n + 1)
                }
            }
            TogglePitch(n) => {
                if n == 5 {
                    self.output.state.flags1 ^= 1;
                }
                if n == 6 {
                    self.restart()
                } else {
                    TogglePitch(n + 1)
                }
            }
            Vibrato(n) => {
                if n == 1 {
                    if self.command == 0xea {
                        FetchCall(Fetch::VibratoDelay)
                    } else {
                        PitchSlide(0)
                    }
                } else {
                    Vibrato(1)
                }
            }
            VibratoDelay(n) => {
                if n == 3 {
                    self.output.state.vibrato_delay = self.last;
                }
                if n == 6 {
                    self.output.vibrato_reload = self.last;
                    FetchCall(Fetch::VibratoDepth)
                } else {
                    VibratoDelay(n + 1)
                }
            }
            VibratoDepth(n) => {
                if n == 11 {
                    let depth = self.last >> 4;
                    self.output.vibrato_extent = Some((depth / 2) | ((depth / 2 + depth % 2) << 4));
                }
                if n == 19 {
                    self.output.vibrato_rate = Some((self.last & 15) * 17);
                }
                if n == 20 {
                    self.restart()
                } else {
                    VibratoDepth(n + 1)
                }
            }
            PitchSlide(n) => {
                if n == 1 {
                    if self.command == 0xeb {
                        Unsupported(StreamInputError::UnsupportedCommand(self.command))
                    } else {
                        Duty(0)
                    }
                } else {
                    PitchSlide(1)
                }
            }
            Duty(n) => {
                if n == 1 {
                    if self.command == 0xec {
                        FetchCall(Fetch::DutyParameter)
                    } else {
                        Tempo(0)
                    }
                } else {
                    Duty(1)
                }
            }
            SetDuty(n) => {
                if n == 6 {
                    self.output.state.duty = (self.last & 3) << 6;
                }
                if n == 7 {
                    self.restart()
                } else {
                    SetDuty(n + 1)
                }
            }
            Tempo(n) => {
                if n == 1 {
                    if self.command == 0xed {
                        MusicTempo(0)
                    } else {
                        OtherCompare(0, 0)
                    }
                } else {
                    Tempo(1)
                }
            }
            MusicTempo(n) => {
                if n == 2 {
                    FetchCall(Fetch::TempoHigh)
                } else {
                    MusicTempo(n + 1)
                }
            }
            StoreTempoHigh => FetchCall(Fetch::TempoLow),
            StoreTempoLow(n) => {
                if n == 0 {
                    self.output.tempo = Some(u16::from(self.high) << 8 | u16::from(self.last));
                }
                if n == 7 {
                    self.restart()
                } else {
                    StoreTempoLow(n + 1)
                }
            }
            OtherCompare(which, n) => {
                if n == 0 {
                    OtherCompare(which, 1)
                } else if which == 3 && self.command == 0xf0 {
                    FetchCall(Fetch::Volume)
                } else if [0xee, 0xef, 0xfc, 0xf0, 0xf8][which as usize] == self.command {
                    Unsupported(StreamInputError::UnsupportedCommand(self.command))
                } else if which == 4 {
                    Octave(0)
                } else {
                    OtherCompare(which + 1, 0)
                }
            }
            MasterVolume(n) => {
                if n == 0 {
                    self.output.master_volume = Some(self.last);
                }
                if n == 1 {
                    self.restart()
                } else {
                    MasterVolume(1)
                }
            }
            Octave(n) => {
                if n == 2 {
                    if self.command & 0xf0 == 0xe0 {
                        if self.command <= 0xe7 {
                            StoreOctave(0)
                        } else {
                            Unsupported(StreamInputError::UnsupportedCommand(self.command))
                        }
                    } else {
                        SfxNote(0)
                    }
                } else {
                    Octave(n + 1)
                }
            }
            StoreOctave(n) => {
                if n == 5 {
                    self.output.state.octave = self.command & 15;
                }
                if n == 6 {
                    self.restart()
                } else {
                    StoreOctave(n + 1)
                }
            }
            SfxNote(n) => {
                if n == 1 {
                    if self.command & 0xf0 == 0x20 {
                        MusicSfxGate(0)
                    } else {
                        PitchSweep(0)
                    }
                } else {
                    SfxNote(1)
                }
            }
            MusicSfxGate(n) => {
                if n == 2 {
                    PitchSweep(0)
                } else {
                    MusicSfxGate(n + 1)
                }
            }
            PitchSweep(n) => {
                if n == 2 {
                    Note(0)
                } else {
                    PitchSweep(n + 1)
                }
            }
            Note(n) => {
                if n == 2 {
                    if self.command >= 0xd0 {
                        Unsupported(StreamInputError::UnsupportedCommand(self.command))
                    } else if self.channel == 3 {
                        NoiseNote(0)
                    } else {
                        self.output.note = self.command;
                        NoteLength
                    }
                } else {
                    Note(n + 1)
                }
            }
            NoiseNote(n) => {
                if n == 3 && self.command & 0xf0 == 0xb0 {
                    Unsupported(StreamInputError::UnsupportedCommand(self.command))
                } else if n == 4 {
                    if self.command & 0xf0 >= 0xc0 {
                        self.output.note = self.command;
                        NoteLength
                    } else {
                        Unsupported(StreamInputError::UnsupportedCommand(self.command))
                    }
                } else {
                    NoiseNote(n + 1)
                }
            }
            Jump => self.restart(),
            NoteLength => NoteLength,
            Unsupported(e) => Unsupported(e),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[derive(serde::Deserialize)]
    struct Case {
        bank: u8,
        label: alloc::string::String,
        input: StreamInput,
        data: Vec<u8>,
        result: StreamResult,
        trace: Vec<(u16, u8)>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }
    #[test]
    fn command_streams_match_each_original_instruction_and_all_temporary_results() {
        let f: Fixture =
            serde_json::from_str(include_str!("note_stream_instructions_golden.json")).unwrap();
        assert_eq!(f.cases.len(), 478);
        for (index, c) in f.cases.into_iter().enumerate() {
            assert!(matches!(c.bank, 2 | 31));
            let mut w = StreamInstructions::new(c.input).unwrap();
            let mut cycles = 0;
            for (pc, t) in c.trace {
                assert!(!w.finished());
                assert_eq!(w.result(), None);
                let frozen = w.clone();
                assert_eq!(
                    w.try_step(&c.data, |_| Err("yield")),
                    Err(StreamStepError::Clock("yield"))
                );
                assert_eq!(w, frozen);
                w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
                w.try_step(&c.data, |actual| {
                    assert_eq!(actual, t, "case {index} {} source PC {pc:04x}", c.label);
                    Ok::<(), ()>(())
                })
                .unwrap();
                cycles += u32::from(t);
                assert_eq!(w.cycles(), cycles);
            }
            assert!(w.finished(), "case {index} {}", c.label);
            assert_eq!(w.result(), Some(c.result), "case {index} {}", c.label);
            assert!(w
                .try_step(&c.data, |_| -> Result<(), ()> {
                    panic!("note-length boundary")
                })
                .unwrap());
        }
    }
    #[test]
    fn missing_bytes_unsupported_commands_and_nonterminating_streams_never_invent_a_note() {
        let input = StreamInput {
            channel: 0,
            ptr: 0,
            return_ptr: 0,
            loop_counter: 0,
            state: MusicCommandState {
                speed: 6,
                volume: 179,
                duty: 192,
                octave: 6,
                flags1: 0,
                vibrato_delay: 0,
            },
            vibrato_reload: 4,
        };
        assert!(StreamInstructions::new(StreamInput {
            channel: 4,
            ..input
        })
        .is_err());
        for (data, expected) in [
            (&[0xec][..], StreamInputError::TruncatedStream(1)),
            (&[0xff][..], StreamInputError::ChannelEndUnsupported),
            (
                &[0xeb, 1, 2, 3][..],
                StreamInputError::UnsupportedCommand(0xeb),
            ),
            (&[0xfe, 0, 0, 0][..], StreamInputError::CommandLimit),
        ] {
            let mut w = StreamInstructions::new(input).unwrap();
            let mut failure = None;
            for _ in 0..30000 {
                match w.try_step(data, |_| Ok::<(), ()>(())) {
                    Ok(false) => (),
                    Ok(true) => panic!("fabricated note"),
                    Err(e) => {
                        failure = Some(e);
                        break;
                    }
                }
            }
            assert_eq!(failure, Some(StreamStepError::Input(expected)));
            let frozen = w.clone();
            w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
            assert_eq!(
                w.try_step(data, |_| -> Result<(), ()> {
                    panic!("input failure executes nothing")
                }),
                Err(StreamStepError::Input(expected))
            );
            assert_eq!(w, frozen);
            assert_eq!(w.result(), None);
        }
    }
}
