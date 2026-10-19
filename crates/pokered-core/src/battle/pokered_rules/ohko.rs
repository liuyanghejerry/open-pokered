//! Gen I OHKO damage is a typed, randomized $ffff, not a direct HP assignment.
use super::*;

thread_local! {
    static SPEED_FAILURE: RefCell<[Option<MoveId>; 2]> = const { RefCell::new([None, None]) };
}

pub(super) fn clear() {
    SPEED_FAILURE.with(|values| *values.borrow_mut() = [None, None]);
}

pub(super) fn speed_failed(actor: BattlerRef, move_id: MoveId) -> bool {
    SPEED_FAILURE.with(|values| values.borrow()[actor.side as usize] == Some(move_id))
}

/// Called before the ordinary MoveHitTest gates. CalculateDamage calls the OHKO
/// effect first; a slower user returns before type adjustment or accuracy RNG.
pub(super) fn prepare(
    ctx: &mut BattleCtx<'_, PokeredRules>,
    target: BattlerRef,
    source: BattlerRef,
) -> bool {
    let pm = current_move_for(source);
    ctx.mv.is_critical = false; // OneHitKOEffect overwrites CriticalHitTest's flag.
    if effective_speed(ctx.battler(source)) < effective_speed(ctx.battler(target)) {
        SPEED_FAILURE.with(|values| values.borrow_mut()[source.side as usize] = Some(pm.id));
        ctx.mv.damage = 0;
        set_shared_damage(ctx, 0);
        return false;
    }
    let (a1, a2) = effective_types(ctx, source);
    let (d1, d2) = effective_types(ctx, target);
    let mut damage = u16::MAX;
    if pm.move_type == a1 || pm.move_type == a2 {
        damage = damage.wrapping_add(damage / 2);
    }
    let mut missed = false;
    let mut displayed = Effectiveness::Normal;
    // AdjustDamageForMoveType traverses ROM row order and stores a 16-bit
    // result after EACH row. This matters for $ffff overflow and dual types.
    for row in pokered_data::type_chart::TYPE_CHART.iter() {
        if row.attacker != pm.move_type || (row.defender != d1 && row.defender != d2) {
            continue;
        }
        displayed = row.effectiveness;
        let multiplier = match displayed {
            Effectiveness::NoEffect => 0,
            Effectiveness::NotVeryEffective => 5,
            Effectiveness::Normal => 10,
            Effectiveness::SuperEffective => 20,
        };
        damage = (u32::from(damage) * multiplier / 10) as u16;
        missed |= damage == 0;
    }
    // RandomizeDamage skips 0/1, rotates BattleRandom right once, rejects
    // values below 217, then divides by 255. It precedes MoveHitTest.
    if damage > 1 {
        let roll = loop {
            let byte = ctx.rng.next_u8().rotate_right(1);
            if byte >= 217 {
                break byte;
            }
        };
        damage = (u32::from(damage) * u32::from(roll) / 255) as u16;
    }
    ctx.mv.damage = damage;
    PRESENTED_EFFECTIVENESS.with(|values| {
        values.borrow_mut()[source.side as usize] = Some((pm.id, displayed, missed));
    });
    // Even an immune move continues into MoveHitTest (including its RNG draw).
    // The caller preserves this miss after the hit test's early X Accuracy return.
    !missed
}
