use dotzuki_renderer::tile::{Tile, TileSet};

static PIXELS: [[u8; 8]; 65536] = {
    let mut table = [[0; 8]; 65536];
    let mut n = 0;
    while n < 65536 {
        let low = n & 255;
        let high = n >> 8;
        let mut bit = 0;
        while bit < 8 {
            table[n][bit] = (((low >> (7 - bit)) & 1) | (((high >> (7 - bit)) & 1) << 1)) as u8;
            bit += 1;
        }
        n += 1;
    }
    table
};

pub fn from_2bpp(data: &[u8]) -> TileSet {
    assert_eq!(data.len() % 16, 0);
    let mut tiles = TileSet::blank(data.len() / 16);
    for (index, encoded) in data.chunks_exact(16).enumerate() {
        let mut tile = Tile::blank();
        for (row, planes) in encoded.chunks_exact(2).enumerate() {
            tile.pixels[row] = PIXELS[u16::from_le_bytes([planes[0], planes[1]]) as usize];
        }
        tiles.set(index, tile);
    }
    tiles
}

#[cfg(all(test, feature = "gba-resource-tests"))]
mod tests {
    use super::*;

    #[test]
    fn every_plane_pair_matches_the_upstream_decoder() {
        for low in 0..=255u8 {
            for high in 0..=255u8 {
                let mut encoded = [0; 16];
                encoded[0] = low;
                encoded[1] = high;
                let expected = Tile::from_2bpp(&encoded);
                assert_eq!(
                    PIXELS[u16::from_le_bytes([low, high]) as usize],
                    expected.pixels[0]
                );
            }
        }
    }
}
