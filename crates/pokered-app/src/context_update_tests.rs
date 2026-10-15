use super::{fnv_mix, fnv_multiply_arm32, QuerySeedSnapshot, FNV_OFFSET};
use crate::alloc_prelude::*;

fn reference_multiply(value: u64) -> u64 {
    (u128::from(value) * 0x0000_0100_0000_01b3u128) as u64
}

#[test]
fn arm32_fnv_product_matches_independent_wide_arithmetic() {
    for high in [
        0u32,
        1,
        0xffff,
        0x1_0000,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ] {
        for middle in [0u32, 1, 255, 65534, 65535] {
            for low in 0..=65535u32 {
                let value = (u64::from(high) << 32) | (u64::from(middle) << 16) | u64::from(low);
                assert_eq!(fnv_multiply_arm32(value), reference_multiply(value));
            }
        }
    }
    let mut value = 0x98f7_24ae_1cc3_018bu64;
    for _ in 0..1_000_000 {
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        assert_eq!(fnv_multiply_arm32(value), reference_multiply(value));
    }
}

#[test]
fn complete_query_hash_streams_preserve_the_old_hash_and_chunk_boundaries() {
    let bytes: Vec<u8> = (0..=255).cycle().take(4096).collect();
    for size in [0, 1, 2, 4, 11, 19, 38, 64, 255, 4096] {
        let data = &bytes[..size];
        let expected = data.iter().fold(FNV_OFFSET, |hash, byte| {
            reference_multiply(hash ^ u64::from(*byte))
        });
        let mut old = FNV_OFFSET;
        fnv_mix(&mut old, data);
        assert_eq!(old, expected);
        for chunk_size in [1, 2, 4, 11, 19, 64] {
            let mut arm32 = FNV_OFFSET;
            for chunk in data.chunks(chunk_size) {
                for &byte in chunk {
                    arm32 = fnv_multiply_arm32(arm32 ^ u64::from(byte));
                }
            }
            assert_eq!(arm32, expected);
        }
    }
}

#[test]
fn query_scalar_hashes_preserve_byte_order_and_full_width() {
    let mut actual = FNV_OFFSET;
    QuerySeedSnapshot::hash_u8(&mut actual, 0xff);
    QuerySeedSnapshot::hash_u32(&mut actual, 0x80ff_0100);
    QuerySeedSnapshot::hash_u8(&mut actual, 0);
    let bytes = [0xffu8, 0x00, 0x01, 0xff, 0x80, 0x00];
    let expected = bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        reference_multiply(hash ^ u64::from(*byte))
    });
    let arm32 = bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        fnv_multiply_arm32(hash ^ u64::from(*byte))
    });
    assert_eq!(actual, expected);
    assert_eq!(arm32, expected);
}

fn game() -> super::PokemonGame {
    use pokered_core::data::wild_data::GameVersion;
    let mut game = super::PokemonGame::new_with_options(
        GameVersion::Red,
        None,
        None,
        None,
        false,
        None,
        false,
        true,
        #[cfg(feature = "debug-server")]
        None,
    );
    game.player_name = "RED".to_string();
    game.set_seed(42);
    game
}

fn party() -> pokered_core::pokemon::party::Party {
    use pokered_data::species::Species;
    let mon =
        pokered_core::pokemon::stats::create_pokemon(Species::Pikachu, 12, [0x98, 0x76]).unwrap();
    pokered_core::pokemon::party::Party::from(vec![mon])
}

fn reserved_name() -> String {
    let mut name = String::with_capacity(256);
    name.push_str("RED");
    name
}

#[test]
fn ordinary_battle_keeps_a_stable_name_allocation_and_updates_a_changed_name() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use pokered_core::{battle::BattleScreen, game_state::GameScreen};
            let mut game = game();
            let mons = party().to_vec();
            game.battle = BattleScreen::from_parties(false, &mons, &mons, None);
            game.state.screen = GameScreen::Battle;
            let name = reserved_name();
            let pointer = name.as_ptr();
            game.battle.player_name = Some(name);
            for _ in 0..3 {
                game.update(&pokered_renderer::input::InputState::new());
                let name = game.battle.player_name.as_ref().unwrap();
                assert_eq!(name.as_ptr(), pointer);
                assert_eq!(name.capacity(), 256);
            }
            game.player_name = "BLUE".to_string();
            game.update(&pokered_renderer::input::InputState::new());
            assert_eq!(game.battle.player_name.as_deref(), Some("BLUE"));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn canonical_link_battle_keeps_a_stable_name_allocation_and_updates_a_changed_name() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use pokered_core::{
                battle::link_battle_driver::LinkBattleDriver, game_state::GameScreen,
                link::transport::ChannelTransport,
            };
            let (a, b) = ChannelTransport::new_pair();
            let mut local = LinkBattleDriver::new(Box::new(a), party(), "RED".to_string())
                .with_host_random_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
            let mut peer = LinkBattleDriver::new(Box::new(b), party(), "BLUE".to_string())
                .with_host_random_list([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
            local.start_handshake().unwrap();
            peer.poll();
            local.poll();
            local.request_battle().unwrap();
            peer.poll();
            peer.accept_battle().unwrap();
            local.poll();
            peer.poll();
            let name = reserved_name();
            let pointer = name.as_ptr();
            local.screen_mut().unwrap().player_name = Some(name);
            let mut game = game();
            game.battle = local.screen().unwrap().clone();
            game.link_battle = Some(local);
            game.state.screen = GameScreen::Battle;
            for _ in 0..3 {
                game.update(&pokered_renderer::input::InputState::new());
                let name = game
                    .link_battle
                    .as_ref()
                    .unwrap()
                    .screen()
                    .unwrap()
                    .player_name
                    .as_ref()
                    .unwrap();
                assert_eq!(name.as_ptr(), pointer);
                assert_eq!(name.capacity(), 256);
            }
            game.player_name = "GREEN".to_string();
            game.update(&pokered_renderer::input::InputState::new());
            assert_eq!(
                game.link_battle
                    .as_ref()
                    .unwrap()
                    .screen()
                    .unwrap()
                    .player_name
                    .as_deref(),
                Some("GREEN")
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
