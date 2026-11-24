use crate::FrameBuffer;

const DMA3_SOURCE: *mut u32 = 0x0400_00D4 as *mut u32;
const DMA3_DESTINATION: *mut u32 = 0x0400_00D8 as *mut u32;
const DMA3_CONTROL: *mut u32 = 0x0400_00DC as *mut u32;
const DMA_ENABLE: u32 = 1 << 31;
const DMA_32BIT: u32 = 1 << 26;
const DMA_SOURCE_FIXED: u32 = 1 << 24;

/// Immediate DMA3 copy. The CPU resumes after all words have been transferred.
#[inline]
pub unsafe fn copy_words(source: *const u32, destination: *mut u32, words: usize) {
    debug_assert!(words != 0 && words <= u16::MAX as usize);
    unsafe {
        core::ptr::write_volatile(DMA3_SOURCE, source as usize as u32);
        core::ptr::write_volatile(DMA3_DESTINATION, destination as usize as u32);
        core::ptr::write_volatile(DMA3_CONTROL, DMA_ENABLE | DMA_32BIT | words as u32);
    }
}

#[inline]
unsafe fn fill_zero_words(destination: *mut u32, words: usize) {
    debug_assert!(words != 0 && words <= u16::MAX as usize);
    let source = 0u32;
    unsafe {
        core::ptr::write_volatile(DMA3_SOURCE, &source as *const u32 as usize as u32);
        core::ptr::write_volatile(DMA3_DESTINATION, destination as usize as u32);
        core::ptr::write_volatile(
            DMA3_CONTROL,
            DMA_ENABLE | DMA_32BIT | DMA_SOURCE_FIXED | words as u32,
        );
    }
}

pub fn copy_frame(destination: &mut FrameBuffer, source: &FrameBuffer) {
    destination.copy_from_with(source, |output, input| {
        let words = input.len() / 4;
        let aligned =
            (input.as_ptr() as usize | output.as_mut_ptr() as usize | input.len()) & 3 == 0;
        if aligned && words != 0 && words <= u16::MAX as usize {
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            unsafe { copy_words(input.as_ptr().cast(), output.as_mut_ptr().cast(), words) };
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        } else {
            output.copy_from_slice(input);
        }
    });
}

pub fn clear_frame(destination: &mut FrameBuffer) {
    let output = destination.indices_mut();
    let words = output.len() / 4;
    let aligned = (output.as_mut_ptr() as usize | output.len()) & 3 == 0;
    if aligned && words != 0 && words <= u16::MAX as usize {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        unsafe { fill_zero_words(output.as_mut_ptr().cast(), words) };
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    } else {
        output.fill(0);
    }
}
