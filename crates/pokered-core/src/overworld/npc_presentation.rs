//! Original sprite image updates and shadow-OAM and hardware-OAM publication.
use super::{npc_movement::NpcRuntimeState, Direction};
use crate::alloc_prelude::*;
use pokered_data::maps::MapId;

fn image_visible_default() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NpcImage {
    pub facing: Direction,
    pub phase: u8,
    pub scripted_frame: Option<u8>,
    #[serde(default = "image_visible_default")]
    pub visible: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NpcScreenPose {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub image: NpcImage,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NpcPresentation {
    map: Option<MapId>,
    images: Vec<NpcImage>,
    poses: Vec<NpcScreenPose>,
    shadow_poses: Vec<NpcScreenPose>,
    player_walking: bool,
    #[serde(default)]
    scan_player: (u16, u16),
    #[serde(default)]
    scan_camera: (i32, i32),
    #[serde(default)]
    origin_adjustments: Vec<(i32, i32)>,
    #[serde(default)]
    follow_animation: Vec<(u8, u8)>,
}
impl NpcPresentation {
    fn image(n: &NpcRuntimeState) -> NpcImage {
        NpcImage {
            facing: n.facing,
            phase: super::presentation::npc_walk_anim_phase(n.walk_counter),
            scripted_frame: n.scripted_frame,
            visible: n.visible,
        }
    }
    pub fn publish(
        &mut self,
        map: MapId,
        npcs: &[NpcRuntimeState],
        player: (u16, u16),
        camera: (i32, i32),
        player_walking: bool,
    ) {
        if self.map != Some(map) || self.images.len() != npcs.len() {
            self.map = Some(map);
            self.images = npcs
                .iter()
                .map(|n| {
                    let mut image = Self::image(n);
                    image.visible &= Self::coordinate_available(n, player);
                    image
                })
                .collect();
            self.origin_adjustments = vec![(0, 0); npcs.len()];
            self.follow_animation = vec![(0, 0); npcs.len()];
            self.poses.clear();
            self.shadow_poses.clear();
        }
        self.player_walking = player_walking;
        self.scan_player = player;
        self.scan_camera = camera;
        core::mem::swap(&mut self.poses, &mut self.shadow_poses);
        self.shadow_poses.clear();
        if self.origin_adjustments.len() != npcs.len() {
            self.origin_adjustments = vec![(0, 0); npcs.len()];
        }
        for (slot, (n, image)) in npcs.iter().zip(&self.images).enumerate() {
            let px = super::presentation::npc_walk_pixel_offset(n.walk_counter);
            let (dx, dy) = match n.walking_direction() {
                Direction::Down => (0, px),
                Direction::Up => (0, -px),
                Direction::Left => (-px, 0),
                Direction::Right => (px, 0),
            };
            self.shadow_poses.push(NpcScreenPose {
                x: 64 + (i32::from(n.x) - i32::from(player.0)) * 16 + dx - camera.0
                    + self.origin_adjustments[slot].0,
                y: 60 + (i32::from(n.y) - i32::from(player.1)) * 16 + dy - camera.1
                    + self.origin_adjustments[slot].1,
                visible: n.visible && image.visible,
                image: *image,
            });
        }
        if self.poses.is_empty() {
            self.poses.clone_from(&self.shadow_poses);
        }
    }
    /// CheckSpriteAvailability updates the image before the walking counter;
    /// while the player walks it leaves the previous image in place.
    fn axis_available(player: u8, sprite: u8, span: u8) -> bool {
        sprite == player || (sprite > player && sprite <= player.wrapping_add(span))
    }
    fn coordinate_available(n: &NpcRuntimeState, player: (u16, u16)) -> bool {
        if n.movement_type == super::NpcMovementType::FixedPath || !n.scripted_path.is_empty() {
            return true;
        }
        let (dx, dy) = if n.walk_counter > 0 {
            match n.walking_direction() {
                Direction::Down => (0, 1),
                Direction::Up => (0, -1),
                Direction::Left => (-1, 0),
                Direction::Right => (1, 0),
            }
        } else {
            (0, 0)
        };
        let x = (n.x as u8).wrapping_add(4).wrapping_add(dx as u8);
        let y = (n.y as u8).wrapping_add(4).wrapping_add(dy as u8);
        Self::axis_available(player.0 as u8, x, 9) && Self::axis_available(player.1 as u8, y, 8)
    }
    pub fn initialize_script_position(&mut self, slot: usize, n: &NpcRuntimeState) {
        let x = (n.x as u8)
            .wrapping_add(4)
            .wrapping_sub(self.scan_player.0 as u8)
            .rotate_left(4);
        let y = (n.y as u8)
            .wrapping_add(4)
            .wrapping_sub(self.scan_player.1 as u8)
            .rotate_left(4)
            .wrapping_sub(4);
        let px = i32::from(x.wrapping_add(8)) - 8;
        let py = i32::from(y.wrapping_add(16)) - 16;
        if let Some(adjustment) = self.origin_adjustments.get_mut(slot) {
            *adjustment = (
                px - (64 + (i32::from(n.x) - i32::from(self.scan_player.0)) * 16
                    - self.scan_camera.0),
                py - (60 + (i32::from(n.y) - i32::from(self.scan_player.1)) * 16
                    - self.scan_camera.1),
            );
        }
    }
    pub fn hold_wander_roll(&self, n: &NpcRuntimeState) -> bool {
        self.player_walking && n.walk_counter == 0 && n.delay_counter == 0
    }
    pub fn step_within_screen(&self, n: &NpcRuntimeState) -> bool {
        let (dx, dy) = match n.walking_direction() {
            Direction::Down => (0, 1),
            Direction::Up => (0, -1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        };
        let x =
            (64 + (i32::from(n.x) - i32::from(self.scan_player.0)) * 16 - self.scan_camera.0) as u8;
        let y =
            (60 + (i32::from(n.y) - i32::from(self.scan_player.1)) * 16 - self.scan_camera.1) as u8;
        y.wrapping_add(4).wrapping_add(dy as u8) < 128 && x.wrapping_add(dx as u8) < 144
    }
    pub fn started_wander_image(&mut self, slot: usize, n: &NpcRuntimeState) {
        if let Some(image) = self.images.get_mut(slot) {
            *image = Self::image(n);
        }
    }
    pub fn available(&self, n: &NpcRuntimeState) -> bool {
        n.visible && Self::coordinate_available(n, self.scan_player)
    }
    pub fn update_images_before_movement(&mut self, npcs: &[NpcRuntimeState]) {
        let player = self.scan_player;
        for (image, n) in self.images.iter_mut().zip(npcs) {
            if !n.visible || !Self::coordinate_available(n, player) {
                image.visible = false;
            } else if !self.player_walking || (n.walk_counter == 0 && n.delay_counter != 0) {
                // UpdateSpriteMovementDelay reaches NotYetMoving, which
                // refreshes its standing image even during a player step.
                *image = Self::image(n);
            }
        }
    }
    /// The synchronized movement routine animates even while the player
    /// walks. Its image clock continues across tile and direction changes.
    pub fn advance_follow_image(&mut self, slot: usize, n: &NpcRuntimeState) {
        self.follow_animation.resize(self.images.len(), (0, 0));
        if let (Some(clock), Some(image)) = (self.follow_animation.get_mut(slot), self.images.get_mut(slot)) {
            clock.0 += 1;
            if clock.0 == 4 {
                clock.0 = 0;
                clock.1 = (clock.1 + 1) & 3;
            }
            *image = NpcImage { facing: n.facing, phase: clock.1,
                scripted_frame: n.scripted_frame, visible: n.visible };
        }
    }
    pub fn refresh_font_images(&mut self, npcs: &[NpcRuntimeState]) {
        self.follow_animation.fill((0, 0));
        for (image, n) in self.images.iter_mut().zip(npcs) {
            *image = NpcImage {
                facing: n.facing,
                phase: 0,
                scripted_frame: n.scripted_frame,
                visible: image.visible && n.visible,
            };
        }
    }
    pub fn pose(&self, map: MapId, slot: usize) -> Option<NpcScreenPose> {
        (self.map == Some(map))
            .then(|| self.poses.get(slot).copied())
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stationary_images_and_delays_match_original_update_sequences() {
        let oracle: serde_json::Value = serde_json::from_str(include_str!("stationary_sequence_golden.json")).unwrap();
        for case in oracle["cases"].as_array().unwrap() {
            let mut n=human();n.movement_type=super::super::NpcMovementType::Stationary;
            n.range=255;n.walk_counter=0;n.delay_counter=0;n.facing=Direction::Up;
            let mut p=NpcPresentation::default();
            p.publish(MapId::ViridianCity,&[n.clone()],(20,30),(0,0),false);
            for (i,row) in case["updates"].as_array().unwrap().iter().enumerate() {
                let moving=row[1].as_bool().unwrap();
                p.publish(MapId::ViridianCity,&[n.clone()],(20,30),(0,0),moving);
                p.update_images_before_movement(&[n.clone()]);
                super::super::npc_movement::advance_stationary_facing(&mut n,moving,192,case["delay_random"].as_u64().unwrap() as u8);
                assert_eq!(n.delay_counter as u8,row[3].as_u64().unwrap() as u8,"{} call {i}",case["name"]);
                assert_eq!(n.delay_counter==0,row[2].as_u64().unwrap()==1,"ready state at call {i}");
                let facing=match p.images[0].facing {Direction::Down=>0,Direction::Up=>4,Direction::Left=>8,Direction::Right=>12};
                assert_eq!(32+facing,row[5].as_u64().unwrap(),"{} image at call {i}",case["name"]);
            }
        }
    }
    #[test]
    fn synchronized_image_clock_matches_natural_retail_escort() {
        let reference: serde_json::Value = serde_json::from_str(include_str!("follow_image_golden.json")).unwrap();
        let mut n = human();
        let mut p = NpcPresentation::default();
        p.publish(MapId::ViridianCity, &[n.clone()], (20, 30), (0, 0), true);
        for (i, row) in reference["frames"].as_array().unwrap().iter().enumerate() {
            n.facing = match row["facing"].as_u64().unwrap() {
                0 => Direction::Down, 4 => Direction::Up,
                8 => Direction::Left, 12 => Direction::Right, _ => unreachable!(),
            };
            p.update_images_before_movement(&[n.clone()]);
            p.advance_follow_image(0, &n);
            assert_eq!(p.images[0].phase, row["phase"].as_u64().unwrap() as u8, "call {i}");
            assert_eq!(p.images[0].facing, n.facing, "call {i}");
            assert_eq!(p.follow_animation[0].0, row["intra"].as_u64().unwrap() as u8, "call {i}");
            if i == 60 {
                p = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
            }
        }
    }
    fn human() -> NpcRuntimeState {
        let s = super::super::OverworldScreen::new(
            MapId::ViridianCity,
            None,
            pokered_data::impl_traits::PokemonRedData,
        );
        let mut n = s.npc_states[0].clone();
        n.x = 22;
        n.y = 30;
        n.walk_counter = 16;
        n.scripted_frame = None;
        n
    }
    #[test]
    fn turned_sprite_keeps_its_original_physical_offset() {
        let mut n = human();
        n.x = 10;
        n.y = 6;
        n.walk_counter = 8;
        n.facing = Direction::Right;
        n.step_direction = Some(Direction::Left);
        let mut p = NpcPresentation::default();
        p.publish(MapId::MtMoonPokecenter, core::slice::from_ref(&n),
            (11, 6), (0, 0), false);
        assert_eq!(p.poses[0].x, 40, "eight pixels left of the origin tile");
        assert_eq!(p.poses[0].y, 60);
        assert_eq!(p.poses[0].image.facing, Direction::Right);
    }

    #[test]
    fn scripted_stop_preserves_original_negative_coordinate_swap_glitch() {
        let mut n = human();
        n.x = 22;
        n.y = 25;
        n.walk_counter = 0;
        n.movement_type = super::super::NpcMovementType::FixedPath;
        let mut s = NpcPresentation::default();
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            false,
        );
        s.initialize_script_position(0, &n);
        for _ in 0..2 {
            s.publish(
                MapId::ViridianCity,
                core::slice::from_ref(&n),
                (20, 30),
                (0, 0),
                false,
            );
        }
        // audit607: mapY29-player30 -> SWAP($ff) -> $ff-4=$fb;
        // OAM Y adds16 and wraps to11, so the visible top is -5.
        assert_eq!(s.pose(MapId::ViridianCity, 0).unwrap().y, -5);
        assert_eq!(s.pose(MapId::ViridianCity, 0).unwrap().x, 96);
    }
    #[test]
    fn wander_start_bounds_match_eight_original_routine_probes() {
        // audit599: unmodified original CanWalkOntoTile, passable tile and
        // collision-free controlled inputs; inside/edge for each direction.
        for (x, y, dir, allowed) in [
            (17, 30, Direction::Left, true),
            (16, 30, Direction::Left, false),
            (24, 30, Direction::Right, true),
            (25, 30, Direction::Right, false),
            (20, 27, Direction::Up, true),
            (20, 26, Direction::Up, false),
            (20, 33, Direction::Down, true),
            (20, 34, Direction::Down, false),
        ] {
            let mut n = human();
            n.x = x;
            n.y = y;
            n.facing = dir;
            n.walk_counter = 0;
            let mut s = NpcPresentation::default();
            s.publish(
                MapId::ViridianCity,
                core::slice::from_ref(&n),
                (20, 30),
                (0, 0),
                false,
            );
            assert_eq!(s.step_within_screen(&n), allowed, "{dir:?} ({x},{y})");
        }
    }
    #[test]
    fn walking_player_holds_only_ready_wander_rolls() {
        let mut n = human();
        n.walk_counter = 0;
        let mut s = NpcPresentation::default();
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            true,
        );
        n.delay_counter = 0;
        assert!(s.hold_wander_roll(&n));
        n.delay_counter = 1;
        assert!(
            !s.hold_wander_roll(&n),
            "existing movement delay still advances"
        );
        n.delay_counter = 0;
        n.walk_counter = 8;
        assert!(!s.hold_wander_roll(&n), "existing walk still advances");
    }
    #[test]
    fn original_edge_visibility_uses_the_walking_destination() {
        let mut n = human();
        n.facing = Direction::Right;
        n.walk_counter = 4;
        assert!(NpcPresentation::coordinate_available(&n, (18, 30)));
        assert!(
            !NpcPresentation::coordinate_available(&n, (17, 30)),
            "audit595: targetMapX27 exceeds player17+9, source freezes counter4"
        );
        n.movement_type = super::super::NpcMovementType::FixedPath;
        assert!(
            NpcPresentation::coordinate_available(&n, (17, 30)),
            "scripted movement bypasses the map-coordinate visibility tests"
        );
    }
    #[test]
    fn hidden_image_is_restored_only_at_an_idle_player_sprite_scan() {
        let mut n = human();
        n.facing = Direction::Right;
        n.walk_counter = 4;
        let mut s = NpcPresentation::default();
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (18, 30),
            (0, 0),
            true,
        );
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (17, 30),
            (0, 0),
            true,
        );
        s.update_images_before_movement(core::slice::from_ref(&n));
        assert!(!s.images[0].visible);
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (18, 30),
            (0, 0),
            true,
        );
        s.update_images_before_movement(core::slice::from_ref(&n));
        assert!(
            !s.images[0].visible,
            "CheckSpriteAvailability does not update image while wWalkCounter is nonzero"
        );
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (18, 30),
            (0, 0),
            false,
        );
        s.update_images_before_movement(core::slice::from_ref(&n));
        assert!(s.images[0].visible);
        assert_eq!(s.images[0].phase, 3);
    }
    #[test]
    fn visibility_preserves_the_original_unsigned_byte_wrap() {
        assert!(NpcPresentation::axis_available(250, 250, 9));
        assert!(!NpcPresentation::axis_available(250, 251, 9));
        assert!(!NpcPresentation::axis_available(250, 3, 9));
    }
    #[test]
    fn two_stage_publication_matches_complete_original_rendered_walks() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("npc_presentation_golden.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let (direction, dx, dy) = match case["direction"].as_str().unwrap() {
                "down" => (Direction::Down, 0, 1),
                "up" => (Direction::Up, 0, -1),
                "left" => (Direction::Left, -1, 0),
                _ => (Direction::Right, 1, 0),
            };
            let mut n = human();
            n.facing = direction;
            let mut state = NpcPresentation::default();
            for (t, expected) in case["frames"].as_array().unwrap().iter().enumerate() {
                state.publish(
                    MapId::ViridianCity,
                    core::slice::from_ref(&n),
                    (20, 30),
                    (0, 0),
                    false,
                );
                let p = state.pose(MapId::ViridianCity, 0).unwrap();
                assert_eq!(
                    (p.x, p.y),
                    (
                        expected["x"].as_i64().unwrap() as i32,
                        expected["y"].as_i64().unwrap() as i32
                    ),
                    "{direction:?} t{t}"
                );
                assert_eq!(
                    p.image.phase & 1 != 0,
                    expected["walking"].as_bool().unwrap(),
                    "{direction:?} t{t}"
                );
                let flip = match direction {
                    Direction::Right => true,
                    Direction::Left => false,
                    _ => p.image.phase == 3,
                };
                assert_eq!(
                    flip,
                    expected["flip"].as_bool().unwrap(),
                    "{direction:?} t{t}"
                );
                if t % 2 == 0 {
                    state.update_images_before_movement(core::slice::from_ref(&n));
                    if n.walk_counter > 0 {
                        n.walk_counter -= 1;
                        if n.walk_counter == 0 {
                            n.x = (i32::from(n.x) + dx) as u16;
                            n.y = (i32::from(n.y) + dy) as u16;
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn map_reload_discards_previous_sprite_publication() {
        let n = human();
        let mut s = NpcPresentation::default();
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            false,
        );
        assert!(s.pose(MapId::PalletTown, 0).is_none());
        s.publish(
            MapId::PalletTown,
            core::slice::from_ref(&n),
            (22, 30),
            (0, 0),
            false,
        );
        assert_eq!(s.pose(MapId::PalletTown, 0).unwrap().x, 64);
    }
    #[test]
    fn snapshot_roundtrip_preserves_both_pending_publication_stages() {
        let mut n = human();
        let mut a = NpcPresentation::default();
        a.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            false,
        );
        n.walk_counter = 15;
        a.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            false,
        );
        let mut b: NpcPresentation =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        n.walk_counter = 14;
        a.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            false,
        );
        b.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            false,
        );
        assert_eq!(a, b);
    }
    #[test]
    fn walking_player_preserves_image_and_font_reload_restores_facing() {
        let mut n = human();
        let mut s = NpcPresentation::default();
        s.publish(
            MapId::ViridianCity,
            core::slice::from_ref(&n),
            (20, 30),
            (0, 0),
            true,
        );
        n.facing = Direction::Left;
        n.walk_counter = 12;
        s.update_images_before_movement(core::slice::from_ref(&n));
        assert_eq!(s.images[0].phase, 0);
        assert_ne!(s.images[0].facing, Direction::Left);
        s.refresh_font_images(core::slice::from_ref(&n));
        assert_eq!(s.images[0].facing, Direction::Left);
        assert_eq!(s.images[0].phase, 0);
    }
}


/// SCX/SCY are copied at VBlank, before foreground advancement. The initial
/// LoadCurrentMapView crosses that VBlank before it writes the first scroll.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FieldViewport {
    map: Option<MapId>,
    scroll: (i32, i32),
    displayed: (i32, i32),
}
impl FieldViewport {
    pub fn begin(&mut self, map: MapId, active: bool, current: (i32, i32)) {
        if !active {
            self.map = None;
            return;
        }
        if self.map != Some(map) {
            self.map = Some(map);
            self.scroll = current;
        }
        self.displayed = self.scroll;
    }
    pub fn finish(&mut self, map: MapId, current: (i32, i32), redraw_crosses_vblank: bool) {
        if self.map == Some(map) && !redraw_crosses_vblank {
            self.scroll = current;
        }
    }
    pub fn residual(&self, map: MapId, player: (u16, u16)) -> Option<(i32, i32)> {
        (self.map == Some(map)).then(|| {
            (
                self.displayed.0 - i32::from(player.0) * 16,
                self.displayed.1 - i32::from(player.1) * 16,
            )
        })
    }
}

#[cfg(test)]
mod viewport_tests {
    use super::*;
    #[test]
    fn field_scroll_publication_matches_two_complete_original_steps() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("field_viewport_golden.json")).unwrap();
        let mut s = FieldViewport::default();
        for (t, row) in golden["rows"].as_array().unwrap().iter().enumerate() {
            let by = row[0].as_i64().unwrap() as u16;
            let bc = row[1].as_i64().unwrap() as u8;
            let bw = row[2].as_bool().unwrap();
            let ay = row[3].as_i64().unwrap() as u16;
            let ac = row[4].as_i64().unwrap() as u8;
            let aw = row[5].as_bool().unwrap();
            s.begin(
                MapId::ViridianCity,
                true,
                (
                    320,
                    i32::from(by) * 16 + if bw { i32::from(8 - bc) * 2 } else { 0 },
                ),
            );
            assert_eq!(
                s.displayed.1,
                row[6].as_i64().unwrap() as i32,
                "raw frame {t}"
            );
            s.finish(
                MapId::ViridianCity,
                (
                    320,
                    i32::from(ay) * 16 + if aw { i32::from(8 - ac) * 2 } else { 0 },
                ),
                !bw && aw,
            );
        }
    }
    #[test]
    fn snapshot_preserves_pending_scroll_and_map_change_resets_it() {
        let mut a = FieldViewport::default();
        a.begin(MapId::ViridianCity, true, (320, 480));
        a.finish(MapId::ViridianCity, (320, 482), false);
        let mut b: FieldViewport =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        a.begin(MapId::ViridianCity, true, (320, 482));
        b.begin(MapId::ViridianCity, true, (320, 482));
        assert_eq!(a, b);
        b.begin(MapId::PalletTown, true, (80, 96));
        assert_eq!(b.displayed, (80, 96));
        assert!(b.residual(MapId::ViridianCity, (20, 30)).is_none());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerImage {
    pub facing: Direction,
    pub phase: u8,
}
/// Player animation advances every UpdatePlayerSprite, independent of tile
/// distance. Its image persists across held-direction steps and OAM stages.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerPresentation {
    map: Option<MapId>,
    intra: u8,
    phase: u8,
    current: Option<PlayerImage>,
    shadow: Option<PlayerImage>,
    displayed: Option<PlayerImage>,
    was_moving: bool,
}
impl PlayerPresentation {
    pub fn publish(&mut self, map: MapId, facing: Direction, moving: bool, font_loaded: bool) {
        if self.map != Some(map) {
            *self = Self::default();
            self.map = Some(map);
            let image = Some(PlayerImage { facing, phase: 0 });
            self.current = image;
            self.shadow = image;
        }
        if font_loaded {
            self.intra = 0;
            self.phase = 0;
            self.current = Some(PlayerImage { facing, phase: 0 });
        }
        self.was_moving = moving;
        self.displayed = self.shadow;
        self.shadow = self.current;
    }
    pub fn tick(&mut self, facing: Direction, moving: bool) {
        if moving || self.was_moving {
            self.intra += 1;
            if self.intra == 4 {
                self.intra = 0;
                self.phase = (self.phase + 1) & 3;
            }
        } else {
            self.intra = 0;
            self.phase = 0;
        }
        self.current = Some(PlayerImage {
            facing,
            phase: self.phase,
        });
    }
    pub fn image(&self, map: MapId) -> Option<PlayerImage> {
        (self.map == Some(map)).then_some(self.displayed).flatten()
    }
}

#[cfg(test)]
mod player_tests {
    use super::*;
    #[test]
    fn player_animation_and_publication_match_original_across_two_steps() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("player_presentation_golden.json")).unwrap();
        let mut s = PlayerPresentation::default();
        let mut tick = 0;
        for (t, expected) in golden["images"].as_array().unwrap().iter().enumerate() {
            s.publish(MapId::ViridianCity, Direction::Down, false, false);
            let image = s.image(MapId::ViridianCity).unwrap();
            assert_eq!(
                image.phase & 1 != 0,
                expected[0].as_bool().unwrap(),
                "raw frame {t}"
            );
            assert_eq!(
                image.phase == 3,
                expected[1].as_bool().unwrap(),
                "raw frame {t}"
            );
            if let Some(row) = golden["ticks"].as_array().unwrap().get(tick) {
                if row[0].as_u64().unwrap() == t as u64 {
                    s.tick(
                        Direction::Down,
                        row[1].as_u64().unwrap() != 0 || row[2].as_u64().unwrap() != 0,
                    );
                    assert_eq!(s.intra, row[3].as_u64().unwrap() as u8, "tick {tick}");
                    assert_eq!(s.phase, row[4].as_u64().unwrap() as u8, "tick {tick}");
                    tick += 1;
                }
            }
        }
        assert_eq!(tick, golden["ticks"].as_array().unwrap().len());
    }
    #[test]
    fn font_restores_standing_despite_the_previous_held_direction() {
        let mut s = PlayerPresentation::default();
        s.publish(MapId::ViridianCity, Direction::Down, true, false);
        for _ in 0..4 {
            s.tick(Direction::Down, true);
        }
        assert_eq!(s.phase, 1);
        s.publish(MapId::ViridianCity, Direction::Down, true, true);
        assert_eq!((s.intra, s.phase), (0, 0));
        assert_eq!(s.current.unwrap().phase, 0);
    }
    #[test]
    fn snapshot_preserves_animation_counters_and_both_sprite_images() {
        let mut a = PlayerPresentation::default();
        a.publish(MapId::ViridianCity, Direction::Up, false, false);
        for _ in 0..5 {
            a.tick(Direction::Up, true);
            a.publish(MapId::ViridianCity, Direction::Up, true, false);
        }
        let mut b: PlayerPresentation =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        a.tick(Direction::Up, true);
        b.tick(Direction::Up, true);
        a.publish(MapId::ViridianCity, Direction::Up, true, false);
        b.publish(MapId::ViridianCity, Direction::Up, true, false);
        assert_eq!(a, b);
        b.publish(MapId::PalletTown, Direction::Left, false, false);
        assert_eq!(
            b.image(MapId::PalletTown),
            Some(PlayerImage {
                facing: Direction::Left,
                phase: 0
            })
        );
        assert!(b.image(MapId::ViridianCity).is_none());
    }
}
