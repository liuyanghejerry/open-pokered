//! Live sprite inputs for the field restore caller. This observes native
//! current images/registers; it neither prepares OAM nor accepts an interrupt.
//! Enable/movement flags and complete IRQ ownership remain separate inputs.
use super::*;
use pokered_core::overworld::{field_restore::SpriteSlot, prepare_oam_work::OamSprite, MovementState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) enum FieldOamInputError {
    MissingPlayerPublication,
    MissingNpcPublication(usize),
    MissingPatternSlot(usize),
    InvalidPatternSlot(usize),
    UnsupportedScriptedFrame(usize),
    GrassPriorityNotTracked,
    UnsupportedMovement,
    TooManySprites,
    UnknownSpriteControl,
}
fn facing(facing: pokered_core::overworld::Direction) -> u8 {
    use pokered_core::overworld::Direction::*;
    match facing {
        Down => 0,
        Up => 4,
        Left => 8,
        Right => 12,
    }
}

/// Full stationary field input, retaining the actual native door/warp bits.
/// The existing actor producer still rejects unsupported movement/grass.
pub(super) fn field_oam_input(field: &OverworldScreen,
    time: &pokered_core::overworld::display_work_time::DisplayWorkTime,
    slots: &[SpriteSlot]) -> Result<pokered_core::overworld::prepare_oam_work::PrepareOamState, FieldOamInputError> {
    let enabled = time.sprite_updates_enabled().ok_or(FieldOamInputError::UnknownSpriteControl)?;
    let movement_flags = u8::from(field.state.standing_on_door)
        | (u8::from(field.state.exiting_door) << 1)
        | (u8::from(field.state.standing_on_warp) << 2);
    Ok(pokered_core::overworld::prepare_oam_work::PrepareOamState {
        enabled, movement_flags, sprites: field_oam_sprites(field,slots)?,
    })
}

pub(super) fn field_oam_sprites(
    field: &OverworldScreen,
    slots: &[SpriteSlot],
) -> Result<Vec<OamSprite>, FieldOamInputError> {
    use FieldOamInputError::*;
    if field.npc_states.len() > 15 {
        return Err(TooManySprites);
    }
    if field.ledge_jump.is_some()
        || field.fishing_anim.is_some()
        || field.field_move_step.is_some()
        || field.teleport_spin.is_some()
        || field.active_script_effect_label().is_some()
        || field.state.player.movement_state != MovementState::Idle
    {
        return Err(UnsupportedMovement);
    }
    // No invented zero priority for grass maps. Their retained foreground
    // grass-priority register still needs an actual native producer.
    let map = field.map_data.as_ref().ok_or(GrassPriorityNotTracked)?;
    let grass_requires_register = pokered_data::tileset_data::get_tileset_header(map.tileset).grass_tile.is_some();
    let player = field
        .player_presentation
        .current_oam_image(field.state.current_map)
        .ok_or(MissingPlayerPublication)?;
    // A genuinely suppressed image never reaches the OAM attribute branch.
    // Its unread grass byte remains opaque; do not invent a visible zero-
    // priority sprite. Visible actors still require a real native register.
    if player.is_some() && grass_requires_register { return Err(GrassPriorityNotTracked); }

    let mut sprites = vec![
        OamSprite {
            picture: 0,
            image: 0xff,
            x: 0,
            y: 0,
            adjusted_x: 0,
            adjusted_y: 0,
            grass: 0
        };
        16
    ];
    // ResetPlayerSpriteData and native player rendering share the same fixed
    // standing origin. Active movement/ledge/fishing owners were rejected.
    sprites[0] = OamSprite {
        picture: 1,
        image: player.map_or(0xff, |i| facing(i.facing) + i.phase),
        x: 64,
        y: 60,
        adjusted_x: 64,
        adjusted_y: 64,
        grass: 0,
    };
    for (i, npc) in field.npc_states.iter().enumerate() {
        let (image, pixels) = field
            .npc_presentation
            .current_oam_input(field.state.current_map, i)
            .ok_or(MissingNpcPublication(i))?;
        let index = if npc.sprite_id == 0 || !image.visible || !npc.visible {
            0xff
        } else {
            if grass_requires_register { return Err(GrassPriorityNotTracked); }
            let slot = slots
                .iter()
                .find(|slot| slot.picture == npc.sprite_id)
                .ok_or(MissingPatternSlot(i))?
                .vram_slot;
            if !(2..=12).contains(&slot) {
                return Err(InvalidPatternSlot(i));
            }
            let low = if let Some(frame) = image.scripted_frame {
                match frame {
                    0 => 0,
                    1 => 4,
                    2 => 8,
                    3 => 1,
                    4 => 5,
                    5 => 9,
                    _ => return Err(UnsupportedScriptedFrame(i)),
                }
            } else {
                facing(image.facing) + image.phase
            };
            ((slot - 1) << 4) | low
        };
        sprites[i + 1] = OamSprite {
            picture: npc.sprite_id,
            image: index,
            x: pixels.x,
            y: pixels.y,
            adjusted_x: pixels.adjusted_x,
            adjusted_y: pixels.adjusted_y,
            grass: 0,
        };
    }
    Ok(sprites)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pokered_data::{impl_traits::PokemonRedData, maps::MapId};
    #[test]
    fn genuinely_suppressed_grass_images_skip_unread_priority_but_visible_images_still_refuse_unknown() {
        let mut field = OverworldScreen::new(MapId::PalletTown, None, PokemonRedData);
        field.publish_npc_frame(false);
        field.player_presentation.suppress_current_image();
        field.npc_presentation.suppress_current_images();
        let input = field_oam_sprites(&field, &[]).unwrap();
        assert!(input.iter().all(|sprite| sprite.image == 0xff));
        field.player_presentation.tick(pokered_core::overworld::Direction::Down, false);
        assert_eq!(field_oam_sprites(&field, &[]), Err(FieldOamInputError::GrassPriorityNotTracked));
    }
    fn field() -> OverworldScreen {
        let mut field = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        field.state.player.x = 15;
        field.state.player.y = 3;
        field.publish_npc_frame(false);
        field
    }
    #[test]
    fn full_input_uses_written_control_and_actual_door_warp_bits() {
        let mut field=field();let mut time=pokered_core::overworld::display_work_time::DisplayWorkTime::default();time.begin_native_update();
        assert_eq!(field_oam_input(&field,&time,&[]).unwrap().enabled,1);
        time.begin_legacy_party_sprite_hide().unwrap();assert_eq!(field_oam_input(&field,&time,&[]).unwrap().enabled,0);
        time.service_legacy_sprite_hide().unwrap();assert_eq!(field_oam_input(&field,&time,&[]).unwrap().enabled,255);
        field.state.standing_on_door=true;field.state.exiting_door=true;field.state.standing_on_warp=true;
        assert_eq!(field_oam_input(&field,&time,&[]).unwrap().movement_flags,7);
        let mut old=serde_json::to_value(&time).unwrap();old.as_object_mut().unwrap().remove("sprite_updates_enabled");let old=serde_json::from_value(old).unwrap();
        assert_eq!(field_oam_input(&field,&old,&[]),Err(FieldOamInputError::UnknownSpriteControl));
    }
    #[test]
    fn real_offscreen_map_objects_retain_raw_pictures_and_zero_pixel_registers() {
        let field = field();
        let sprites = field_oam_sprites(&field, &[]).unwrap();
        assert_eq!(sprites.len(), 16);
        assert_eq!(sprites[0].image, 0);
        for (npc, input) in field.npc_states.iter().zip(&sprites[1..]) {
            assert_eq!(input.picture, npc.sprite_id);
            assert_eq!(input.image, 0xff);
            assert_eq!(
                (input.x, input.y, input.adjusted_x, input.adjusted_y),
                (0, 0, 0, 0)
            );
        }
        assert!(sprites[field.npc_states.len() + 1..]
            .iter()
            .all(|s| s.picture == 0 && s.image == 0xff));
    }
    #[test]
    fn visible_objects_require_real_assigned_slots_and_current_suppression_is_not_delayed_display()
    {
        let mut field = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        field.state.player.x = 15;
        field.state.player.y = 3;
        field.npc_states[0].x = 15;
        field.npc_states[0].y = 3;
        field.npc_states[0].visible = true;
        field.publish_npc_frame(false);
        assert_eq!(
            field_oam_sprites(&field, &[]),
            Err(FieldOamInputError::MissingPatternSlot(0))
        );
        let slots = [SpriteSlot {
            picture: field.npc_states[0].sprite_id,
            vram_slot: 2,
        }];
        let visible = field_oam_sprites(&field, &slots).unwrap();
        assert!(visible[1].image < 0xff);
        field.player_presentation.suppress_current_image();
        field.npc_presentation.suppress_current_images();
        let hidden = field_oam_sprites(&field, &slots).unwrap();
        assert_eq!(hidden[0].image, 0xff);
        assert_eq!(hidden[1].image, 0xff);
        assert_eq!((hidden[1].x, hidden[1].y), (visible[1].x, visible[1].y));
        assert!(field
            .player_presentation
            .image(field.state.current_map)
            .is_some());
    }
    #[test]
    fn missing_publication_and_untracked_grass_do_not_get_guessed_input() {
        let uninitialized = OverworldScreen::new(MapId::RockTunnel1F, None, PokemonRedData);
        assert_eq!(
            field_oam_sprites(&uninitialized, &[]),
            Err(FieldOamInputError::MissingPlayerPublication)
        );
        let mut grass = OverworldScreen::new(MapId::ViridianForest, None, PokemonRedData);
        grass.publish_npc_frame(false);
        assert_eq!(
            field_oam_sprites(&grass, &[]),
            Err(FieldOamInputError::GrassPriorityNotTracked)
        );
    }
}
