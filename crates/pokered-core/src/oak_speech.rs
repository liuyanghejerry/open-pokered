use crate::game_state::Lang;
use crate::naming_screen::{
    NamingInput, NamingScreenResult, NamingScreenState, NamingScreenType, NAMING_FLASH_FRAMES,
};

pub const DEFAULT_PLAYER_NAMES: [&str; 4] = ["RED", "ASH", "JACK", "NEW NAME"];
pub const DEFAULT_RIVAL_NAMES: [&str; 4] = ["BLUE", "GARY", "JOHN", "NEW NAME"];

const TYPEWRITER_CHARS_PER_FRAME: u16 = 1;

// ── Pic entrance animations (engine/movie/oak_speech/oak_speech.asm) ──

/// FadeInIntroPic (oak_speech.asm:191-209): 6 palette steps × DelayFrames(10),
/// used for ProfOakPic (Greeting) and Rival1Pic (IntroduceRival).
pub const INTRO_FADE_IN_FRAMES: u16 = 60;
/// IntroFadePalettes (oak_speech.asm:203-209): the 6 rBGP values of the ramp.
pub const INTRO_FADE_PALETTES: [u8; 6] = [0x54, 0xA8, 0xFC, 0xF8, 0xF4, 0xE4];
/// MovePicLeft (oak_speech.asm:211-225): rWX starts at 119 and is decremented
/// by 8 once per frame until it would wrap to $FF — 15 displayed positions —
/// sliding the pic in from the right. Used for the Nidorino (ShowNidorino)
/// and RedPicFront (IntroducePlayer) entrances.
pub const INTRO_SLIDE_IN_FRAMES: u16 = 15;
/// GBFadeInFromWhite before OakSpeechText3 (oak_speech.asm:108): FadePal7→5,
/// 3 steps × 8 frames (home/fade.asm).
pub const FINAL_FADE_IN_FRAMES: u16 = 24;

/// Frames of pic entrance animation before a text phase's typewriter starts
/// (the original finishes the entrance before PrintText is called).
pub fn entrance_frames(phase: &OakSpeechPhase) -> u16 {
    match phase {
        OakSpeechPhase::Greeting { .. } | OakSpeechPhase::IntroduceRival { .. } => {
            INTRO_FADE_IN_FRAMES
        }
        OakSpeechPhase::ShowNidorino { .. } | OakSpeechPhase::IntroducePlayer { .. } => {
            INTRO_SLIDE_IN_FRAMES
        }
        OakSpeechPhase::FinalSpeech { .. } => FINAL_FADE_IN_FRAMES,
        _ => 0,
    }
}

/// Horizontal pixel offset of the incoming pic during a MovePicLeft entrance
/// (window left edge = rWX - 7, rWX = 119 - 8·frame).
pub fn entrance_slide_offset(phase_frame: u16) -> u32 {
    (112i32 - 8 * phase_frame as i32).max(0) as u32
}

// ── Pic slide when the default-name menu opens/closes (oak_speech2.asm) ──

/// OakSpeechSlidePicRight/Left shift the pic one tile-column per iteration.
pub const SLIDE_PIC_STEPS: u8 = 6;
/// Delay3 between slide iterations (oak_speech2.asm:138).
pub const SLIDE_PIC_STEP_FRAMES: u8 = 3;
/// Total frames of OakSpeechSlidePicRight: 6 iterations × Delay3.
pub const SLIDE_PIC_RIGHT_FRAMES: u8 = SLIDE_PIC_STEPS * SLIDE_PIC_STEP_FRAMES;
/// OakSpeechSlidePicLeft first clears the name list box (DelayFrames 10,
/// oak_speech2.asm:72-73) and waits Delay3 (line 78) before sliding.
pub const SLIDE_PIC_LEFT_PRE_DELAY: u8 = 13;
/// Total frames of OakSpeechSlidePicLeft.
pub const SLIDE_PIC_LEFT_FRAMES: u8 = SLIDE_PIC_LEFT_PRE_DELAY + SLIDE_PIC_STEPS * SLIDE_PIC_STEP_FRAMES;

/// Pic tilemap x when centered (hlcoord 6,4) and when slid right (hlcoord 12,4).
pub const INTRO_PIC_X: u32 = 6 * 8;
pub const INTRO_PIC_SLID_X: u32 = 12 * 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicSlideDirection {
    /// Menu opening: pic slides right one column per iteration.
    Right,
    /// Default name chosen: pic slides left back to centered.
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicSlideSubject {
    Player,
    Rival,
}

/// Absolute pixel x of the pic's left edge at `frame` of a SlidePic phase.
pub fn slide_pic_x(direction: PicSlideDirection, frame: u8) -> u32 {
    match direction {
        PicSlideDirection::Right => {
            let step = (frame / SLIDE_PIC_STEP_FRAMES) as u32 + 1;
            INTRO_PIC_X + step * 8
        }
        PicSlideDirection::Left => {
            if frame < SLIDE_PIC_LEFT_PRE_DELAY {
                INTRO_PIC_SLID_X
            } else {
                let step = ((frame - SLIDE_PIC_LEFT_PRE_DELAY) / SLIDE_PIC_STEP_FRAMES) as u32 + 1;
                INTRO_PIC_SLID_X.saturating_sub(step * 8)
            }
        }
    }
}

// ── ShrinkPlayer finale beats (oak_speech.asm:115-165) ──

/// SFX_SHRINK plays, then RedPicFront stays for DelayFrames(4).
pub const SHRINK_BEAT_RED_END: u16 = 4;
/// ShrinkPic1 for DelayFrames(4).
pub const SHRINK_BEAT_PIC1_END: u16 = SHRINK_BEAT_RED_END + 4;
/// ShrinkPic2 for DelayFrames(20).
pub const SHRINK_BEAT_PIC2_END: u16 = SHRINK_BEAT_PIC1_END + 20;
/// 7×7 area at (6,5) cleared, DelayFrames(50).
pub const SHRINK_BEAT_CLEARED_END: u16 = SHRINK_BEAT_PIC2_END + 50;
/// GBFadeOutToWhite (FadePal6→8, 3 × 8 frames), then Done.
pub const SHRINK_FADE_OUT_FRAMES: u16 = 24;
pub const SHRINK_TOTAL_FRAMES: u16 = SHRINK_BEAT_CLEARED_END + SHRINK_FADE_OUT_FRAMES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPage {
    pub line1: &'static str,
    pub line2: &'static str,
}

impl TextPage {
    pub const fn new(line1: &'static str, line2: &'static str) -> Self {
        Self { line1, line2 }
    }

    pub fn total_chars(&self, player_name: Option<&str>) -> usize {
        let l1 = Self::replace_player(self.line1, player_name);
        let l2 = Self::replace_player(self.line2, player_name);
        l1.chars().count() + l2.chars().count()
    }

    fn replace_player(text: &str, player_name: Option<&str>) -> String {
        text.replace("<PLAYER>", player_name.unwrap_or("RED"))
    }

    pub fn get_display_text(&self, player_name: Option<&str>, char_index: u16) -> (String, String) {
        let l1 = Self::replace_player(self.line1, player_name);
        let l2 = Self::replace_player(self.line2, player_name);

        let l1_chars: Vec<char> = l1.chars().collect();
        let l2_chars: Vec<char> = l2.chars().collect();
        let total = l1_chars.len() + l2_chars.len();
        let idx = char_index as usize;

        if idx >= total {
            (l1, l2)
        } else if idx <= l1_chars.len() {
            (l1_chars[..idx].iter().collect(), String::new())
        } else {
            let l2_idx = idx - l1_chars.len();
            (l1.clone(), l2_chars[..l2_idx].iter().collect())
        }
    }
}

pub const OAK_SPEECH_TEXT1_PAGES: &[TextPage] = &[
    TextPage::new("Hello there!", "Welcome to the"),
    TextPage::new("world of #MON!", ""),
    TextPage::new("My name is OAK!", "People call me"),
    TextPage::new("the #MON PROF!", ""),
];

pub const OAK_SPEECH_TEXT2A_PAGES: &[TextPage] = &[
    TextPage::new("This world is", "inhabited by"),
    TextPage::new("creatures called", "#MON!"),
];

pub const OAK_SPEECH_TEXT2B_PAGES: &[TextPage] = &[
    TextPage::new("For some people,", "#MON are"),
    TextPage::new("pets. Others use", "them for fights."),
    TextPage::new("Myself...", ""),
    TextPage::new("I study #MON", "as a profession."),
];

pub const INTRODUCE_PLAYER_TEXT_PAGES: &[TextPage] =
    &[TextPage::new("First, what is", "your name?")];

pub const INTRODUCE_RIVAL_TEXT_PAGES: &[TextPage] = &[
    TextPage::new("This is my grand-", "son. He's been"),
    TextPage::new("your rival since", "you were a baby."),
    TextPage::new("...Erm, what is", "his name again?"),
];

pub const OAK_SPEECH_TEXT3_PAGES: &[TextPage] = &[
    TextPage::new("<PLAYER>!", ""),
    TextPage::new("Your very own", "#MON legend is"),
    TextPage::new("about to unfold!", ""),
    TextPage::new("A world of dreams", "and adventures"),
    TextPage::new("with #MON", "awaits! Let's go!"),
];

// ── Chinese (zh) text pages ────────────────────────────────────────

pub const OAK_SPEECH_TEXT1_PAGES_ZH: &[TextPage] = &[
    TextPage::new("你好！欢迎来到", "宝可梦的世界！"),
    TextPage::new("我是大木博士！", "大家都叫我"),
    TextPage::new("宝可梦博士！", ""),
];

pub const OAK_SPEECH_TEXT2A_PAGES_ZH: &[TextPage] = &[
    TextPage::new("这个世界生活着", "一种叫做宝可梦的"),
    TextPage::new("神奇生物！", ""),
];

pub const OAK_SPEECH_TEXT2B_PAGES_ZH: &[TextPage] = &[
    TextPage::new("对一些人来说，", "宝可梦是宠物。"),
    TextPage::new("另一些人会用", "它们来对战。"),
    TextPage::new("而我……", ""),
    TextPage::new("则以研究宝可梦", "为职业。"),
];

pub const INTRODUCE_PLAYER_TEXT_PAGES_ZH: &[TextPage] =
    &[TextPage::new("首先，请问你", "叫什么名字？")];

pub const INTRODUCE_RIVAL_TEXT_PAGES_ZH: &[TextPage] = &[
    TextPage::new("这是我的孙子。", "从你还是婴儿时"),
    TextPage::new("他就是你的", "竞争对手了。"),
    TextPage::new("……呃，他叫", "什么名字来着？"),
];

pub const OAK_SPEECH_TEXT3_PAGES_ZH: &[TextPage] = &[
    TextPage::new("<PLAYER>！", ""),
    TextPage::new("属于你的宝可梦", "传说就要开始了！"),
    TextPage::new("一个充满梦想", "与冒险的世界"),
    TextPage::new("正等着你！", "出发吧！"),
];

pub fn text_pages_for_lang(phase: &OakSpeechPhase, lang: Lang) -> Option<&'static TextPage> {
    let (pages, page_index) = match phase {
        OakSpeechPhase::Greeting { page_index, .. } => {
            if lang == Lang::Zh { (OAK_SPEECH_TEXT1_PAGES_ZH.as_ref(), *page_index) }
            else { (OAK_SPEECH_TEXT1_PAGES.as_ref(), *page_index) }
        }
        OakSpeechPhase::ShowNidorino { page_index, .. } => {
            if lang == Lang::Zh { (OAK_SPEECH_TEXT2A_PAGES_ZH.as_ref(), *page_index) }
            else { (OAK_SPEECH_TEXT2A_PAGES.as_ref(), *page_index) }
        }
        OakSpeechPhase::Explanation { page_index, .. } => {
            if lang == Lang::Zh { (OAK_SPEECH_TEXT2B_PAGES_ZH.as_ref(), *page_index) }
            else { (OAK_SPEECH_TEXT2B_PAGES.as_ref(), *page_index) }
        }
        OakSpeechPhase::IntroducePlayer { page_index, .. } => {
            if lang == Lang::Zh { (INTRODUCE_PLAYER_TEXT_PAGES_ZH.as_ref(), *page_index) }
            else { (INTRODUCE_PLAYER_TEXT_PAGES.as_ref(), *page_index) }
        }
        OakSpeechPhase::IntroduceRival { page_index, .. } => {
            if lang == Lang::Zh { (INTRODUCE_RIVAL_TEXT_PAGES_ZH.as_ref(), *page_index) }
            else { (INTRODUCE_RIVAL_TEXT_PAGES.as_ref(), *page_index) }
        }
        OakSpeechPhase::FinalSpeech { page_index, .. } => {
            if lang == Lang::Zh { (OAK_SPEECH_TEXT3_PAGES_ZH.as_ref(), *page_index) }
            else { (OAK_SPEECH_TEXT3_PAGES.as_ref(), *page_index) }
        }
        _ => return None,
    };
    pages.get(page_index)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OakSpeechPhase {
    Greeting {
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
    },
    ShowNidorino {
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
    },
    Explanation {
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
    },
    IntroducePlayer {
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
    },
    PlayerNameChoice {
        cursor: usize,
    },
    PlayerNaming,
    IntroduceRival {
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
    },
    RivalNameChoice {
        cursor: usize,
    },
    RivalNaming,
    FinalSpeech {
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
    },
    /// Timed pic slide played when the default-name menu opens
    /// (`OakSpeechSlidePicRight`) or closes after a default name is picked
    /// (`OakSpeechSlidePicLeft`), oak_speech2.asm:67-160.
    SlidePic {
        direction: PicSlideDirection,
        subject: PicSlideSubject,
        frame: u8,
    },
    ShrinkPlayer {
        /// Frames since the shrink sequence started (SFX_SHRINK beat 0);
        /// see the SHRINK_BEAT_* constants.
        frame: u16,
    },
    Done,
}

impl OakSpeechPhase {
    fn text_pages(&self) -> Option<&'static [TextPage]> {
        match self {
            OakSpeechPhase::Greeting { .. } => Some(OAK_SPEECH_TEXT1_PAGES),
            OakSpeechPhase::ShowNidorino { .. } => Some(OAK_SPEECH_TEXT2A_PAGES),
            OakSpeechPhase::Explanation { .. } => Some(OAK_SPEECH_TEXT2B_PAGES),
            OakSpeechPhase::IntroducePlayer { .. } => Some(INTRODUCE_PLAYER_TEXT_PAGES),
            OakSpeechPhase::IntroduceRival { .. } => Some(INTRODUCE_RIVAL_TEXT_PAGES),
            OakSpeechPhase::FinalSpeech { .. } => Some(OAK_SPEECH_TEXT3_PAGES),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OakSpeechInput {
    pub up: bool,
    pub down: bool,
    pub a: bool,
    pub b: bool,
}

impl OakSpeechInput {
    pub fn none() -> Self {
        Self {
            up: false,
            down: false,
            a: false,
            b: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OakSpeechResult {
    Active,
    PlayerNameSet(String),
    RivalNameSet(String),
    Finished,
}

#[derive(Debug, Clone)]
pub struct OakSpeechState {
    pub phase: OakSpeechPhase,
    pub player_name: Option<String>,
    pub rival_name: Option<String>,
    pub naming_screen: Option<NamingScreenState>,
    /// Frames elapsed in the current phase (0 on the frame the phase was
    /// entered); drives the pic entrance animations (`entrance_frames`).
    pub phase_frame: u16,
    /// Remaining frames of the naming-screen white flash
    /// (`GBPalWhiteOutWithDelay3`, naming_screen.asm:88/163) on open/submit.
    flash_frames: u8,
    /// Name picked from the default-name list during the last
    /// `process_phase` call; surfaced by `update_frame` so frontends can
    /// mirror the choice into their own state (start menu, save file).
    pending_result: Option<OakSpeechResult>,
}

impl OakSpeechState {
    pub fn new() -> Self {
        Self {
            phase: OakSpeechPhase::Greeting {
                page_index: 0,
                char_index: 0,
                waiting_for_input: false,
            },
            player_name: None,
            rival_name: None,
            naming_screen: None,
            phase_frame: 0,
            flash_frames: 0,
            pending_result: None,
        }
    }

    pub fn is_flashing(&self) -> bool {
        self.flash_frames > 0
    }

    pub fn update_frame(&mut self, input: OakSpeechInput) -> OakSpeechResult {
        if self.flash_frames > 0 {
            self.flash_frames -= 1;
            return OakSpeechResult::Active;
        }

        let prev_phase = std::mem::discriminant(&self.phase);
        if let Some(new_phase) = self.process_phase(input) {
            self.phase = new_phase;
        }
        if std::mem::discriminant(&self.phase) == prev_phase {
            self.phase_frame += 1;
        } else {
            self.phase_frame = 0;
        }

        if let Some(result) = self.pending_result.take() {
            return result;
        }

        if self.phase == OakSpeechPhase::Done {
            OakSpeechResult::Finished
        } else {
            OakSpeechResult::Active
        }
    }

    fn process_phase(&mut self, input: OakSpeechInput) -> Option<OakSpeechPhase> {
        let new_phase = match &self.phase {
            OakSpeechPhase::Greeting {
                page_index,
                char_index,
                waiting_for_input,
            } => self.process_text_phase_inline(
                input,
                *page_index,
                *char_index,
                *waiting_for_input,
                OAK_SPEECH_TEXT1_PAGES,
                INTRO_FADE_IN_FRAMES,
                |pi, ci, wfi| OakSpeechPhase::Greeting {
                    page_index: pi,
                    char_index: ci,
                    waiting_for_input: wfi,
                },
                || OakSpeechPhase::ShowNidorino {
                    page_index: 0,
                    char_index: 0,
                    waiting_for_input: false,
                },
            ),
            OakSpeechPhase::ShowNidorino {
                page_index,
                char_index,
                waiting_for_input,
            } => self.process_text_phase_inline(
                input,
                *page_index,
                *char_index,
                *waiting_for_input,
                OAK_SPEECH_TEXT2A_PAGES,
                INTRO_SLIDE_IN_FRAMES,
                |pi, ci, wfi| OakSpeechPhase::ShowNidorino {
                    page_index: pi,
                    char_index: ci,
                    waiting_for_input: wfi,
                },
                || OakSpeechPhase::Explanation {
                    page_index: 0,
                    char_index: 0,
                    waiting_for_input: false,
                },
            ),
            OakSpeechPhase::Explanation {
                page_index,
                char_index,
                waiting_for_input,
            } => self.process_text_phase_inline(
                input,
                *page_index,
                *char_index,
                *waiting_for_input,
                OAK_SPEECH_TEXT2B_PAGES,
                0,
                |pi, ci, wfi| OakSpeechPhase::Explanation {
                    page_index: pi,
                    char_index: ci,
                    waiting_for_input: wfi,
                },
                || OakSpeechPhase::IntroducePlayer {
                    page_index: 0,
                    char_index: 0,
                    waiting_for_input: false,
                },
            ),
            OakSpeechPhase::IntroducePlayer {
                page_index,
                char_index,
                waiting_for_input,
            } => self.process_text_phase_inline(
                input,
                *page_index,
                *char_index,
                *waiting_for_input,
                INTRODUCE_PLAYER_TEXT_PAGES,
                INTRO_SLIDE_IN_FRAMES,
                |pi, ci, wfi| OakSpeechPhase::IntroducePlayer {
                    page_index: pi,
                    char_index: ci,
                    waiting_for_input: wfi,
                },
                || OakSpeechPhase::SlidePic {
                    direction: PicSlideDirection::Right,
                    subject: PicSlideSubject::Player,
                    frame: 0,
                },
            ),
            OakSpeechPhase::IntroduceRival {
                page_index,
                char_index,
                waiting_for_input,
            } => self.process_text_phase_inline(
                input,
                *page_index,
                *char_index,
                *waiting_for_input,
                INTRODUCE_RIVAL_TEXT_PAGES,
                INTRO_FADE_IN_FRAMES,
                |pi, ci, wfi| OakSpeechPhase::IntroduceRival {
                    page_index: pi,
                    char_index: ci,
                    waiting_for_input: wfi,
                },
                || OakSpeechPhase::SlidePic {
                    direction: PicSlideDirection::Right,
                    subject: PicSlideSubject::Rival,
                    frame: 0,
                },
            ),
            OakSpeechPhase::FinalSpeech {
                page_index,
                char_index,
                waiting_for_input,
            } => self.process_text_phase_inline(
                input,
                *page_index,
                *char_index,
                *waiting_for_input,
                OAK_SPEECH_TEXT3_PAGES,
                FINAL_FADE_IN_FRAMES,
                |pi, ci, wfi| OakSpeechPhase::FinalSpeech {
                    page_index: pi,
                    char_index: ci,
                    waiting_for_input: wfi,
                },
                || OakSpeechPhase::ShrinkPlayer { frame: 0 },
            ),
            OakSpeechPhase::PlayerNameChoice { cursor } => {
                let mut new_cursor = *cursor;
                if input.up && new_cursor > 0 {
                    new_cursor -= 1;
                } else if input.down && new_cursor < DEFAULT_PLAYER_NAMES.len() - 1 {
                    new_cursor += 1;
                }
                if input.a {
                    if new_cursor == DEFAULT_PLAYER_NAMES.len() - 1 {
                        self.naming_screen = Some(NamingScreenState::new(NamingScreenType::Player));
                        self.flash_frames = NAMING_FLASH_FRAMES;
                        return Some(OakSpeechPhase::PlayerNaming);
                    } else {
                        let name = DEFAULT_PLAYER_NAMES[new_cursor].to_string();
                        self.player_name = Some(name.clone());
                        self.pending_result = Some(OakSpeechResult::PlayerNameSet(name));
                        return Some(OakSpeechPhase::SlidePic {
                            direction: PicSlideDirection::Left,
                            subject: PicSlideSubject::Player,
                            frame: 0,
                        });
                    }
                }
                return Some(OakSpeechPhase::PlayerNameChoice { cursor: new_cursor });
            }
            OakSpeechPhase::PlayerNaming => None,
            OakSpeechPhase::RivalNameChoice { cursor } => {
                let mut new_cursor = *cursor;
                if input.up && new_cursor > 0 {
                    new_cursor -= 1;
                } else if input.down && new_cursor < DEFAULT_RIVAL_NAMES.len() - 1 {
                    new_cursor += 1;
                }
                if input.a {
                    if new_cursor == DEFAULT_RIVAL_NAMES.len() - 1 {
                        self.naming_screen = Some(NamingScreenState::new(NamingScreenType::Rival));
                        self.flash_frames = NAMING_FLASH_FRAMES;
                        return Some(OakSpeechPhase::RivalNaming);
                    } else {
                        let name = DEFAULT_RIVAL_NAMES[new_cursor].to_string();
                        self.rival_name = Some(name.clone());
                        self.pending_result = Some(OakSpeechResult::RivalNameSet(name));
                        return Some(OakSpeechPhase::SlidePic {
                            direction: PicSlideDirection::Left,
                            subject: PicSlideSubject::Rival,
                            frame: 0,
                        });
                    }
                }
                return Some(OakSpeechPhase::RivalNameChoice { cursor: new_cursor });
            }
            OakSpeechPhase::RivalNaming => None,
            OakSpeechPhase::SlidePic {
                direction,
                subject,
                frame,
            } => {
                let total = match direction {
                    PicSlideDirection::Right => SLIDE_PIC_RIGHT_FRAMES,
                    PicSlideDirection::Left => SLIDE_PIC_LEFT_FRAMES,
                };
                let next = frame + 1;
                if next < total {
                    return Some(OakSpeechPhase::SlidePic {
                        direction: *direction,
                        subject: *subject,
                        frame: next,
                    });
                }
                match (direction, subject) {
                    (PicSlideDirection::Right, PicSlideSubject::Player) => {
                        Some(OakSpeechPhase::PlayerNameChoice { cursor: 0 })
                    }
                    (PicSlideDirection::Right, PicSlideSubject::Rival) => {
                        Some(OakSpeechPhase::RivalNameChoice { cursor: 0 })
                    }
                    (PicSlideDirection::Left, PicSlideSubject::Player) => {
                        Some(OakSpeechPhase::IntroduceRival {
                            page_index: 0,
                            char_index: 0,
                            waiting_for_input: false,
                        })
                    }
                    (PicSlideDirection::Left, PicSlideSubject::Rival) => {
                        Some(OakSpeechPhase::FinalSpeech {
                            page_index: 0,
                            char_index: 0,
                            waiting_for_input: false,
                        })
                    }
                }
            }
            OakSpeechPhase::ShrinkPlayer { frame } => {
                let next = frame + 1;
                if next >= SHRINK_TOTAL_FRAMES {
                    return Some(OakSpeechPhase::Done);
                }
                return Some(OakSpeechPhase::ShrinkPlayer { frame: next });
            }
            OakSpeechPhase::Done => None,
        };
        new_phase
    }

    fn process_text_phase_inline<F, G>(
        &self,
        input: OakSpeechInput,
        page_index: usize,
        char_index: u16,
        waiting_for_input: bool,
        pages: &[TextPage],
        entrance: u16,
        make_current: F,
        make_next: G,
    ) -> Option<OakSpeechPhase>
    where
        F: FnOnce(usize, u16, bool) -> OakSpeechPhase,
        G: FnOnce() -> OakSpeechPhase,
    {
        // The pic entrance animation (FadeInIntroPic / MovePicLeft /
        // GBFadeInFromWhite) runs to completion before PrintText is called;
        // joypad input is ignored while it plays.
        if self.phase_frame < entrance {
            return None;
        }
        if waiting_for_input {
            if input.a || input.b {
                let total_pages = pages.len();
                if page_index + 1 >= total_pages {
                    return Some(make_next());
                } else {
                    return Some(make_current(page_index + 1, 0, false));
                }
            }
            return None;
        }

        let page = &pages[page_index];
        let total_chars = page.total_chars(self.player_name.as_deref()) as u16;

        if input.a || input.b {
            return Some(make_current(page_index, total_chars, true));
        }

        let new_char_index = char_index + TYPEWRITER_CHARS_PER_FRAME;
        if new_char_index >= total_chars {
            Some(make_current(page_index, total_chars, true))
        } else {
            Some(make_current(page_index, new_char_index, false))
        }
    }

    pub fn update_naming_frame(&mut self, input: NamingInput, is_zh: bool) -> OakSpeechResult {
        if self.flash_frames > 0 {
            self.flash_frames -= 1;
            return OakSpeechResult::Active;
        }

        let Some(naming) = &mut self.naming_screen else {
            return OakSpeechResult::Active;
        };

        match naming.update_frame(input, is_zh) {
            NamingScreenResult::Editing => OakSpeechResult::Active,
            NamingScreenResult::Submitted(name) => match self.phase {
                OakSpeechPhase::PlayerNaming => {
                    self.player_name = Some(name.clone());
                    self.naming_screen = None;
                    self.flash_frames = NAMING_FLASH_FRAMES;
                    self.phase = OakSpeechPhase::RivalNameChoice { cursor: 0 };
                    OakSpeechResult::PlayerNameSet(name)
                }
                OakSpeechPhase::RivalNaming => {
                    self.rival_name = Some(name.clone());
                    self.naming_screen = None;
                    self.flash_frames = NAMING_FLASH_FRAMES;
                    self.phase = OakSpeechPhase::ShrinkPlayer { frame: 0 };
                    OakSpeechResult::RivalNameSet(name)
                }
                _ => OakSpeechResult::Active,
            },
            NamingScreenResult::Cancelled => match self.phase {
                OakSpeechPhase::PlayerNaming => {
                    let name = DEFAULT_PLAYER_NAMES[0].to_string();
                    self.player_name = Some(name.clone());
                    self.naming_screen = None;
                    self.flash_frames = NAMING_FLASH_FRAMES;
                    self.phase = OakSpeechPhase::RivalNameChoice { cursor: 0 };
                    OakSpeechResult::PlayerNameSet(name)
                }
                OakSpeechPhase::RivalNaming => {
                    let name = DEFAULT_RIVAL_NAMES[0].to_string();
                    self.rival_name = Some(name.clone());
                    self.naming_screen = None;
                    self.flash_frames = NAMING_FLASH_FRAMES;
                    self.phase = OakSpeechPhase::ShrinkPlayer { frame: 0 };
                    OakSpeechResult::RivalNameSet(name)
                }
                _ => OakSpeechResult::Active,
            },
        }
    }

    pub fn is_naming_active(&self) -> bool {
        matches!(
            self.phase,
            OakSpeechPhase::PlayerNaming | OakSpeechPhase::RivalNaming
        )
    }

    pub fn current_text_page(&self) -> Option<TextPage> {
        match &self.phase {
            OakSpeechPhase::Greeting { page_index, .. } => {
                OAK_SPEECH_TEXT1_PAGES.get(*page_index).cloned()
            }
            OakSpeechPhase::ShowNidorino { page_index, .. } => {
                OAK_SPEECH_TEXT2A_PAGES.get(*page_index).cloned()
            }
            OakSpeechPhase::Explanation { page_index, .. } => {
                OAK_SPEECH_TEXT2B_PAGES.get(*page_index).cloned()
            }
            OakSpeechPhase::IntroducePlayer { page_index, .. } => {
                INTRODUCE_PLAYER_TEXT_PAGES.get(*page_index).cloned()
            }
            OakSpeechPhase::IntroduceRival { page_index, .. } => {
                INTRODUCE_RIVAL_TEXT_PAGES.get(*page_index).cloned()
            }
            OakSpeechPhase::FinalSpeech { page_index, .. } => {
                OAK_SPEECH_TEXT3_PAGES.get(*page_index).cloned()
            }
            _ => None,
        }
    }

    pub fn current_char_index(&self) -> u16 {
        match &self.phase {
            OakSpeechPhase::Greeting { char_index, .. } => *char_index,
            OakSpeechPhase::ShowNidorino { char_index, .. } => *char_index,
            OakSpeechPhase::Explanation { char_index, .. } => *char_index,
            OakSpeechPhase::IntroducePlayer { char_index, .. } => *char_index,
            OakSpeechPhase::IntroduceRival { char_index, .. } => *char_index,
            OakSpeechPhase::FinalSpeech { char_index, .. } => *char_index,
            _ => 0,
        }
    }

    pub fn is_waiting_for_input(&self) -> bool {
        match &self.phase {
            OakSpeechPhase::Greeting {
                waiting_for_input, ..
            } => *waiting_for_input,
            OakSpeechPhase::ShowNidorino {
                waiting_for_input, ..
            } => *waiting_for_input,
            OakSpeechPhase::Explanation {
                waiting_for_input, ..
            } => *waiting_for_input,
            OakSpeechPhase::IntroducePlayer {
                waiting_for_input, ..
            } => *waiting_for_input,
            OakSpeechPhase::IntroduceRival {
                waiting_for_input, ..
            } => *waiting_for_input,
            OakSpeechPhase::FinalSpeech {
                waiting_for_input, ..
            } => *waiting_for_input,
            _ => false,
        }
    }

    pub fn current_intro_text(&self) -> Option<String> {
        let page = self.current_text_page()?;
        let player_name = self.player_name.as_deref();
        let l1 = page.line1.replace("<PLAYER>", player_name.unwrap_or("RED"));
        let l2 = page.line2.replace("<PLAYER>", player_name.unwrap_or("RED"));
        if l2.is_empty() {
            Some(l1)
        } else {
            Some(format!("{} {}", l1, l2))
        }
    }

    pub fn get_display_text(&self, text: &str) -> String {
        text.replace("<PLAYER>", self.player_name.as_deref().unwrap_or("RED"))
    }
}

impl Default for OakSpeechState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_a() -> OakSpeechInput {
        OakSpeechInput {
            a: true,
            ..OakSpeechInput::none()
        }
    }

    /// Drive the state machine to just after the Greeting phase.
    fn reach_phase(speech: &mut OakSpeechState, pred: fn(&OakSpeechPhase) -> bool) {
        for _ in 0..10_000 {
            if pred(&speech.phase) {
                return;
            }
            let input = if speech.is_waiting_for_input() {
                press_a()
            } else {
                OakSpeechInput::none()
            };
            speech.update_frame(input);
        }
        panic!("did not reach target phase");
    }

    #[test]
    fn greeting_entrance_gates_typewriter() {
        let mut speech = OakSpeechState::new();
        // FadeInIntroPic: 6 steps × 10 frames = 60 frames before PrintText.
        // Joypad input is ignored during the entrance (DelayFrames).
        for i in 0..INTRO_FADE_IN_FRAMES {
            let input = if i == 30 {
                press_a()
            } else {
                OakSpeechInput::none()
            };
            speech.update_frame(input);
            assert_eq!(speech.current_char_index(), 0);
            assert!(matches!(speech.phase, OakSpeechPhase::Greeting { .. }));
        }
        // After the entrance the typewriter starts.
        for _ in 0..3 {
            speech.update_frame(OakSpeechInput::none());
        }
        assert!(speech.current_char_index() > 0);
    }

    #[test]
    fn slide_in_entrance_gates_nidorino_text() {
        let mut speech = OakSpeechState::new();
        reach_phase(&mut speech, |p| matches!(p, OakSpeechPhase::ShowNidorino { .. }));
        // MovePicLeft: 15 frames of slide-in before the text prints.
        for _ in 0..INTRO_SLIDE_IN_FRAMES {
            speech.update_frame(OakSpeechInput::none());
            assert!(matches!(speech.phase, OakSpeechPhase::ShowNidorino { .. }));
            assert_eq!(speech.current_char_index(), 0);
        }
        for _ in 0..3 {
            speech.update_frame(OakSpeechInput::none());
        }
        assert!(speech.current_char_index() > 0);
    }

    #[test]
    fn slide_right_runs_before_name_choice() {
        let mut speech = OakSpeechState::new();
        reach_phase(&mut speech, |p| matches!(p, OakSpeechPhase::IntroducePlayer { .. }));
        reach_phase(&mut speech, |p| matches!(p, OakSpeechPhase::SlidePic { .. }));
        match speech.phase {
            OakSpeechPhase::SlidePic {
                direction,
                subject,
                frame,
            } => {
                assert_eq!(direction, PicSlideDirection::Right);
                assert_eq!(subject, PicSlideSubject::Player);
                assert_eq!(frame, 0);
            }
            _ => unreachable!(),
        }
        // 6 iterations × Delay3 = 18 frames, then the menu opens.
        for _ in 0..SLIDE_PIC_RIGHT_FRAMES {
            assert!(matches!(speech.phase, OakSpeechPhase::SlidePic { .. }));
            speech.update_frame(OakSpeechInput::none());
        }
        assert!(matches!(
            speech.phase,
            OakSpeechPhase::PlayerNameChoice { .. }
        ));
    }

    #[test]
    fn slide_left_runs_after_default_name() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::PlayerNameChoice { cursor: 0 };
        let result = speech.update_frame(press_a());
        assert!(matches!(result, OakSpeechResult::PlayerNameSet(ref n) if n == "RED"));
        assert!(matches!(
            speech.phase,
            OakSpeechPhase::SlidePic {
                direction: PicSlideDirection::Left,
                subject: PicSlideSubject::Player,
                ..
            }
        ));
        // 13-frame pre-delay + 6 iterations × Delay3 = 31 frames.
        for _ in 0..SLIDE_PIC_LEFT_FRAMES {
            assert!(matches!(speech.phase, OakSpeechPhase::SlidePic { .. }));
            speech.update_frame(OakSpeechInput::none());
        }
        assert!(matches!(
            speech.phase,
            OakSpeechPhase::IntroduceRival { .. }
        ));
    }

    #[test]
    fn rival_slide_right_runs_before_rival_name_choice() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::SlidePic {
            direction: PicSlideDirection::Right,
            subject: PicSlideSubject::Rival,
            frame: 0,
        };
        for _ in 0..SLIDE_PIC_RIGHT_FRAMES {
            speech.update_frame(OakSpeechInput::none());
        }
        assert!(matches!(
            speech.phase,
            OakSpeechPhase::RivalNameChoice { .. }
        ));
    }

    #[test]
    fn rival_slide_left_runs_after_default_name() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::RivalNameChoice { cursor: 1 };
        let result = speech.update_frame(press_a());
        assert!(matches!(result, OakSpeechResult::RivalNameSet(ref n) if n == "GARY"));
        for _ in 0..SLIDE_PIC_LEFT_FRAMES {
            assert!(matches!(speech.phase, OakSpeechPhase::SlidePic { .. }));
            speech.update_frame(OakSpeechInput::none());
        }
        assert!(matches!(speech.phase, OakSpeechPhase::FinalSpeech { .. }));
    }

    #[test]
    fn slide_pic_x_matches_asm() {
        // Slide right: one column per Delay3 iteration, 48px → 96px.
        assert_eq!(slide_pic_x(PicSlideDirection::Right, 0), 56);
        assert_eq!(slide_pic_x(PicSlideDirection::Right, 2), 56);
        assert_eq!(slide_pic_x(PicSlideDirection::Right, 3), 64);
        assert_eq!(slide_pic_x(PicSlideDirection::Right, 17), 96);
        // Slide left: holds at 96px through the 13-frame pre-delay, then back.
        assert_eq!(slide_pic_x(PicSlideDirection::Left, 0), 96);
        assert_eq!(slide_pic_x(PicSlideDirection::Left, 12), 96);
        assert_eq!(slide_pic_x(PicSlideDirection::Left, 13), 88);
        assert_eq!(slide_pic_x(PicSlideDirection::Left, 30), 48);
    }

    #[test]
    fn entrance_slide_offset_matches_move_pic_left() {
        // rWX = 119 - 8·frame; window left edge = rWX - 7.
        assert_eq!(entrance_slide_offset(0), 112);
        assert_eq!(entrance_slide_offset(7), 56);
        assert_eq!(entrance_slide_offset(14), 0);
        assert_eq!(entrance_slide_offset(15), 0);
    }

    #[test]
    fn shrink_player_beat_timing() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::ShrinkPlayer { frame: 0 };
        // 4 (red) + 4 (shrink1) + 20 (shrink2) + 50 (cleared) + 24 (fade) = 102.
        let mut frames = 0;
        while matches!(speech.phase, OakSpeechPhase::ShrinkPlayer { .. }) {
            frames += 1;
            speech.update_frame(OakSpeechInput::none());
        }
        assert_eq!(frames, SHRINK_TOTAL_FRAMES);
        assert!(matches!(speech.phase, OakSpeechPhase::Done));
    }

    #[test]
    fn shrink_player_input_does_not_skip() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::ShrinkPlayer { frame: 0 };
        for _ in 0..10 {
            speech.update_frame(press_a());
        }
        assert!(matches!(
            speech.phase,
            OakSpeechPhase::ShrinkPlayer { frame: 10 }
        ));
    }

    #[test]
    fn naming_screen_open_flashes_white() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::PlayerNameChoice { cursor: 3 }; // NEW NAME
        speech.update_frame(press_a());
        assert!(matches!(speech.phase, OakSpeechPhase::PlayerNaming));
        assert!(speech.is_flashing());
        // GBPalWhiteOutWithDelay3: 3 frames of white before the naming screen
        // accepts input.
        for _ in 0..NAMING_FLASH_FRAMES {
            let result = speech.update_naming_frame(
                NamingInput {
                    a: true,
                    ..NamingInput::none()
                },
                false,
            );
            assert_eq!(result, OakSpeechResult::Active);
            assert_eq!(speech.naming_screen.as_ref().unwrap().name(), "");
        }
        assert!(!speech.is_flashing());
        // Input reaches the naming screen after the flash.
        speech.update_naming_frame(
            NamingInput {
                a: true,
                ..NamingInput::none()
            },
            false,
        );
        assert_eq!(speech.naming_screen.as_ref().unwrap().name(), "A");
    }

    #[test]
    fn naming_screen_submit_flashes_white() {
        let mut speech = OakSpeechState::new();
        speech.phase = OakSpeechPhase::PlayerNameChoice { cursor: 3 };
        speech.update_frame(press_a());
        for _ in 0..NAMING_FLASH_FRAMES {
            speech.update_naming_frame(NamingInput::none(), false);
        }
        // Type one letter, then submit with Start.
        speech.update_naming_frame(
            NamingInput {
                a: true,
                ..NamingInput::none()
            },
            false,
        );
        let result = speech.update_naming_frame(
            NamingInput {
                start: true,
                ..NamingInput::none()
            },
            false,
        );
        assert!(matches!(result, OakSpeechResult::PlayerNameSet(ref n) if n == "A"));
        assert!(speech.is_flashing());
        // The exit flash blocks the underlying state machine for 3 frames.
        for _ in 0..NAMING_FLASH_FRAMES {
            let result = speech.update_frame(press_a());
            assert_eq!(result, OakSpeechResult::Active);
        }
        assert!(!speech.is_flashing());
    }
}
