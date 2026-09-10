//! # Hello JRPG — a minimal demo proving `dotzuki-engine` is game-agnostic.
//!
//! Demonstrates: overworld movement, NPC interaction, turn-based battle,
//! dialog, save/load, menu navigation. Uses **only** `dotzuki_engine` traits.
//! Zero pokered-* imports. Zero imports from `examples/` crates.

use crate::alloc_prelude::*;
use dotzuki_engine::battle::{
    BattleAI, BattleProvider, BattleState, BattlerState, DamageResult, EffectHandler, EffectResult,
    EnumMap, MoveEffect, TypeChart,
};
use dotzuki_engine::items::{ItemProvider, ItemResult, ShopProvider};
use dotzuki_engine::map::MapTrait;
use dotzuki_engine::menu::{MenuInput, MenuLayout, MenuOption, MenuProvider, MenuSystem};
use dotzuki_engine::overworld::{
    try_move, CollisionProvider, Direction, MapConnections, MapData, NpcDefinition,
    NpcMovementType, NpcRuntimeState, OverworldState,
};
use dotzuki_engine::save::{InMemoryStorage, SaveData, SaveError, SaveManager, SaveSlot};
use dotzuki_engine::text::{ControlAction, DialogEngine, DialogState, TextProvider, TileBuffer};
use dotzuki_engine::tileset::TilesetTrait;

// ── Game-specific types ───────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Element {
    Physical,
    Magical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Species {
    Warrior,
    Mage,
}

impl Species {
    fn name(&self) -> &str {
        match self {
            Self::Warrior => "Warrior",
            Self::Mage => "Mage",
        }
    }
    fn element(&self) -> Element {
        match self {
            Self::Warrior => Element::Physical,
            Self::Mage => Element::Magical,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MoveKind {
    Slash,
    Fireball,
}

impl MoveKind {
    fn name(&self) -> &str {
        match self {
            Self::Slash => "Slash",
            Self::Fireball => "Fireball",
        }
    }
    fn power(&self) -> u16 {
        match self {
            Self::Slash => 10,
            Self::Fireball => 12,
        }
    }
    fn element(&self) -> Element {
        match self {
            Self::Slash => Element::Physical,
            Self::Fireball => Element::Magical,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatId {
    HP,
    ATK,
    DEF,
    SPD,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StatusKind {
    Healthy,
    Burned,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ability {
    Brave,
    Arcane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ItemKind {
    Potion,
    Elixir,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ItemEffect {
    Heal(u16),
    CureStatus,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MonsterData {
    species: Species,
    max_hp: u16,
    current_hp: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AsciiChar {
    Char(char),
    Newline,
    WaitInput,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MapId(u8);
impl MapTrait for MapId {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Tileset(u8);
impl TilesetTrait for Tileset {
    fn id(&self) -> u8 {
        self.0
    }
    fn name(&self) -> &'static str {
        "TownTiles"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuScreen {
    Main,
}

// ── HelloConfig — implements all engine traits ─────────────────────

struct HelloConfig {
    main_menu_options: Vec<MenuOption>,
}

impl HelloConfig {
    fn new() -> Self {
        Self {
            main_menu_options: vec![
                MenuOption::new("Hello"),
                MenuOption::new("Battle"),
                MenuOption::new("Quit"),
            ],
        }
    }
}

impl Default for HelloConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ── TypeChart ──────────────────────────────────────────────────────

impl TypeChart for HelloConfig {
    type Type = Element;

    fn effectiveness(attacking: &Self::Type, defending: &[Self::Type]) -> f32 {
        let def = defending.first().copied().unwrap_or(Element::Physical);
        match (attacking, def) {
            (Element::Physical, Element::Magical) => 2.0,
            (Element::Magical, Element::Physical) => 0.5,
            _ => 1.0,
        }
    }
}

// ── BattleProvider ─────────────────────────────────────────────────

impl BattleProvider for HelloConfig {
    type Monster = MonsterData;
    type Move = MoveKind;
    type Ability = Ability;
    type Status = StatusKind;
    type Stat = StatId;
    type Species = Species;
    type Type = Element;
    type Item = ItemKind;

    fn calculate_damage(
        &self,
        move_: &MoveKind,
        attacker: &BattlerState<Self>,
        defender: &BattlerState<Self>,
        _random: u8,
        _is_critical: bool,
    ) -> DamageResult {
        let atk = attacker.stats.get(StatId::ATK).copied().unwrap_or(10);
        let def = defender.stats.get(StatId::DEF).copied().unwrap_or(5).max(1);
        let eff = HelloConfig::effectiveness(&move_.element(), &[defender.species.element()]);
        let raw = (move_.power() as f32 * eff * atk as f32 / def as f32) as u16;
        DamageResult {
            damage: raw.max(1),
            effectiveness: eff,
            is_miss: false,
        }
    }

    fn select_move(&self, battler: &BattlerState<Self>, _state: &BattleState<Self>) -> MoveKind {
        battler.moves.first().cloned().unwrap_or(MoveKind::Slash)
    }

    fn apply_move_effect(
        &self,
        effect: MoveEffect,
        user: &mut BattlerState<Self>,
        target: &mut BattlerState<Self>,
    ) -> EffectResult {
        match effect {
            MoveEffect::Damage => {
                let mv = user.moves.first().cloned().unwrap_or(MoveKind::Slash);
                let result = self.calculate_damage(&mv, user, target, 100, false);
                target.take_damage(result.damage);
                EffectResult::DamageDealt {
                    amount: result.damage,
                }
            }
            MoveEffect::Heal => {
                let amount = target.max_hp / 4;
                target.heal(amount);
                EffectResult::Healed { amount }
            }
            MoveEffect::StatusCondition => EffectResult::StatusInflicted,
            MoveEffect::StatChange => EffectResult::StatModified { stages: 1 },
            _ => EffectResult::NoEffect,
        }
    }

    fn create_monster(&self, species: Species, level: u8) -> BattlerState<Self> {
        let (hp, atk, def, spd, moves) = match species {
            Species::Warrior => (100u16, 15u16, 10u16, 8u16, vec![MoveKind::Slash]),
            Species::Mage => (80u16, 20u16, 5u16, 12u16, vec![MoveKind::Fireball]),
        };
        let mut stats = EnumMap::new();
        stats.set(StatId::HP, hp);
        stats.set(StatId::ATK, atk);
        stats.set(StatId::DEF, def);
        stats.set(StatId::SPD, spd + level as u16);
        BattlerState::new(species, hp, hp, stats, moves)
    }
}

// ── BattleAI ───────────────────────────────────────────────────────

impl BattleAI<HelloConfig> for HelloConfig {
    fn select_move(
        &self,
        battler: &BattlerState<HelloConfig>,
        _state: &BattleState<HelloConfig>,
    ) -> MoveKind {
        battler.moves.first().cloned().unwrap_or(MoveKind::Slash)
    }
    fn should_switch(&self, _battler: &BattlerState<HelloConfig>) -> bool {
        false
    }
    fn should_use_item(&self, _battler: &BattlerState<HelloConfig>) -> Option<ItemKind> {
        None
    }
}

// ── EffectHandler ──────────────────────────────────────────────────

impl EffectHandler<HelloConfig> for HelloConfig {
    fn handle_effect(
        &self,
        effect: MoveEffect,
        user: &mut BattlerState<HelloConfig>,
        target: &mut BattlerState<HelloConfig>,
        _provider: &HelloConfig,
    ) -> EffectResult {
        // Delegate to the BattleProvider impl.
        self.apply_move_effect(effect, user, target)
    }
}

// ── ItemProvider ──────────────────────────────────────────────────

impl ItemProvider for HelloConfig {
    type Item = ItemKind;
    type Effect = ItemEffect;
    type Monster = MonsterData;
    type CustomKind = ();

    fn item_name(&self, item: &ItemKind) -> &str {
        match item {
            ItemKind::Potion => "Potion",
            ItemKind::Elixir => "Elixir",
        }
    }
    fn item_description(&self, item: &ItemKind) -> &str {
        match item {
            ItemKind::Potion => "Restores 20 HP.",
            ItemKind::Elixir => "Cures status.",
        }
    }
    fn item_effect(&self, item: &ItemKind) -> ItemEffect {
        match item {
            ItemKind::Potion => ItemEffect::Heal(20),
            ItemKind::Elixir => ItemEffect::CureStatus,
        }
    }
    fn item_price(&self, item: &ItemKind) -> u32 {
        match item {
            ItemKind::Potion => 100,
            ItemKind::Elixir => 300,
        }
    }
    fn can_use_outside_battle(&self, _item: &ItemKind) -> bool {
        true
    }
    fn can_use_in_battle(&self, _item: &ItemKind) -> bool {
        true
    }
    fn use_on_monster(&self, item: &ItemKind, monster: &mut MonsterData) -> ItemResult {
        match self.item_effect(item) {
            ItemEffect::Heal(amount) => {
                if monster.current_hp >= monster.max_hp {
                    return ItemResult::NoEffect;
                }
                monster.current_hp = (monster.current_hp + amount).min(monster.max_hp);
                ItemResult::Used
            }
            ItemEffect::CureStatus => ItemResult::Used,
            ItemEffect::None => ItemResult::NoEffect,
        }
    }
    fn consume(&self, _item: &ItemKind) -> bool {
        true
    }
    fn item_kind(&self, item: &ItemKind) -> dotzuki_engine::items::ItemKind<()> {
        let _ = item;
        dotzuki_engine::items::ItemKind::Consumable
    }
}

// ── ShopProvider ───────────────────────────────────────────────────

impl ShopProvider for HelloConfig {
    type Item = ItemKind;
    type ShopId = ();
    fn shop_inventory(&self, _shop_id: &()) -> Vec<(ItemKind, u32)> {
        vec![(ItemKind::Potion, 100), (ItemKind::Elixir, 300)]
    }
    fn shop_name(&self, _shop_id: &()) -> &str {
        "Town Shop"
    }
}

// ── TextProvider — ASCII charmap ──────────────────────────────────

impl TextProvider for HelloConfig {
    type Char = AsciiChar;

    fn decode_byte(&self, byte: u8) -> Option<AsciiChar> {
        match byte {
            0xFE => Some(AsciiChar::Newline),
            0xFF => Some(AsciiChar::Done),
            0xFD => Some(AsciiChar::WaitInput),
            b @ 0x20..=0x7E => Some(AsciiChar::Char(b as char)),
            _ => None,
        }
    }
    fn render_char(&self, c: &AsciiChar, buffer: &mut TileBuffer) {
        if let AsciiChar::Char(ch) = c {
            let pos = buffer.cursor;
            buffer.set_tile(pos, *ch as u16, 0);
            buffer.cursor.x += 1;
        }
    }
    fn string_width(&self, text: &[AsciiChar]) -> u16 {
        text.iter()
            .filter(|c| matches!(c, AsciiChar::Char(_)))
            .count() as u16
            * 8
    }
    fn is_control_code(&self, c: &AsciiChar) -> bool {
        !matches!(c, AsciiChar::Char(_))
    }
    fn process_control(&self, c: &AsciiChar, _state: &mut DialogState) -> ControlAction {
        match c {
            AsciiChar::Newline => ControlAction::Newline,
            AsciiChar::Done => ControlAction::Done,
            AsciiChar::WaitInput => ControlAction::WaitInput,
            _ => ControlAction::None,
        }
    }
}

impl MenuProvider for HelloConfig {
    type MenuId = MenuScreen;

    fn title(&self, _menu: MenuScreen) -> &str {
        "Hello JRPG"
    }
    fn options(&self, _menu: MenuScreen) -> &[MenuOption] {
        &self.main_menu_options
    }
    fn option_count(&self, _menu: MenuScreen) -> u8 {
        self.main_menu_options.len() as u8
    }
    fn scrollable(&self, _menu: MenuScreen) -> bool {
        false
    }
    fn layout(&self, _menu: MenuScreen) -> MenuLayout {
        MenuLayout::new(5, 5, 10, 8)
    }
}

// ── SaveData — binary format: [1B name_len][0..32B name][1B level] ─

#[derive(Debug, Clone, PartialEq, Eq)]
struct GameSave {
    player_name: String,
    player_level: u8,
}

impl SaveData for GameSave {
    fn serialize(&self) -> Vec<u8> {
        let name = self.player_name.as_bytes();
        let len = name.len().min(32);
        let mut v = Vec::with_capacity(1 + len + 1);
        v.push(len as u8);
        v.extend_from_slice(&name[..len]);
        v.push(self.player_level);
        v
    }
    fn deserialize(data: &[u8]) -> Result<Self, SaveError> {
        if data.len() < 2 {
            return Err(SaveError::InvalidData);
        }
        let name_len = data[0] as usize;
        if data.len() < 1 + name_len + 1 {
            return Err(SaveError::InvalidData);
        }
        let name = String::from_utf8(data[1..1 + name_len].to_vec())
            .map_err(|_| SaveError::InvalidData)?;
        let level = data[1 + name_len];
        Ok(GameSave {
            player_name: name,
            player_level: level,
        })
    }
    fn save_size() -> usize {
        35
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Overworld collision — simple provider: only tile 0xFF is a wall
// ═══════════════════════════════════════════════════════════════════════════

struct SimpleCollision;

impl CollisionProvider<Tileset> for SimpleCollision {
    fn is_tile_passable(&self, _tileset: Tileset, tile_id: u8) -> bool {
        tile_id != 0xFF
    }
    fn check_tile_pair_collision(&self, _t: Tileset, _a: u8, _b: u8, _w: bool) -> bool {
        false
    }
    fn check_ledge_jump(&self, _t: Tileset, _f: u8, _s: u8, _tg: u8, _i: u8) -> bool {
        false
    }
    fn is_counter_tile(&self, _t: Tileset, _id: u8) -> bool {
        false
    }
    fn get_tile_at_position(&self, _t: Tileset, blk: &[u8], w: u8, x: u16, y: u16) -> u8 {
        blk.get((y as usize * w as usize + x as usize).min(blk.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    }
    fn is_door_tile(&self, _t: Tileset, _id: u8) -> bool {
        false
    }
    fn is_warp_tile(&self, _t: Tileset, _id: u8) -> bool {
        false
    }
    fn is_warp_carpet_tile_in_front(&self, _t: Tileset, _f: u8, _id: u8) -> bool {
        false
    }
    fn uses_warp_tile_in_front_check(&self, _t: Tileset) -> bool {
        false
    }
    fn check_extra_warp_special(&self, _t: Tileset, _id: u8) -> Option<bool> {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Map builder — 5×5 town with walls, floor, NPC
// ═══════════════════════════════════════════════════════════════════════════
//  Layout:  # = wall (0xFF), . = floor (0x01), N=NPC, P=player start
//  #####
//  #...#
//  #.N.#
//  #.P.#
//  #####

fn build_town_map() -> MapData<MapId, Tileset, ()> {
    let blocks: Vec<u8> = vec![
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x01, 0x01, 0xFF, 0xFF, 0x01, 0x01, 0x01, 0xFF,
        0xFF, 0x01, 0x01, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    let npcs = vec![NpcDefinition::new(
        0,
        2,
        2,
        NpcMovementType::Stationary,
        Direction::Down,
        0,
        0,
    )];
    MapData::new(
        MapId(0),
        5,
        5,
        Tileset(0),
        (),
        blocks,
        vec![],
        npcs,
        vec![],
        MapConnections::default(),
    )
}

/// Build NPC runtime states from the map's NPC definitions.
fn build_npc_states(npcs: &[NpcDefinition]) -> Vec<NpcRuntimeState> {
    npcs.iter()
        .enumerate()
        .map(|(i, def)| {
            let s = NpcRuntimeState {
                npc_index: i as u8,
                sprite_id: def.sprite_id,
                x: def.x as u16,
                y: def.y as u16,
                home_x: def.x as u16,
                home_y: def.y as u16,
                facing: def.facing,
                scripted_frame: None,
                movement_type: def.movement,
                range: def.range,
                walk_counter: 0,
                delay_counter: 0,
                text_id: def.text_id,
                defeated: false,
                visible: true,
                scripted_path: std::collections::VecDeque::new(),
                wander_axis: dotzuki_engine::overworld::NpcWanderAxis::Any,
            };
            s
        })
        .collect()
}

/// Print the map as ASCII with player position.
fn show_map(blocks: &[u8], _w: u8, px: u16, py: u16, npcs: &[NpcRuntimeState]) {
    for y in 0..5u16 {
        print!("  ");
        for x in 0..5u16 {
            let idx = (y * 5 + x) as usize;
            if px == x && py == y {
                print!("P");
            } else if npcs.iter().any(|n| n.x == x && n.y == y) {
                print!("N");
            } else if idx < blocks.len() && blocks[idx] == 0xFF {
                print!("#");
            } else {
                print!(".");
            }
        }
        println!();
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Demo functions
// ═══════════════════════════════════════════════════════════════════════════

fn demo_overworld() {
    println!("\n╔══ OVERWORLD MOVEMENT ═══════════════════╗");
    let map = build_town_map();
    let mut state = OverworldState::new(MapId(0));
    let collision = SimpleCollision;
    let npcs = build_npc_states(&map.npcs);
    let npc_positions = dotzuki_engine::overworld::get_npc_positions(&npcs);

    // Start at (3, 3) — bottom center
    state.player.x = 3;
    state.player.y = 3;
    println!("║ Initial map:");
    show_map(&map.blocks, 5, state.player.x, state.player.y, &npcs);

    // Move right → blocked by wall at (4, 3)
    let standing = collision.get_tile_at_position(Tileset(0), &map.blocks, 5, 3, 3);
    let target = collision.get_tile_at_position(Tileset(0), &map.blocks, 5, 4, 3);
    let result = try_move(
        &mut state,
        Direction::Right,
        Tileset(0),
        5,
        5,
        standing,
        target,
        &npc_positions,
        0,
        &collision,
    );
    println!("║ Move Right → {:?}", result);

    // Move up to (3, 2) — floor is passable
    let standing = collision.get_tile_at_position(Tileset(0), &map.blocks, 5, 3, 3);
    let target = collision.get_tile_at_position(Tileset(0), &map.blocks, 5, 3, 2);
    let result = try_move(
        &mut state,
        Direction::Up,
        Tileset(0),
        5,
        5,
        standing,
        target,
        &npc_positions,
        0,
        &collision,
    );
    println!("║ Move Up   → {:?}", result);
    println!(
        "║ Player at ({}, {}), facing {:?}",
        state.player.x, state.player.y, state.player.facing
    );

    // Advance the walk step
    dotzuki_engine::overworld::advance_step(&mut state);
    dotzuki_engine::overworld::advance_step(&mut state);
    // ...walking finishes after WALK_COUNTER_INIT (8) frames
    for _ in 0..8 {
        dotzuki_engine::overworld::advance_step(&mut state);
    }
    println!(
        "║ After walk: player at ({}, {})",
        state.player.x, state.player.y
    );

    // NPC interaction: check if NPC is nearby
    let interaction = dotzuki_engine::overworld::try_interact(
        &npcs,
        state.player.x,
        state.player.y,
        state.player.facing,
        Some(&map),
        &collision,
    );
    println!("║ Interact → {:?}", interaction);
    println!("╚══════════════════════════════════════════╝");
}

fn demo_dialog() {
    println!("\n╔══ NPC DIALOG ═══════════════════════════╗");
    let provider = HelloConfig::new();
    let mut engine = DialogEngine::new(provider);
    let mut buffer = TileBuffer::new(20, 18);

    // "Greetings, young hero!" + DONE
    let text: &[u8] = b"Greetings, young hero!";
    let mut full = text.to_vec();
    full.push(0xFF); // DONE

    engine.open_dialog(&full);
    while engine.is_active() {
        engine.update(&mut buffer);
    }

    print!("║ Merlin: \"");
    for i in 0..20usize {
        let t = buffer.tiles[i].tile_id;
        if (0x20..=0x7E).contains(&t) {
            print!("{}", t as u8 as char);
        }
    }
    println!("\"");
    println!("╚══════════════════════════════════════════╝");
}

fn demo_battle() {
    println!("\n╔══ BATTLE ENCOUNTER ═════════════════════╗");
    let provider = HelloConfig::new();
    let warrior = provider.create_monster(Species::Warrior, 5);
    let mage = provider.create_monster(Species::Mage, 5);

    println!(
        "║ {} (HP:{}/{}) vs {} (HP:{}/{})",
        warrior.species.name(),
        warrior.hp,
        warrior.max_hp,
        mage.species.name(),
        mage.hp,
        mage.max_hp
    );

    // Warrior attacks with Slash → super effective (2×) vs Mage
    let result = provider.calculate_damage(&MoveKind::Slash, &warrior, &mage, 100, false);
    println!(
        "║ {} uses Slash! {} damage ({}x effective)",
        warrior.species.name(),
        result.damage,
        result.effectiveness
    );

    let mut enemy = mage.clone();
    enemy.take_damage(result.damage);
    println!("║ {} HP: {} → {}", mage.species.name(), mage.hp, enemy.hp);

    // Mage attacks with Fireball → not very effective (0.5×) vs Warrior
    let result = provider.calculate_damage(&MoveKind::Fireball, &mage, &warrior, 100, false);
    println!(
        "║ {} uses Fireball! {} damage ({}x effective)",
        mage.species.name(),
        result.damage,
        result.effectiveness
    );

    let mut hero = warrior.clone();
    hero.take_damage(result.damage);
    println!(
        "║ {} HP: {} → {}",
        warrior.species.name(),
        warrior.hp,
        hero.hp
    );

    // EffectHandler
    let mut user = warrior.clone();
    let mut target = mage.clone();
    let eff_result = HelloConfig::new().handle_effect(
        MoveEffect::Damage,
        &mut user,
        &mut target,
        &HelloConfig::new(),
    );
    println!("║ EffectHandler(Damage) → {:?}", eff_result);

    // BattleAI
    let state = BattleState::new(vec![warrior.clone()], vec![mage.clone()]);
    let ai_move = BattleAI::select_move(&HelloConfig::default(), &warrior, &state);
    println!("║ BattleAI chose: {}", ai_move.name());
    println!("╚══════════════════════════════════════════╝");
}

fn demo_menu() {
    println!("\n╔══ MENU NAVIGATION ═════════════════════╗");
    let provider = HelloConfig::new();
    let mut menu = MenuSystem::new(&provider);
    menu.open(MenuScreen::Main);

    println!("║ Menu '{}' — 3 options", provider.title(MenuScreen::Main));
    for (i, opt) in provider.main_menu_options.iter().enumerate() {
        let marker = if i == menu.cursor as usize { ">" } else { " " };
        println!(
            "║  {} {}{}",
            marker,
            opt.label,
            if opt.enabled { "" } else { " (disabled)" }
        );
    }

    // Navigate down × 2
    menu.handle_input(&MenuInput {
        down: true,
        ..Default::default()
    });
    menu.handle_input(&MenuInput {
        down: true,
        ..Default::default()
    });
    println!("║ Cursor after 2× Down: {}", menu.cursor);

    // Select
    let action = menu.handle_input(&MenuInput {
        confirm: true,
        ..Default::default()
    });
    println!(
        "║ Confirm → {:?} ('{}')",
        action,
        menu.selected_option()
            .map(|o| o.label.as_str())
            .unwrap_or("?")
    );
    println!("╚══════════════════════════════════════════╝");
}

fn demo_items() {
    println!("\n╔══ ITEM SYSTEM ═════════════════════════╗");
    let provider = HelloConfig::new();

    let mut monster = MonsterData {
        species: Species::Warrior,
        max_hp: 100,
        current_hp: 30,
    };
    println!(
        "║ {} HP before: {}/{}",
        monster.species.name(),
        monster.current_hp,
        monster.max_hp
    );
    let r = provider.use_on_monster(&ItemKind::Potion, &mut monster);
    println!(
        "║ Used Potion → {:?}, HP now: {}/{}",
        r, monster.current_hp, monster.max_hp
    );

    let shop = provider.shop_inventory(&());
    println!("║ Shop inventory: {:?}", shop);
    println!("╚══════════════════════════════════════════╝");
}

fn demo_save_load() {
    println!("\n╔══ SAVE / LOAD ═════════════════════════╗");
    let storage = Box::new(InMemoryStorage::new());
    let manager = SaveManager::<GameSave>::new(storage);

    let save = GameSave {
        player_name: "Hero".to_string(),
        player_level: 7,
    };
    manager.save(SaveSlot::Slot1, &save).expect("save");
    let loaded = manager.load(SaveSlot::Slot1).expect("load");
    println!("║ Saved: {:?}", save);
    println!("║ Loaded: {:?}", loaded);
    assert_eq!(save, loaded);
    println!("║ ✓ Round-trip verified");
    println!("╚══════════════════════════════════════════╝");
}

// ═══════════════════════════════════════════════════════════════════════════
// main
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    println!("╔══════════════════════════════════════════╗");
    println!("║     Hello JRPG — Engine Demo v0.2      ║");
    println!("║  Zero pokered-* imports. Pure engine.   ║");
    println!("╚══════════════════════════════════════════╝");

    demo_overworld();
    demo_dialog();
    demo_battle();
    demo_items();
    demo_menu();
    demo_save_load();

    println!("\nAll demos complete. ✓");
}

// ═══════════════════════════════════════════════════════════════════════════
// Integration tests — cargo test --example hello_dotzuki
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use dotzuki_engine::items::Inventory;
    use dotzuki_engine::menu::MenuAction;

    // ── Test 1: Create monsters and deal type-effective damage ───────

    #[test]
    fn test_warrior_beats_mage() {
        let provider = HelloConfig::new();
        let w = provider.create_monster(Species::Warrior, 5);
        let m = provider.create_monster(Species::Mage, 5);
        assert_eq!(w.hp, 100);
        assert_eq!(m.hp, 80);

        // Slash (Physical) vs Mage (Magical) → 2×
        let r = provider.calculate_damage(&MoveKind::Slash, &w, &m, 100, false);
        assert!((r.effectiveness - 2.0).abs() < f32::EPSILON);
        assert!(r.damage > 0);
    }

    #[test]
    fn test_mage_weak_to_warrior() {
        let provider = HelloConfig::new();
        let w = provider.create_monster(Species::Warrior, 5);
        let m = provider.create_monster(Species::Mage, 5);

        // Fireball (Magical) vs Warrior (Physical) → 0.5×
        let r = provider.calculate_damage(&MoveKind::Fireball, &m, &w, 100, false);
        assert!((r.effectiveness - 0.5).abs() < f32::EPSILON);
    }

    // ── Test 2: Dialog stream processing ────────────────────────────

    #[test]
    fn test_dialog_stream() {
        let provider = HelloConfig::new();
        let mut engine = DialogEngine::new(provider);
        let mut buffer = TileBuffer::new(20, 18);

        engine.open_dialog(&[b'H', b'I', 0xFF]);
        assert!(engine.is_active());
        engine.update(&mut buffer); // H
        engine.update(&mut buffer); // I
        engine.update(&mut buffer); // DONE
        assert!(!engine.is_active());
        assert_eq!(buffer.tiles[0].tile_id, b'H' as u16);
        assert_eq!(buffer.tiles[1].tile_id, b'I' as u16);
    }

    #[test]
    fn test_dialog_newline_control() {
        let provider = HelloConfig::new();
        let mut engine = DialogEngine::new(provider);
        let mut buffer = TileBuffer::new(20, 18);

        // "A" + NEWLINE + "B" + DONE
        engine.open_dialog(&[b'A', 0xFE, b'B', 0xFF]);
        for _ in 0..4 {
            engine.update(&mut buffer);
        }
        assert_eq!(buffer.tiles[0].tile_id, b'A' as u16);
        assert_eq!(buffer.tiles[20].tile_id, b'B' as u16); // row 1, col 0
    }

    // ── Test 3: Menu navigation — cursor movement & selection ──────

    #[test]
    fn test_menu_cursor_movement() {
        let provider = HelloConfig::new();
        let mut menu = MenuSystem::new(&provider);
        menu.open(MenuScreen::Main);
        assert_eq!(menu.cursor, 0);

        let a = menu.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(a, MenuAction::Down);
        assert_eq!(menu.cursor, 1);

        let a = menu.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(a, MenuAction::Down);
        assert_eq!(menu.cursor, 2);

        // Stop at last
        let a = menu.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        assert_eq!(a, MenuAction::None);
    }

    #[test]
    fn test_menu_selection_and_cancel() {
        let provider = HelloConfig::new();
        let mut menu = MenuSystem::new(&provider);
        menu.open(MenuScreen::Main);

        // Move to "Battle"
        menu.handle_input(&MenuInput {
            down: true,
            ..Default::default()
        });
        let a = menu.handle_input(&MenuInput {
            confirm: true,
            ..Default::default()
        });
        assert_eq!(a, MenuAction::Selected(1));

        // Re-open and cancel
        menu.open(MenuScreen::Main);
        let a = menu.handle_input(&MenuInput {
            cancel: true,
            ..Default::default()
        });
        assert_eq!(a, MenuAction::Cancelled);
        assert!(!menu.is_open());
    }

    // ── Test 4: Save/Load round-trip ────────────────────────────────

    #[test]
    fn test_save_load_roundtrip() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<GameSave>::new(storage);
        let save = GameSave {
            player_name: "Hero".to_string(),
            player_level: 42,
        };

        manager.save(SaveSlot::Slot1, &save).expect("save");
        let loaded = manager.load(SaveSlot::Slot1).expect("load");
        assert_eq!(loaded, save);
    }

    #[test]
    fn test_save_empty_slot_error() {
        let storage = Box::new(InMemoryStorage::new());
        let manager = SaveManager::<GameSave>::new(storage);
        let result = manager.load(SaveSlot::Slot1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SaveError::SlotEmpty);
    }

    // ── Test 5: Item usage ──────────────────────────────────────────

    #[test]
    fn test_potion_heals() {
        let provider = HelloConfig::new();
        let mut m = MonsterData {
            species: Species::Warrior,
            max_hp: 100,
            current_hp: 50,
        };
        assert_eq!(
            provider.use_on_monster(&ItemKind::Potion, &mut m),
            ItemResult::Used
        );
        assert_eq!(m.current_hp, 70);
    }

    #[test]
    fn test_potion_no_effect_full_hp() {
        let provider = HelloConfig::new();
        let mut m = MonsterData {
            species: Species::Warrior,
            max_hp: 100,
            current_hp: 100,
        };
        assert_eq!(
            provider.use_on_monster(&ItemKind::Potion, &mut m),
            ItemResult::NoEffect
        );
    }

    // ── Test: TypeChart ─────────────────────────────────────────────

    #[test]
    fn test_physical_beats_magical() {
        let eff = HelloConfig::effectiveness(&Element::Physical, &[Element::Magical]);
        assert!((eff - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_magical_weak_to_physical() {
        let eff = HelloConfig::effectiveness(&Element::Magical, &[Element::Physical]);
        assert!((eff - 0.5).abs() < f32::EPSILON);
    }

    // ── Test: Inventory ─────────────────────────────────────────────

    #[test]
    fn test_inventory_ops() {
        let mut inv: Inventory<ItemKind, 64> = Inventory::new();
        inv.add(ItemKind::Potion, 5);
        assert!(inv.contains(&ItemKind::Potion, 3));
        assert!(!inv.contains(&ItemKind::Potion, 6));
        assert!(inv.remove(&ItemKind::Potion, 2));
        assert!(inv.contains(&ItemKind::Potion, 3));
    }
}
