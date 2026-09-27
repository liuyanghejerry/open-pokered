//! EWRAM shadow-allocator audit (desktop, `--features ewram-audit`).
//!
//! Replays the heap traffic of the desktop game process against a model of
//! agb 0.25's `BlockAllocator` over the GBA's real EWRAM heap window
//! (`__ewram_data_end..0x0204_0000`, 166,424 B on the current release ELF).
//! The model mirrors the allocator's semantics:
//!
//! - 32-bit `Block` header = 8 bytes, 8-byte alignment; an allocation costs
//!   `max(8, align_up(size, 8))`,
//! - first-fit over a sorted free list with splitting (remainder `>= 8`),
//! - bump allocation from the tip when nothing fits,
//! - freed blocks are inserted in address order and merged with neighbours,
//! - `realloc` extends in place when the block sits at the bump tip (the
//!   common `Vec` growth pattern) and otherwise allocates + copies + frees.
//!
//! An allocation the model cannot satisfy is a **violation**: on hardware it
//! would be agb's alloc-error panic, an invisible freeze.
//!
//! Caveats, deliberately accepted for a diagnostic build:
//! - The desktop process allocates far more than the GBA (window/renderer,
//!   clap, debug-protocol strings). [`reset`] is called when the game loop
//!   starts so pre-gameplay churn does not flood the model, but the headless
//!   backend still contributes — treat absolute numbers as upper bounds and
//!   lean on violations, per-phase samples and the live-bytes trend.
//! - No mutex around the model: std's `Mutex` lock path allocates under
//!   audit (empirically proven to recurse the allocator), and both the
//!   game loop and the GBA build are single-threaded anyway. Allocations
//!   from other threads are skipped rather than modelled.
//! - Log output goes to `$EWRAM_AUDIT_LOG` (default `/tmp/ewram-audit.log`).

use crate::alloc_prelude::*;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::alloc::System;
use std::collections::HashMap;
use std::io::Write;

/// GBA EWRAM heap window: `__ewram_data_end..0x0204_0000`.
const REGION: usize = 166_424;
/// 32-bit `Block { size: usize, next: Option<NonNull> }`.
const BLOCK_HEADER: usize = 8;
/// Sample the watermark every N allocations.
const SAMPLE_EVERY: usize = 5_000;
/// Largest single allocations retained in the report.
const TOP_KEEP: usize = 12;

/// 0 = inactive, 1 = active, 2 = active on a different thread (skip).
static OWNER: AtomicU8 = AtomicU8::new(0);

fn effective(size: usize) -> usize {
    size.max(BLOCK_HEADER).next_multiple_of(8)
}

#[derive(Clone, Copy)]
struct BlockInfo {
    offset: usize,
    size: usize,
}

struct Model {
    bump: usize,
    free: Vec<(usize, usize)>, // sorted (offset, size)
    live: usize,
    allocs: usize,
    frees: usize,
    violations: usize,
    worst_free: usize,
    worst_at_alloc: usize,
    top: Vec<usize>,
    log: Option<std::fs::File>,
    log_tried: bool,
    blocks: HashMap<usize, BlockInfo>,
}

impl Model {
    fn new() -> Self {
        Model {
            bump: 0,
            free: Vec::new(),
            live: 0,
            allocs: 0,
            frees: 0,
            violations: 0,
            worst_free: REGION,
            worst_at_alloc: 0,
            top: Vec::new(),
            log: None,
            log_tried: false,
            blocks: HashMap::new(),
        }
    }

    fn largest_free(&self) -> usize {
        self.free
            .iter()
            .map(|&(_, size)| size)
            .chain(std::iter::once(REGION - self.bump))
            .max()
            .unwrap_or(0)
    }

    fn note_top(&mut self, size: usize) {
        if self.top.len() < TOP_KEEP {
            self.top.push(size);
        } else if size > self.top[TOP_KEEP - 1] {
            self.top[TOP_KEEP - 1] = size;
        } else {
            return;
        }
        self.top.sort_unstable_by(|a, b| b.cmp(a));
    }

    fn write_line(&mut self, line: String) {
        if !self.log_tried {
            // Opened lazily: `std::env` is not usable during pre-main
            // allocations, and the first line is written well after boot.
            self.log_tried = true;
            self.log = std::env::var("EWRAM_AUDIT_LOG")
                .ok()
                .and_then(|p| std::fs::File::create(p).ok());
        }
        if let Some(ref mut f) = self.log {
            let _ = writeln!(f, "{}", line);
        }
    }

    fn sample(&mut self, reason: &str) {
        let free = self.largest_free();
        if free < self.worst_free {
            self.worst_free = free;
            self.worst_at_alloc = self.allocs;
        }
        let line = format!(
            "sample {} allocs={} frees={} live={} largest_free={} bump_used={} worst_free={} worst_at={} violations={} top={:?}",
            reason,
            self.allocs,
            self.frees,
            self.live,
            free,
            self.bump,
            self.worst_free,
            self.worst_at_alloc,
            self.violations,
            self.top
        );
        self.write_line(line);
    }

    fn insert_free(&mut self, offset: usize, size: usize) {
        let pos = self
            .free
            .binary_search_by_key(&offset, |&(o, _)| o)
            .unwrap_or_else(|p| p);
        self.free.insert(pos, (offset, size));
        self.merge_at(pos);
    }

    /// agb normalise: merge with contiguous neighbours.
    fn merge_at(&mut self, pos: usize) {
        let mut pos = pos;
        while pos + 1 < self.free.len() {
            let (off, size) = self.free[pos];
            let (next_off, next_size) = self.free[pos + 1];
            if off + size == next_off {
                self.free[pos] = (off, size + next_size);
                self.free.remove(pos + 1);
            } else {
                break;
            }
        }
        if pos > 0 {
            let (prev_off, prev_size) = self.free[pos - 1];
            let (off, size) = self.free[pos];
            if prev_off + prev_size == off {
                self.free[pos - 1] = (prev_off, prev_size + size);
                self.free.remove(pos);
            }
        }
    }

    fn alloc(&mut self, ptr: usize, size: usize) {
        let want = effective(size);
        self.allocs += 1;
        self.note_top(size);

        let mut chosen: Option<(usize, usize)> = None;
        for (i, &(off, block)) in self.free.iter().enumerate() {
            if block == want || block >= want + BLOCK_HEADER {
                chosen = Some((i, off));
                break;
            }
        }
        let offset = if let Some((i, off)) = chosen {
            let (_, block) = self.free.remove(i);
            if block > want {
                let pos = self
                    .free
                    .binary_search_by_key(&(off + want), |&(o, _)| o)
                    .unwrap_or_else(|p| p);
                self.free.insert(pos, (off + want, block - want));
            }
            off
        } else if self.bump + want <= REGION {
            let off = self.bump;
            self.bump += want;
            off
        } else {
            self.violations += 1;
            let free = self.largest_free();
            let line = format!(
                "violation: alloc size={} want={} live={} largest_free={} bump={} allocs={}",
                size, want, self.live, free, self.bump, self.allocs
            );
            self.write_line(line);
            // Saturate so later statistics stay meaningful.
            let off = self.bump;
            self.bump = REGION;
            off
        };
        self.blocks.insert(ptr, BlockInfo { offset, size: want });
        self.live += want;

        if self.allocs % SAMPLE_EVERY == 0 {
            self.sample("periodic");
        }
    }

    fn free(&mut self, ptr: usize) {
        let Some(info) = self.blocks.remove(&ptr) else {
            return;
        };
        self.frees += 1;
        self.live = self.live.saturating_sub(info.size);
        self.insert_free(info.offset, info.size);
    }

    fn realloc(&mut self, old: usize, new: usize, new_size: usize) {
        let want = effective(new_size);
        let Some(info) = self.blocks.get(&old).copied() else {
            self.alloc(new, new_size);
            return;
        };
        if old == new {
            if want > info.size && info.offset + info.size == self.bump {
                // In-place growth at the bump tip: agb extends the bump.
                self.bump += want - info.size;
                self.live += want - info.size;
                self.blocks.insert(
                    old,
                    BlockInfo {
                        offset: info.offset,
                        size: want,
                    },
                );
            } else if want > info.size {
                // agb would try the free block right after; approximate with
                // the allocate-and-copy path.
                self.alloc(new, new_size);
                self.free(old);
            } else {
                self.live = self.live.saturating_sub(info.size - want);
                self.blocks.insert(
                    old,
                    BlockInfo {
                        offset: info.offset,
                        size: want,
                    },
                );
            }
            return;
        }
        self.alloc(new, new_size);
        self.free(old);
    }

    fn summary(&mut self) {
        let free = self.largest_free();
        if free < self.worst_free {
            self.worst_free = free;
            self.worst_at_alloc = self.allocs;
        }
        let line = format!(
            "SUMMARY allocs={} frees={} live={} bump_used={} largest_free={} worst_free={} worst_at={} violations={} top={:?}",
            self.allocs,
            self.frees,
            self.live,
            self.bump,
            free,
            self.worst_free,
            self.worst_at_alloc,
            self.violations,
            self.top
        );
        self.write_line(line);
    }
}

// No Mutex: std's lock path allocates under audit (proven to recurse the
// allocator), and the game loop is single-threaded. `OWNER` serialises
// lightly: only one thread (the first to touch the model) is modelled.
static mut MODEL_SLOT: Option<Model> = None;
static IN_MODEL: AtomicBool = AtomicBool::new(false);

#[inline]
fn in_model() -> bool {
    IN_MODEL.load(Ordering::SeqCst)
}

fn with_model<R>(f: impl FnOnce(&mut Model) -> R) -> R {
    let slot = unsafe { &mut *core::ptr::addr_of_mut!(MODEL_SLOT) };
    if slot.is_none() {
        *slot = Some(Model::new());
    }
    IN_MODEL.store(true, Ordering::SeqCst);
    let out = f(slot.as_mut().unwrap());
    IN_MODEL.store(false, Ordering::SeqCst);
    out
}

/// Start the model from a clean slate: called when the game loop begins, so
/// desktop setup (clap, window/renderer, save loading) does not flood the
/// model. The log file is truncated by the reset.
pub fn reset() {
    let slot = unsafe { &mut *core::ptr::addr_of_mut!(MODEL_SLOT) };
    *slot = None;
}

/// Flush a summary line (no automatic at-exit hook: the playthrough driver
/// terminates the game process).
pub fn dump_summary() {
    with_model(|m| m.summary());
}

#[inline]
fn should_track() -> bool {
    // 0 -> claiming this thread as the owner; 2 -> a different thread.
    match OWNER.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(_) | Err(1) => true,
        Err(_) => false,
    }
}

pub struct AuditAllocator;

unsafe impl GlobalAlloc for AuditAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() && !in_model() && should_track() {
            let size = layout.size();
            with_model(|m| m.alloc(ptr as usize, size));
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !in_model() && should_track() {
            with_model(|m| m.free(ptr as usize));
        }
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, layout, new_size);
        if !new_ptr.is_null() && !in_model() && should_track() {
            with_model(|m| m.realloc(ptr as usize, new_ptr as usize, new_size));
        }
        new_ptr
    }
}
