//! Lock-free single-producer single-consumer ring buffer for real-time audio.
//!
//! The game thread (producer) pushes audio samples each frame.
//! The iOS AVAudioEngine render callback (consumer) pops samples on the
//! real-time audio thread. No locks, no blocking, no heap allocation on
//! the hot path.

use std::sync::atomic::{AtomicU64, Ordering};

// ── Constants ────────────────────────────────────────────────────────────

/// Number of stereo sample pairs the ring buffer can hold.
/// 4096 pairs × 2 channels = 8192 f32 slots ≈ 85ms @ 48 kHz.
pub const STEREO_CAPACITY: usize = 4096;

/// Internal buffer size in individual f32 slots (= STEREO_CAPACITY × 2).
/// Must be a power of two for efficient bitmask-based modulo.
const BUFFER_SIZE: usize = STEREO_CAPACITY * 2; // 8192

// ── AudioRingBuffer ──────────────────────────────────────────────────────

/// Fixed-size, lock-free ring buffer for interleaved stereo f32 samples.
///
/// # Thread Safety
///
/// Single-producer single-consumer (SPSC):
/// - `push()` — called ONLY from the game thread
/// - `pop()` — called ONLY from the audio thread
///
/// Uses `AtomicU64` head pointers with `Acquire`/`Release` ordering to
/// ensure correct happens-before relationships without locks.
///
/// # Buffer Full Behaviour
///
/// When the buffer is full, `push` silently drops the oldest samples by
/// advancing the read head. No panic, no block.
///
/// # Buffer Empty Behaviour
///
/// When the buffer is empty, `pop` returns 0. The caller is responsible
/// for filling the output with silence.
pub struct AudioRingBuffer {
    /// Raw pointer to the heap-allocated data. Never freed while the struct
    /// lives (`_data` owns the allocation). Accessed via `unsafe` pointer
    /// arithmetic in `push`/`pop` because both take `&self`.
    data_ptr: *mut f32,
    /// Owns the heap allocation. Never resized after construction.
    _data: Vec<f32>,
    /// Write position (monotonically increasing). Only written by the
    /// producer (game thread), read by the consumer (audio thread).
    write_head: AtomicU64,
    /// Read position (monotonically increasing). Only written by the
    /// consumer (audio thread), read by the producer (game thread).
    read_head: AtomicU64,
    /// Total capacity in f32 slots (BUFFER_SIZE, a power of two).
    capacity: usize,
    /// Bitmask for fast modulo: `index = head & mask`.
    mask: usize,
}

// SAFETY: AudioRingBuffer is designed for cross-thread usage.
// Data races on the buffer data are prevented by the atomic head pointers,
// which ensure producer and consumer never touch the same slots concurrently.
// The raw pointer is from a Vec that is never moved in memory after
// construction and never resized.
unsafe impl Send for AudioRingBuffer {}
unsafe impl Sync for AudioRingBuffer {}

impl AudioRingBuffer {
    /// Create a new ring buffer with capacity for `STEREO_CAPACITY` (4096)
    /// stereo sample pairs. All samples are initialised to zero.
    pub fn new() -> Self {
        let data = vec![0.0f32; BUFFER_SIZE];
        let data_ptr = data.as_ptr() as *mut f32;
        Self {
            data_ptr,
            _data: data,
            write_head: AtomicU64::new(0),
            read_head: AtomicU64::new(0),
            capacity: BUFFER_SIZE,
            mask: BUFFER_SIZE - 1,
        }
    }

    /// Return the buffer capacity in stereo sample pairs (4096).
    pub fn capacity(&self) -> usize {
        STEREO_CAPACITY
    }

    /// Push interleaved stereo f32 samples into the buffer.
    ///
    /// Called from the **game thread** (never blocks).
    ///
    /// `samples` contains interleaved L/R values, so `samples.len()` should
    /// be a multiple of 2.
    ///
    /// If the data would overflow the buffer, the oldest unread samples are
    /// silently dropped by advancing the read head.
    pub fn push(&self, samples: &[f32]) {
        let count = samples.len() as u64;
        if count == 0 {
            return;
        }

        // If the incoming batch is larger than the entire buffer, truncate
        // to the trailing `capacity` items so we keep the most recent data.
        let effective_samples = if count > self.capacity as u64 {
            let offset = (count - self.capacity as u64) as usize;
            &samples[offset..]
        } else {
            samples
        };
        let count = effective_samples.len() as u64;

        let write = self.write_head.load(Ordering::Relaxed);
        // Acquire on read_head: see the latest pops (which were stored with Release).
        let read = self.read_head.load(Ordering::Acquire);

        // How many unread items are currently in the buffer.
        let used = write.wrapping_sub(read);
        // How many free slots remain.
        let available = self.capacity as u64 - used;

        if count > available {
            // Buffer overflow — advance read head to drop oldest samples.
            // new_read = write + count - capacity
            let new_read = write.wrapping_add(count).wrapping_sub(self.capacity as u64);
            self.read_head.store(new_read, Ordering::Release);
        }

        // Write samples into the data buffer.
        let start = (write as usize) & self.mask;
        unsafe {
            if start + count as usize <= self.capacity {
                // Single contiguous write.
                std::ptr::copy_nonoverlapping(
                    effective_samples.as_ptr(),
                    self.data_ptr.add(start),
                    count as usize,
                );
            } else {
                // Wraparound: write first part to end, rest from beginning.
                let first_part = self.capacity - start;
                std::ptr::copy_nonoverlapping(
                    effective_samples.as_ptr(),
                    self.data_ptr.add(start),
                    first_part,
                );
                std::ptr::copy_nonoverlapping(
                    effective_samples.as_ptr().add(first_part),
                    self.data_ptr,
                    count as usize - first_part,
                );
            }
        }

        // Release on write_head: make the written data visible to the consumer.
        self.write_head.store(write.wrapping_add(count), Ordering::Release);
    }

    /// Pop up to `count` interleaved stereo f32 samples from the buffer.
    ///
    /// Called from the **audio thread** (never blocks).
    ///
    /// Returns the number of samples (individual f32 values) actually
    /// read. If the buffer is empty, returns 0 immediately — the caller
    /// should fill `buf` with silence in that case.
    ///
    /// `buf` must have space for at least `count` values.
    pub fn pop(&self, buf: &mut [f32], count: usize) -> usize {
        let count = count as u64;
        if count == 0 {
            return 0;
        }

        let read = self.read_head.load(Ordering::Relaxed);
        // Acquire on write_head: see the latest pushes (which were stored with Release).
        let write = self.write_head.load(Ordering::Acquire);

        // Number of readable items.
        let available = write.wrapping_sub(read);
        if available == 0 {
            return 0;
        }

        let actual = count.min(available) as usize;

        // Read samples from the data buffer.
        let start = (read as usize) & self.mask;
        unsafe {
            if start + actual <= self.capacity {
                std::ptr::copy_nonoverlapping(
                    self.data_ptr.add(start),
                    buf.as_mut_ptr(),
                    actual,
                );
            } else {
                // Wraparound: read first part from end, rest from beginning.
                let first_part = self.capacity - start;
                std::ptr::copy_nonoverlapping(
                    self.data_ptr.add(start),
                    buf.as_mut_ptr(),
                    first_part,
                );
                std::ptr::copy_nonoverlapping(
                    self.data_ptr,
                    buf.as_mut_ptr().add(first_part),
                    actual - first_part,
                );
            }
        }

        // Release on read_head: make the consumed space visible to the producer.
        self.read_head.store(read.wrapping_add(actual as u64), Ordering::Release);

        actual
    }
}

impl Default for AudioRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_capacity() {
        let rb = AudioRingBuffer::new();
        assert_eq!(rb.capacity(), STEREO_CAPACITY);
        assert_eq!(BUFFER_SIZE, 8192);
        assert!(BUFFER_SIZE.is_power_of_two());
    }

    #[test]
    fn test_push_pop_basic() {
        let rb = AudioRingBuffer::new();

        // Push 2000 interleaved stereo f32 values (1000 pairs).
        let input: Vec<f32> = (0..2000).map(|i| i as f32).collect();
        rb.push(&input);

        // Pop 1000 values.
        let mut output = vec![0.0f32; 1000];
        let popped = rb.pop(&mut output, 1000);
        assert_eq!(popped, 1000);

        // Verify data integrity.
        for (i, &v) in output.iter().enumerate() {
            assert_eq!(v, i as f32, "mismatch at index {i}");
        }

        // Remaining 1000 values should still be in the buffer.
        let mut output2 = vec![0.0f32; 1000];
        let popped2 = rb.pop(&mut output2, 1000);
        assert_eq!(popped2, 1000);
        for (i, &v) in output2.iter().enumerate() {
            assert_eq!(v, (1000 + i) as f32, "mismatch at index {i}");
        }
    }

    #[test]
    fn test_wraparound() {
        let rb = AudioRingBuffer::new();

        // Fill the buffer to 7000 so that a push of 3000 will wrap around.
        let fill: Vec<f32> = (0..7000).map(|i| i as f32).collect();
        rb.push(&fill);

        // Pop 7000 to make room, then push 3000 that wraps.
        let mut drain = vec![0.0f32; 7000];
        rb.pop(&mut drain, 7000);

        // Now write_head is at 7000 (mod 8192 = 7000).
        // Push 3000: should wrap around (7000..8192 = 1192, then 0..1808 = 1808).
        let wrap_input: Vec<f32> = (0..3000).map(|i| (10000 + i) as f32).collect();
        rb.push(&wrap_input);

        // Pop 3000 — should get all 3000 back in order.
        let mut output = vec![0.0f32; 3000];
        let popped = rb.pop(&mut output, 3000);
        assert_eq!(popped, 3000);
        for (i, &v) in output.iter().enumerate() {
            assert_eq!(v, (10000 + i) as f32, "mismatch at index {i}");
        }
    }

    #[test]
    fn test_empty_pop() {
        let rb = AudioRingBuffer::new();
        let mut buf = vec![0.0f32; 100];
        let popped = rb.pop(&mut buf, 100);
        assert_eq!(popped, 0);
    }

    #[test]
    fn test_full_push_no_panic() {
        let rb = AudioRingBuffer::new();

        // Push exactly full capacity.
        let fill: Vec<f32> = (0..BUFFER_SIZE as i32).map(|i| i as f32).collect();
        rb.push(&fill);

        // Push additional samples — should not panic, oldest dropped.
        let extra: Vec<f32> = vec![999.0f32; 1000];
        rb.push(&extra);

        // Should be able to read 8192 samples (the latest).
        let mut output = vec![0.0f32; BUFFER_SIZE];
        let popped = rb.pop(&mut output, BUFFER_SIZE);
        assert_eq!(popped, BUFFER_SIZE);

        // The first 1000 should be from the fill (offset by 1000 since 1000 oldest were dropped).
        for i in 0..(BUFFER_SIZE - 1000) {
            assert_eq!(output[i], (1000 + i) as f32, "mismatch at old index {i}");
        }
        // The last 1000 should be 999.0 from the extra push.
        for i in (BUFFER_SIZE - 1000)..BUFFER_SIZE {
            assert_eq!(output[i], 999.0, "mismatch at new index {i}");
        }
    }

    #[test]
    fn test_push_larger_than_capacity() {
        let rb = AudioRingBuffer::new();

        // Push a batch larger than the entire buffer.
        let big: Vec<f32> = (0..(BUFFER_SIZE + 2000) as i32).map(|i| i as f32).collect();
        rb.push(&big);

        // Should only keep the last BUFFER_SIZE values.
        let mut output = vec![0.0f32; BUFFER_SIZE];
        let popped = rb.pop(&mut output, BUFFER_SIZE);
        assert_eq!(popped, BUFFER_SIZE);

        // The first value should be 2000 (oldest 2000 dropped).
        for (i, &v) in output.iter().enumerate() {
            assert_eq!(v, (2000 + i) as f32, "mismatch at index {i}");
        }
    }

    #[test]
    fn test_push_zero_length() {
        let rb = AudioRingBuffer::new();
        rb.push(&[]); // should not panic

        let mut buf = vec![0.0f32; 10];
        assert_eq!(rb.pop(&mut buf, 10), 0);
    }

    #[test]
    fn test_pop_more_than_available() {
        let rb = AudioRingBuffer::new();
        let input: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        rb.push(&input);

        let mut output = vec![0.0f32; 10];
        let popped = rb.pop(&mut output, 10);
        assert_eq!(popped, 4);
        assert_eq!(&output[..4], &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_multiple_push_pop_cycles() {
        let rb = AudioRingBuffer::new();

        for cycle in 0..10 {
            let base = (cycle * 2000) as f32;
            let input: Vec<f32> = (0..2000).map(|i| base + i as f32).collect();
            rb.push(&input);

            let mut output = vec![0.0f32; 1000];
            let popped = rb.pop(&mut output, 1000);
            assert_eq!(popped, 1000);
            for (i, &v) in output.iter().enumerate() {
                assert_eq!(v, base + i as f32, "cycle {cycle} part 1 index {i}");
            }

            let mut output2 = vec![0.0f32; 1000];
            let popped2 = rb.pop(&mut output2, 1000);
            assert_eq!(popped2, 1000);
            for (i, &v) in output2.iter().enumerate() {
                assert_eq!(
                    v,
                    base + 1000.0 + i as f32,
                    "cycle {cycle} part 2 index {i}"
                );
            }
        }
    }

    /// Simulate two-thread behaviour: interleaved push/pop from a single
    /// thread to verify data consistency across many operations.
    #[test]
    fn test_interleaved_operations() {
        let rb = AudioRingBuffer::new();
        let mut total_pushed: u64 = 0;
        let mut total_popped: u64 = 0;

        for _ in 0..50 {
            // Push 500 samples (250 stereo pairs).
            let input: Vec<f32> = (0..500)
                .map(|i| (total_pushed as f32 + i as f32) * 0.01)
                .collect();
            rb.push(&input);
            total_pushed += 500;

            // Pop 300 samples.
            let mut buf = vec![0.0f32; 300];
            let popped = rb.pop(&mut buf, 300);
            assert!(popped <= 300);
            total_popped += popped as u64;
        }

        // Drain remainder.
        let mut drain = vec![0.0f32; BUFFER_SIZE];
        let drained = rb.pop(&mut drain, BUFFER_SIZE);
        total_popped += drained as u64;

        // Total popped should equal total pushed minus what's still in buffer.
        let remaining = total_pushed - total_popped;
        assert!(remaining <= BUFFER_SIZE as u64);
    }

    #[test]
    fn test_default() {
        let rb = AudioRingBuffer::default();
        assert_eq!(rb.capacity(), STEREO_CAPACITY);
        let mut buf = vec![0.0f32; 10];
        assert_eq!(rb.pop(&mut buf, 10), 0);
    }
}
