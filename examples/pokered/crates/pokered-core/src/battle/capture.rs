use pokered_data::items::ItemId;

use super::state::StatusCondition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureResult {
    Captured,
    Failed { shakes: u8 },
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureRandoms {
    pub rand1: u8,
    pub rand2: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureContext {
    pub ball: ItemId,
    pub wild_max_hp: u16,
    pub wild_current_hp: u16,
    pub wild_catch_rate: u8,
    pub wild_status: StatusCondition,
}

fn status_subtract(status: &StatusCondition) -> u8 {
    match status {
        StatusCondition::Burn | StatusCondition::Paralysis | StatusCondition::Poison => 12,
        StatusCondition::Freeze | StatusCondition::Sleep(_) => 25,
        StatusCondition::None => 0,
    }
}

fn status_wobble_bonus(status: &StatusCondition) -> u8 {
    match status {
        StatusCondition::Burn | StatusCondition::Paralysis | StatusCondition::Poison => 5,
        StatusCondition::Freeze | StatusCondition::Sleep(_) => 10,
        StatusCondition::None => 0,
    }
}

fn rand1_threshold(ball: ItemId) -> u8 {
    match ball {
        ItemId::GreatBall => 200,
        ItemId::UltraBall | ItemId::SafariBall => 150,
        _ => 255,
    }
}

fn ball_factor_w(ball: ItemId) -> u16 {
    match ball {
        ItemId::GreatBall => 8,
        _ => 12,
    }
}

fn ball_factor_y(ball: ItemId) -> u16 {
    match ball {
        ItemId::PokeBall => 255,
        ItemId::GreatBall => 200,
        _ => 150,
    }
}

fn calculate_wobbles(catch_rate: u8, ball: ItemId, x: u8, status: &StatusCondition) -> u8 {
    let y = ((catch_rate as u16) * 100) / ball_factor_y(ball);
    let z = ((x as u16) * y) / 255 + status_wobble_bonus(status) as u16;
    if z < 10 {
        0
    } else if z < 30 {
        1
    } else if z < 70 {
        2
    } else {
        3
    }
}

pub fn try_capture(ctx: &CaptureContext, randoms: &CaptureRandoms) -> CaptureResult {
    if ctx.ball == ItemId::MasterBall {
        return CaptureResult::Captured;
    }

    let threshold = rand1_threshold(ctx.ball);
    if ctx.ball != ItemId::PokeBall && randoms.rand1 > threshold {
        return CaptureResult::Failed {
            shakes: calculate_wobbles(ctx.wild_catch_rate, ctx.ball, 0, &ctx.wild_status),
        };
    }

    let status_sub = status_subtract(&ctx.wild_status);
    if status_sub > randoms.rand1 {
        return CaptureResult::Captured;
    }

    let hp_quarter = std::cmp::max(ctx.wild_current_hp / 4, 1) as u16;
    let ball_f = ball_factor_w(ctx.ball);
    let max_hp_scaled = (ctx.wild_max_hp as u32) * 255;
    let w_raw = max_hp_scaled / (ball_f as u32) / (hp_quarter as u32);

    let adjusted_rand1 = randoms.rand1.saturating_sub(status_sub);

    if adjusted_rand1 > ctx.wild_catch_rate {
        let x = std::cmp::min(w_raw, 255) as u8;
        return CaptureResult::Failed {
            shakes: calculate_wobbles(ctx.wild_catch_rate, ctx.ball, x, &ctx.wild_status),
        };
    }

    if w_raw > 255 {
        return CaptureResult::Captured;
    }

    let x = w_raw as u8;
    if randoms.rand2 <= x {
        CaptureResult::Captured
    } else {
        CaptureResult::Failed {
            shakes: calculate_wobbles(ctx.wild_catch_rate, ctx.ball, x, &ctx.wild_status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poke_ball_ctx() -> CaptureContext {
        CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 50,
            wild_catch_rate: 190,
            wild_status: StatusCondition::None,
        }
    }

    #[test]
    fn master_ball_always_captures() {
        let ctx = CaptureContext {
            ball: ItemId::MasterBall,
            wild_max_hp: 500,
            wild_current_hp: 500,
            wild_catch_rate: 3,
            wild_status: StatusCondition::None,
        };
        let randoms = CaptureRandoms {
            rand1: 255,
            rand2: 255,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }

    #[test]
    fn great_ball_rand1_rejection() {
        let ctx = CaptureContext {
            ball: ItemId::GreatBall,
            wild_max_hp: 100,
            wild_current_hp: 100,
            wild_catch_rate: 45,
            wild_status: StatusCondition::None,
        };
        let randoms = CaptureRandoms {
            rand1: 201,
            rand2: 0,
        };
        let result = try_capture(&ctx, &randoms);
        assert!(matches!(result, CaptureResult::Failed { .. }));
    }

    #[test]
    fn great_ball_rand1_at_threshold_passes() {
        let ctx = CaptureContext {
            ball: ItemId::GreatBall,
            wild_max_hp: 100,
            wild_current_hp: 1,
            wild_catch_rate: 255,
            wild_status: StatusCondition::None,
        };
        // rand1=200 <= 200 threshold, passes first check
        // adjusted_rand1=200 > catch_rate=255? No, 200 <= 255
        // w_raw = (100*255)/8/1 = 3187 > 255 → captured
        let randoms = CaptureRandoms {
            rand1: 200,
            rand2: 0,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }

    #[test]
    fn ultra_ball_rand1_rejection() {
        let ctx = CaptureContext {
            ball: ItemId::UltraBall,
            wild_max_hp: 100,
            wild_current_hp: 100,
            wild_catch_rate: 45,
            wild_status: StatusCondition::None,
        };
        let randoms = CaptureRandoms {
            rand1: 151,
            rand2: 0,
        };
        let result = try_capture(&ctx, &randoms);
        assert!(matches!(result, CaptureResult::Failed { .. }));
    }

    #[test]
    fn status_immediate_capture_sleep() {
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 100,
            wild_catch_rate: 45,
            wild_status: StatusCondition::Sleep(3),
        };
        // status_sub = 25 > rand1 = 20 → immediate capture
        let randoms = CaptureRandoms {
            rand1: 20,
            rand2: 255,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }

    #[test]
    fn status_immediate_capture_burn() {
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 100,
            wild_catch_rate: 45,
            wild_status: StatusCondition::Burn,
        };
        // status_sub = 12 > rand1 = 5 → immediate capture
        let randoms = CaptureRandoms {
            rand1: 5,
            rand2: 255,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }

    #[test]
    fn low_hp_increases_capture_chance() {
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 200,
            wild_current_hp: 1,
            wild_catch_rate: 255,
            wild_status: StatusCondition::None,
        };
        // hp_quarter = max(1/4, 1) = 1
        // w_raw = (200 * 255) / 12 / 1 = 4250 > 255 → captured (if rand1 check passes)
        // rand1=0 <= catch_rate=255 → w > 255 → captured
        let randoms = CaptureRandoms {
            rand1: 0,
            rand2: 255,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }

    #[test]
    fn full_hp_hard_to_catch() {
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 200,
            wild_current_hp: 200,
            wild_catch_rate: 45,
            wild_status: StatusCondition::None,
        };
        // hp_quarter = 200/4 = 50
        // w_raw = (200 * 255) / 12 / 50 = 85
        // rand1=100, status_sub=0, adjusted_rand1=100 > catch_rate=45 → fail
        let randoms = CaptureRandoms {
            rand1: 100,
            rand2: 0,
        };
        let result = try_capture(&ctx, &randoms);
        assert!(matches!(result, CaptureResult::Failed { .. }));
    }

    #[test]
    fn rand2_check_captures() {
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 50,
            wild_catch_rate: 190,
            wild_status: StatusCondition::None,
        };
        // hp_quarter = 50/4 = 12
        // w_raw = (100 * 255) / 12 / 12 = 177  (not > 255)
        // rand1=0, adjusted_rand1=0 <= 190 → passes catch_rate check
        // w_raw=177, x=177. rand2=100 <= 177 → captured
        let randoms = CaptureRandoms {
            rand1: 0,
            rand2: 100,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }

    #[test]
    fn rand2_check_fails() {
        let ctx = poke_ball_ctx();
        // hp_quarter = 50/4 = 12
        // w_raw = (100 * 255) / 12 / 12 = 177
        // rand1=0, adjusted=0 <= 190 → passes
        // x=177, rand2=200 > 177 → fail
        let randoms = CaptureRandoms {
            rand1: 0,
            rand2: 200,
        };
        let result = try_capture(&ctx, &randoms);
        assert!(matches!(result, CaptureResult::Failed { .. }));
    }

    #[test]
    fn wobble_count_zero() {
        // catch_rate low, hp full, no status → Z < 10 → 0 shakes
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 200,
            wild_current_hp: 200,
            wild_catch_rate: 3,
            wild_status: StatusCondition::None,
        };
        // hp_quarter = 50. w_raw = (200*255)/12/50 = 85. x=85.
        // adjusted_rand1 = 100 > catch_rate=3 → fail
        // y = (3 * 100) / 255 = 1
        // z = (85 * 1) / 255 + 0 = 0
        // shakes = 0
        let randoms = CaptureRandoms {
            rand1: 100,
            rand2: 0,
        };
        assert_eq!(
            try_capture(&ctx, &randoms),
            CaptureResult::Failed { shakes: 0 }
        );
    }

    #[test]
    fn wobble_count_two_with_status() {
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 100,
            wild_catch_rate: 200,
            wild_status: StatusCondition::Freeze,
        };
        // status_sub=25, rand1=30. 25 > 30? No.
        // hp_quarter=25. w_raw=(100*255)/12/25=85. x=85.
        // adjusted_rand1=30-25=5 <= 200 → passes
        // w_raw=85 not > 255
        // rand2=200 > 85 → fail
        // y = (200*100)/150 = 133
        // z = (85*133)/255 + 10 = 44 + 10 = 54
        // 30 <= 54 < 70 → 2 shakes
        let randoms = CaptureRandoms {
            rand1: 30,
            rand2: 200,
        };
        assert_eq!(
            try_capture(&ctx, &randoms),
            CaptureResult::Failed { shakes: 2 }
        );
    }

    #[test]
    fn wobble_count_three_high_catch_rate_low_hp() {
        // PokeBall, catch_rate=254, current_hp=3 → hp_quarter=1
        // adjusted_rand1=255 > 254 → fail
        // x=min(2125, 255)=255
        // y=(254*100)/255=99
        // z=(255*99)/255+0=99 → 3 shakes (99 >= 70)
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 3,
            wild_catch_rate: 254,
            wild_status: StatusCondition::None,
        };
        let randoms = CaptureRandoms {
            rand1: 255,
            rand2: 0,
        };
        assert_eq!(
            try_capture(&ctx, &randoms),
            CaptureResult::Failed { shakes: 3 }
        );
    }

    #[test]
    fn safari_ball_uses_ultra_threshold() {
        let ctx = CaptureContext {
            ball: ItemId::SafariBall,
            wild_max_hp: 100,
            wild_current_hp: 100,
            wild_catch_rate: 45,
            wild_status: StatusCondition::None,
        };
        // rand1=151 > 150 threshold → rejected
        let randoms = CaptureRandoms {
            rand1: 151,
            rand2: 0,
        };
        let result = try_capture(&ctx, &randoms);
        assert!(matches!(result, CaptureResult::Failed { .. }));
    }

    #[test]
    fn poke_ball_no_rand1_rejection() {
        // Poké Ball threshold is 255 — rand1 always passes
        let ctx = CaptureContext {
            ball: ItemId::PokeBall,
            wild_max_hp: 100,
            wild_current_hp: 1,
            wild_catch_rate: 255,
            wild_status: StatusCondition::None,
        };
        // rand1=255 — Poké Ball doesn't reject on rand1
        // adjusted_rand1=255 > catch_rate=255? No (<=)
        // w_raw = (100*255)/12/1 = 2125 > 255 → captured
        let randoms = CaptureRandoms {
            rand1: 255,
            rand2: 255,
        };
        assert_eq!(try_capture(&ctx, &randoms), CaptureResult::Captured);
    }
}
