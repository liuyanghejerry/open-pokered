//! FLASH PrintText retains its party parent. Map restoration timing remains
//! on the existing overworld path, independently of this notice owner.
use super::*;
use pokered_core::overworld::party_field_text::PartyFieldText;

pub(crate) struct PartyFlashText {
    text: PartyFieldText,
    party: FrameBuffer,
}
impl PokemonGame {
    pub(super) fn begin_party_flash_text(&mut self, message: &str) {
        let mut party = FrameBuffer::new(RenderConfig::new(160, 144), Rgba::WHITE);
        crate::render::draw_party_screen(
            &self.party_screen,
            self.resources.as_mut(),
            self.frame_count,
            &mut party,
            self.state.config.language,
        );
        let text = self.overworld.begin_party_field_text(message);
        self.party_flash_text = Some(Box::new(PartyFlashText { text, party }));
    }
    pub(super) fn update_party_flash_text(&mut self, input: &InputState) {
        let Some(mut flow) = self.party_flash_text.take() else {
            return;
        };
        let raw = [
            GbButton::A,
            GbButton::B,
            GbButton::Select,
            GbButton::Start,
            GbButton::Right,
            GbButton::Left,
            GbButton::Up,
            GbButton::Down,
        ]
        .iter()
        .enumerate()
        .fold(0, |bits, (i, &key)| {
            bits | (u8::from(input.is_held(key)) << i)
        });
        let returned = self.overworld.tick_party_field_text(&mut flow.text, raw);
        if self.overworld.sfx_event == OverworldSfxEvent::TextAdvance {
            if let Some(audio) = &self.audio {
                audio.play_sfx(SfxId::PressAB);
            }
        }
        if returned {
            self.overworld.clear_retained_field_text();
            // Continue the existing whiteout only after PrintText returns.
            // The full CloseTextDisplay/loader timing is a separate audit.
            self.overworld.flash_lit_frames =
                pokered_core::overworld::presentation::FLASH_WHITE_FRAMES;
            self.handle_transition(GameScreen::Overworld);
        } else {
            self.party_flash_text = Some(flow);
        }
    }
    pub(super) fn draw_party_flash_text(&self, fb: &mut FrameBuffer) {
        let flow = self.party_flash_text.as_ref().unwrap();
        fb.copy_from(&flow.party);
        if let Some(dialogue) = self.overworld.displayed_field_dialogue() {
            if let Some((mut text, second)) = dialogue.get_display_text() {
                if !second.is_empty() {
                    text.push('\n');
                    text.push_str(&second);
                }
                let arrow = dialogue.waiting_for_input() && (self.frame_count / 16) % 2 == 0;
                let mut painter = pokered_ui::backends::FrameBufferPainter::new(fb);
                pokered_ui::menus::dialog::draw_paginated(
                    &text,
                    arrow,
                    &pokered_data::ui_layout::schema::DIALOG_DEFAULT_LAYOUT,
                    &mut pokered_ui::Ui::new(&mut painter),
                    self.state.config.language,
                );
            }
        }
    }
}
