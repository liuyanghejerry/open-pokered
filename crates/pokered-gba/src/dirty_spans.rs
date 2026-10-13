//! Contiguous dirty cells in a 20-column Mode 4 viewport row.
/// DirtyTiles clips every rectangle to the viewport; no bits above column19
/// are set. Each returned span can be copied with aligned 32-bit DMA.
#[inline(always)]
pub(crate) fn spans(mut mask: u32) -> impl Iterator<Item = (usize, usize)> {
    core::iter::from_fn(move || {
        if mask == 0 { return None; }
        let first = mask.trailing_zeros();
        let width = (mask >> first).trailing_ones();
        mask &= !(((1u32 << width) - 1) << first);
        Some((first as usize, width as usize))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_viewport_mask_copies_exactly_its_dirty_cells() {
        for mask in 0..(1u32 << 20) {
            let mut coverage = [0u8; 20];
            for (first, width) in spans(mask) {
                assert!(width > 0 && first + width <= coverage.len());
                for cell in &mut coverage[first..first + width] { *cell += 1; }
            }
            for (column, count) in coverage.into_iter().enumerate() {
                assert_eq!(count, ((mask >> column) & 1) as u8, "mask {mask:x} column {column}");
            }
        }
    }
}
