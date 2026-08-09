use pokered_data::blockset_data;
use pokered_data::tilesets::TilesetId;
use pokered_data::collision;

fn main() {
    let tileset = TilesetId::RedsHouse2;
    let passable = collision::get_passable_tiles(tileset);
    println!("Passable tiles for RedsHouse2: {:02X?}", passable);
    
    for block_id in [0x10, 0x11, 0x05, 0x08, 0x0F, 0x0D, 0x0C, 0x12].iter() {
        if let Some(tiles) = blockset_data::block_tiles(tileset, *block_id) {
            println!("\nBlock 0x{:02X} tiles:", block_id);
            for row in 0..4 {
                let row_tiles: Vec<String> = (0..4)
                    .map(|col| format!("{:02X}", tiles[row * 4 + col]))
                    .collect();
                println!("  Row {}: {}", row, row_tiles.join(" "));
            }
            let passable_check: Vec<(usize, bool)> = tiles.iter()
                .enumerate()
                .map(|(i, &t)| (i, passable.contains(&t)))
                .collect();
            println!("  Passable: {:?}", passable_check);
        }
    }
    
    println!("\n\nCollision map for RedsHouse2F (X=blocked, .=passable):");
    let blocks: [u8; 16] = [0x10, 0x11, 0x05, 0x08, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0D, 0x0F, 0x0F, 0x0C, 0x0F, 0x0F, 0x12];
    let map_width = 4u8;
    
    for y in 0..16 {
        for x in 0..16 {
            let block_x = (x / 4) as usize;
            let block_y = (y / 4) as usize;
            let sub_x = (x % 4) as usize;
            let sub_y = (y % 4) as usize;
            
            let block_idx = block_y * (map_width as usize) + block_x;
            let block_id = blocks[block_idx];
            
            if let Some(tiles) = blockset_data::block_tiles(tileset, block_id) {
                let tile_id = tiles[sub_y * 4 + sub_x];
                let is_passable = passable.contains(&tile_id);
                
                if !is_passable {
                    print!("X");
                } else {
                    print!(".");
                }
            }
        }
        println!("  y={}", y);
    }
}
