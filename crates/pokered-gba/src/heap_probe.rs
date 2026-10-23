//! Fallible allocation checks for the running contiguous-heap watermark.
//! A frame which can still allocate the previous low needs no full bisection.

const MAX_PROBE: usize = 256 * 1024;

/// Return the running minimum after checking the current frame's allocator.
///
/// The result is a watermark, not the current largest free block: a successful
/// allocation of the previous minimum proves this frame cannot lower it. Every
/// positive watermark is checked again on every call. If that allocation fails,
/// search below it for the new low using the same byte-sized reserve requests.
pub fn running_minimum(previous: usize, mut fits: impl FnMut(usize) -> bool) -> usize {
    let limit = previous.min(MAX_PROBE);
    if limit == 0 || fits(limit) {
        return limit;
    }
    let mut lo = 0;
    let mut hi = limit - 1;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustive_capacity_and_previous_low_match_the_running_minimum() {
        for previous in 0..=512 {
            for capacity in 0..=1024 {
                let mut attempts = 0;
                let actual = running_minimum(previous, |size| {
                    attempts += 1;
                    size <= capacity
                });
                assert_eq!(actual, previous.min(capacity));
                if previous > 0 && capacity >= previous {
                    assert_eq!(attempts, 1);
                }
            }
        }
    }

    #[test]
    fn every_stable_frame_rechecks_a_real_allocation_and_a_one_frame_dip_is_kept() {
        let mut minimum = usize::MAX;
        let mut expected = MAX_PROBE;
        for capacity in [
            64000, 65000, 62000, 62000, 70000, 4096, 4096, 12000, 4095, 8192,
        ] {
            let previous = minimum;
            let mut requested = Vec::new();
            minimum = running_minimum(previous, |size| {
                requested.push(size);
                size <= capacity
            });
            expected = expected.min(capacity);
            assert_eq!(minimum, expected);
            assert_eq!(requested[0], previous.min(MAX_PROBE));
        }
        let mut attempts = 0;
        for _ in 0..10000 {
            assert_eq!(
                running_minimum(minimum, |size| {
                    attempts += 1;
                    assert_eq!(size, 4095);
                    true
                }),
                4095
            );
        }
        assert_eq!(attempts, 10000);
    }

    #[test]
    fn initial_ceiling_zero_and_exact_gate_boundary_do_not_overflow_or_round_up() {
        assert_eq!(running_minimum(usize::MAX, |n| n <= MAX_PROBE), MAX_PROBE);
        assert_eq!(
            running_minimum(usize::MAX, |n| n < MAX_PROBE),
            MAX_PROBE - 1
        );
        assert_eq!(running_minimum(0, |_| panic!("already-zero watermark")), 0);
        for capacity in [0, 1, 4095, 4096, 4097] {
            assert_eq!(running_minimum(8192, |n| n <= capacity), capacity);
        }
    }
}
