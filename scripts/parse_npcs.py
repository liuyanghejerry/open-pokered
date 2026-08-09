#!/usr/bin/env python3
"""Parse NPC object_event and bg_event data from data/maps/objects/*.asm files.
Generates split Rust source files for npc_data and sign_data modules.
"""
import os, re, glob, sys

ASM_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'data', 'maps', 'objects')
OUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'crates', 'pokered-data', 'src')

# Sprite constant -> Rust SpriteId
SPRITE_MAP = {
    'SPRITE_NONE': 'None', 'SPRITE_RED': 'Red', 'SPRITE_BLUE': 'Blue',
    'SPRITE_OAK': 'Oak', 'SPRITE_YOUNGSTER': 'Youngster', 'SPRITE_MONSTER': 'Monster',
    'SPRITE_COOLTRAINER_F': 'CooltrainerF', 'SPRITE_COOLTRAINER_M': 'CooltrainerM',
    'SPRITE_LITTLE_GIRL': 'LittleGirl', 'SPRITE_BIRD': 'Bird',
    'SPRITE_MIDDLE_AGED_MAN': 'MiddleAgedMan', 'SPRITE_GAMBLER': 'Gambler',
    'SPRITE_SUPER_NERD': 'SuperNerd', 'SPRITE_GIRL': 'Girl', 'SPRITE_HIKER': 'Hiker',
    'SPRITE_BEAUTY': 'Beauty', 'SPRITE_GENTLEMAN': 'Gentleman', 'SPRITE_DAISY': 'Daisy',
    'SPRITE_BIKER': 'Biker', 'SPRITE_SAILOR': 'Sailor', 'SPRITE_COOK': 'Cook',
    'SPRITE_BIKE_SHOP_CLERK': 'BikeShopClerk', 'SPRITE_MR_FUJI': 'MrFuji',
    'SPRITE_GIOVANNI': 'Giovanni', 'SPRITE_ROCKET': 'Rocket',
    'SPRITE_CHANNELER': 'Channeler', 'SPRITE_WAITER': 'Waiter',
    'SPRITE_SILPH_WORKER_F': 'SilphWorkerF', 'SPRITE_MIDDLE_AGED_WOMAN': 'MiddleAgedWoman',
    'SPRITE_BRUNETTE_GIRL': 'BrunetteGirl', 'SPRITE_LANCE': 'Lance',
    'SPRITE_UNUSED_SCIENTIST': 'UnusedScientist', 'SPRITE_SCIENTIST': 'Scientist',
    'SPRITE_ROCKER': 'Rocker', 'SPRITE_SWIMMER': 'Swimmer',
    'SPRITE_SAFARI_ZONE_WORKER': 'SafariZoneWorker', 'SPRITE_GYM_GUIDE': 'GymGuide',
    'SPRITE_GRAMPS': 'Gramps', 'SPRITE_CLERK': 'Clerk',
    'SPRITE_FISHING_GURU': 'FishingGuru', 'SPRITE_GRANNY': 'Granny',
    'SPRITE_NURSE': 'Nurse', 'SPRITE_LINK_RECEPTIONIST': 'LinkReceptionist',
    'SPRITE_SILPH_PRESIDENT': 'SilphPresident', 'SPRITE_SILPH_WORKER_M': 'SilphWorkerM',
    'SPRITE_WARDEN': 'Warden', 'SPRITE_CAPTAIN': 'Captain', 'SPRITE_FISHER': 'Fisher',
    'SPRITE_KOGA': 'Koga', 'SPRITE_GUARD': 'Guard', 'SPRITE_UNUSED_GUARD': 'UnusedGuard',
    'SPRITE_MOM': 'Mom', 'SPRITE_BALDING_GUY': 'BaldingGuy',
    'SPRITE_LITTLE_BOY': 'LittleBoy', 'SPRITE_UNUSED_GAMEBOY_KID': 'UnusedGameboyKid',
    'SPRITE_GAMEBOY_KID': 'GameboyKid', 'SPRITE_FAIRY': 'Fairy',
    'SPRITE_AGATHA': 'Agatha', 'SPRITE_BRUNO': 'Bruno', 'SPRITE_LORELEI': 'Lorelei',
    'SPRITE_SEEL': 'Seel', 'SPRITE_POKE_BALL': 'PokeBall', 'SPRITE_FOSSIL': 'Fossil',
    'SPRITE_BOULDER': 'Boulder', 'SPRITE_PAPER': 'Paper', 'SPRITE_POKEDEX': 'Pokedex',
    'SPRITE_CLIPBOARD': 'Clipboard', 'SPRITE_SNORLAX': 'Snorlax',
    'SPRITE_UNUSED_OLD_AMBER': 'UnusedOldAmber', 'SPRITE_OLD_AMBER': 'OldAmber',
    'SPRITE_UNUSED_GAMBLER_ASLEEP_1': 'UnusedGamblerAsleep1',
    'SPRITE_UNUSED_GAMBLER_ASLEEP_2': 'UnusedGamblerAsleep2',
    'SPRITE_GAMBLER_ASLEEP': 'GamblerAsleep',
}

# Trainer class OPP_ constant -> Rust TrainerClass variant
TRAINER_MAP = {
    'OPP_NOBODY': 'Nobody', 'OPP_YOUNGSTER': 'Youngster', 'OPP_BUG_CATCHER': 'BugCatcher',
    'OPP_LASS': 'Lass', 'OPP_SAILOR': 'Sailor', 'OPP_JR_TRAINER_M': 'JrTrainerM',
    'OPP_JR_TRAINER_F': 'JrTrainerF', 'OPP_POKEMANIAC': 'Pokemaniac',
    'OPP_SUPER_NERD': 'SuperNerd', 'OPP_HIKER': 'Hiker', 'OPP_BIKER': 'Biker',
    'OPP_BURGLAR': 'Burglar', 'OPP_ENGINEER': 'Engineer',
    'OPP_UNUSED_JUGGLER': 'UnusedJuggler', 'OPP_FISHER': 'Fisher',
    'OPP_SWIMMER': 'Swimmer', 'OPP_CUE_BALL': 'CueBall', 'OPP_GAMBLER': 'Gambler',
    'OPP_BEAUTY': 'Beauty', 'OPP_PSYCHIC_TR': 'PsychicTr', 'OPP_ROCKER': 'Rocker',
    'OPP_JUGGLER': 'Juggler', 'OPP_TAMER': 'Tamer', 'OPP_BIRD_KEEPER': 'BirdKeeper',
    'OPP_BLACKBELT': 'Blackbelt', 'OPP_RIVAL1': 'Rival1', 'OPP_PROF_OAK': 'ProfOak',
    'OPP_CHIEF': 'Chief', 'OPP_SCIENTIST': 'Scientist', 'OPP_GIOVANNI': 'Giovanni',
    'OPP_ROCKET': 'Rocket', 'OPP_COOLTRAINER_M': 'CooltrainerM',
    'OPP_COOLTRAINER_F': 'CooltrainerF', 'OPP_BRUNO': 'Bruno', 'OPP_BROCK': 'Brock',
    'OPP_MISTY': 'Misty', 'OPP_LT_SURGE': 'LtSurge', 'OPP_ERIKA': 'Erika',
    'OPP_KOGA': 'Koga', 'OPP_BLAINE': 'Blaine', 'OPP_SABRINA': 'Sabrina',
    'OPP_GENTLEMAN': 'Gentleman', 'OPP_RIVAL2': 'Rival2', 'OPP_RIVAL3': 'Rival3',
    'OPP_LORELEI': 'Lorelei', 'OPP_CHANNELER': 'Channeler', 'OPP_AGATHA': 'Agatha',
    'OPP_LANCE': 'Lance',
}

# Item constant -> Rust ItemId variant
ITEM_MAP = {
    'NO_ITEM': 'NoItem', 'MASTER_BALL': 'MasterBall', 'ULTRA_BALL': 'UltraBall',
    'GREAT_BALL': 'GreatBall', 'POKE_BALL': 'PokeBall', 'TOWN_MAP': 'TownMap',
    'BICYCLE': 'Bicycle', 'SURFBOARD': 'Surfboard', 'SAFARI_BALL': 'SafariBall',
    'POKEDEX': 'Pokedex', 'MOON_STONE': 'MoonStone', 'ANTIDOTE': 'Antidote',
    'BURN_HEAL': 'BurnHeal', 'ICE_HEAL': 'IceHeal', 'AWAKENING': 'Awakening',
    'PARLYZ_HEAL': 'ParlyzHeal', 'FULL_RESTORE': 'FullRestore',
    'MAX_POTION': 'MaxPotion', 'HYPER_POTION': 'HyperPotion',
    'SUPER_POTION': 'SuperPotion', 'POTION': 'Potion',
    'ESCAPE_ROPE': 'EscapeRope', 'REPEL': 'Repel', 'OLD_AMBER': 'OldAmber',
    'FIRE_STONE': 'FireStone', 'THUNDER_STONE': 'ThunderStone',
    'WATER_STONE': 'WaterStone', 'HP_UP': 'HpUp', 'PROTEIN': 'Protein',
    'IRON': 'Iron', 'CARBOS': 'Carbos', 'CALCIUM': 'Calcium',
    'RARE_CANDY': 'RareCandy', 'DOME_FOSSIL': 'DomeFossil',
    'HELIX_FOSSIL': 'HelixFossil', 'SECRET_KEY': 'SecretKey',
    'BIKE_VOUCHER': 'BikeVoucher', 'X_ACCURACY': 'XAccuracy',
    'LEAF_STONE': 'LeafStone', 'CARD_KEY': 'CardKey', 'NUGGET': 'Nugget',
    'POKE_DOLL': 'PokeDoll', 'FULL_HEAL': 'FullHeal', 'REVIVE': 'Revive',
    'MAX_REVIVE': 'MaxRevive', 'GUARD_SPEC': 'GuardSpec',
    'SUPER_REPEL': 'SuperRepel', 'MAX_REPEL': 'MaxRepel', 'DIRE_HIT': 'DireHit',
    'COIN': 'Coin', 'FRESH_WATER': 'FreshWater', 'SODA_POP': 'SodaPop',
    'LEMONADE': 'Lemonade', 'S_S_TICKET': 'SsTicket', 'GOLD_TEETH': 'GoldTeeth',
    'X_ATTACK': 'XAttack', 'X_DEFEND': 'XDefend', 'X_SPEED': 'XSpeed',
    'X_SPECIAL': 'XSpecial', 'COIN_CASE': 'CoinCase', 'OAKS_PARCEL': 'OaksParcel',
    'ITEMFINDER': 'Itemfinder', 'SILPH_SCOPE': 'SilphScope',
    'POKE_FLUTE': 'PokeFlute', 'LIFT_KEY': 'LiftKey', 'EXP_ALL': 'ExpAll',
    'OLD_ROD': 'OldRod', 'GOOD_ROD': 'GoodRod', 'SUPER_ROD': 'SuperRod',
    'PP_UP': 'PpUp', 'ETHER': 'Ether', 'MAX_ETHER': 'MaxEther',
    'ELIXER': 'Elixer', 'MAX_ELIXER': 'MaxElixer',
}

# TM items: TM_MEGA_PUNCH -> item hex value 0xC9, etc.
TM_ITEMS = [
    'MEGA_PUNCH', 'RAZOR_WIND', 'SWORDS_DANCE', 'WHIRLWIND', 'MEGA_KICK',
    'TOXIC', 'HORN_DRILL', 'BODY_SLAM', 'TAKE_DOWN', 'DOUBLE_EDGE',
    'BUBBLEBEAM', 'WATER_GUN', 'ICE_BEAM', 'BLIZZARD', 'HYPER_BEAM',
    'PAY_DAY', 'SUBMISSION', 'COUNTER', 'SEISMIC_TOSS', 'RAGE',
    'MEGA_DRAIN', 'SOLARBEAM', 'DRAGON_RAGE', 'THUNDERBOLT', 'THUNDER',
    'EARTHQUAKE', 'FISSURE', 'DIG', 'PSYCHIC_M', 'TELEPORT',
    'MIMIC', 'DOUBLE_TEAM', 'REFLECT', 'BIDE', 'METRONOME',
    'SELFDESTRUCT', 'EGG_BOMB', 'FIRE_BLAST', 'SWIFT', 'SKULL_BASH',
    'SOFTBOILED', 'DREAM_EATER', 'SKY_ATTACK', 'REST', 'THUNDER_WAVE',
    'PSYWAVE', 'EXPLOSION', 'ROCK_SLIDE', 'TRI_ATTACK', 'SUBSTITUTE',
]
# Build TM_X -> hex value map
for i, name in enumerate(TM_ITEMS):
    ITEM_MAP[f'TM_{name}'] = f'__TM_{i+1:02d}__'  # placeholder, will handle specially

# HM items
HM_ITEMS = ['CUT', 'FLY', 'SURF', 'STRENGTH', 'FLASH']
for i, name in enumerate(HM_ITEMS):
    ITEM_MAP[f'HM_{name}'] = f'__HM_{i+1:02d}__'

# ASM map filename -> Rust MapId variant
# Build from the ASM filenames by converting to PascalCase
def asm_filename_to_map_id(filename):
    """Convert e.g. 'PalletTown.asm' -> 'PalletTown'"""
    return filename.replace('.asm', '')

# Direction constant -> (movement_type, range_or_direction)
# WALK=$FE, STAY=$FF
# ANY_DIR=$00, UP_DOWN=$01, LEFT_RIGHT=$02
# DOWN=$D0, UP=$D1, LEFT=$D2, RIGHT=$D3, NONE=$FF
DIRECTION_MAP = {
    'ANY_DIR': ('Wander', 'Down', 0),    # walks randomly, default facing down
    'UP_DOWN': ('Wander', 'Down', 1),     # walks up/down
    'LEFT_RIGHT': ('Wander', 'Left', 2),  # walks left/right
    'DOWN': ('Stationary', 'Down', 0),
    'UP': ('Stationary', 'Up', 0),
    'LEFT': ('Stationary', 'Left', 0),
    'RIGHT': ('Stationary', 'Right', 0),
    'NONE': ('Stationary', 'Down', 0),    # NONE = faces down, stationary
}

def parse_object_event(args, index):
    """Parse one object_event line's comma-separated args into an NPC dict."""
    if len(args) < 6:
        return None
    x = int(args[0])
    y = int(args[1])
    sprite = args[2].strip()
    walk_stay = args[3].strip()
    dir_const = args[4].strip()
    text_const = args[5].strip()

    sprite_rust = SPRITE_MAP.get(sprite, sprite)

    if walk_stay == 'WALK':
        mv_type, facing, range_code = DIRECTION_MAP.get(dir_const, ('Wander', 'Down', 0))
    else:
        mv_type, facing, range_code = DIRECTION_MAP.get(dir_const, ('Stationary', 'Down', 0))

    npc = {
        'x': x, 'y': y,
        'sprite': sprite_rust,
        'movement': mv_type,
        'facing': facing,
        'range': range_code,
        'text_id': index,
        'is_trainer': False,
        'trainer_class': 'Nobody',
        'trainer_set': 0,
        'item': 'NoItem',
        'is_tm_hm': False,
        'tm_hm_num': 0,
    }

    if len(args) == 8:
        trainer_const = args[6].strip()
        # Only mark as trainer if the constant is a real OPP_ trainer class.
        # 8-arg object_events with a Pokémon species name (e.g. VOLTORB, ZAPDOS)
        # are wild encounter trap NPCs, not trainers.
        if trainer_const in TRAINER_MAP:
            npc['is_trainer'] = True
            npc['trainer_class'] = TRAINER_MAP[trainer_const]
            npc['trainer_set'] = int(args[7])
        else:
            # Wild Pokémon encounter NPC (e.g. PowerPlant Voltorb/Electrode/Zapdos)
            npc['is_trainer'] = False
            npc['trainer_class'] = 'Nobody'
            npc['trainer_set'] = 0
    elif len(args) == 7:
        item_const = args[6].strip()
        item_rust = ITEM_MAP.get(item_const, item_const)
        if item_rust.startswith('__TM_'):
            npc['is_tm_hm'] = True
            npc['tm_hm_num'] = int(item_rust[5:7])
            npc['item'] = 'NoItem'
        elif item_rust.startswith('__HM_'):
            npc['is_tm_hm'] = True
            npc['tm_hm_num'] = 50 + int(item_rust[5:7])
            npc['item'] = 'NoItem'
        else:
            npc['item'] = item_rust

    return npc


def parse_file(filepath):
    """Parse a single .asm map object file. Returns (npcs, signs)."""
    npcs = []
    signs = []
    in_objects = False
    in_bg = False

    with open(filepath) as f:
        for raw_line in f:
            line = raw_line.strip()
            if line.startswith('def_object_events'):
                in_objects = True
                in_bg = False
                continue
            if line.startswith('def_bg_events'):
                in_bg = True
                in_objects = False
                continue
            if line.startswith('def_warp_events') or line.startswith('def_warps_to'):
                in_objects = False
                in_bg = False
                continue

            if in_bg and line.startswith('bg_event'):
                args = [a.strip() for a in line[len('bg_event'):].split(',')]
                if len(args) >= 3:
                    signs.append({'x': int(args[0]), 'y': int(args[1]), 'text_id': len(signs) + 1})

            if in_objects and line.startswith('object_event'):
                args = [a.strip() for a in line[len('object_event'):].split(',')]
                npc = parse_object_event(args, len(npcs) + 1)
                if npc:
                    npcs.append(npc)

    return npcs, signs


SPRITE_TO_U8 = {}
sprite_vals = [
    ('None', 0x00), ('Red', 0x01), ('Blue', 0x02), ('Oak', 0x03),
    ('Youngster', 0x04), ('Monster', 0x05), ('CooltrainerF', 0x06),
    ('CooltrainerM', 0x07), ('LittleGirl', 0x08), ('Bird', 0x09),
    ('MiddleAgedMan', 0x0A), ('Gambler', 0x0B), ('SuperNerd', 0x0C),
    ('Girl', 0x0D), ('Hiker', 0x0E), ('Beauty', 0x0F),
    ('Gentleman', 0x10), ('Daisy', 0x11), ('Biker', 0x12),
    ('Sailor', 0x13), ('Cook', 0x14), ('BikeShopClerk', 0x15),
    ('MrFuji', 0x16), ('Giovanni', 0x17), ('Rocket', 0x18),
    ('Channeler', 0x19), ('Waiter', 0x1A), ('SilphWorkerF', 0x1B),
    ('MiddleAgedWoman', 0x1C), ('BrunetteGirl', 0x1D), ('Lance', 0x1E),
    ('UnusedScientist', 0x1F), ('Scientist', 0x20), ('Rocker', 0x21),
    ('Swimmer', 0x22), ('SafariZoneWorker', 0x23), ('GymGuide', 0x24),
    ('Gramps', 0x25), ('Clerk', 0x26), ('FishingGuru', 0x27),
    ('Granny', 0x28), ('Nurse', 0x29), ('LinkReceptionist', 0x2A),
    ('SilphPresident', 0x2B), ('SilphWorkerM', 0x2C), ('Warden', 0x2D),
    ('Captain', 0x2E), ('Fisher', 0x2F), ('Koga', 0x30),
    ('Guard', 0x31), ('UnusedGuard', 0x32), ('Mom', 0x33),
    ('BaldingGuy', 0x34), ('LittleBoy', 0x35), ('UnusedGameboyKid', 0x36),
    ('GameboyKid', 0x37), ('Fairy', 0x38), ('Agatha', 0x39),
    ('Bruno', 0x3A), ('Lorelei', 0x3B), ('Seel', 0x3C),
    ('PokeBall', 0x3D), ('Fossil', 0x3E), ('Boulder', 0x3F),
    ('Paper', 0x40), ('Pokedex', 0x41), ('Clipboard', 0x42),
    ('Snorlax', 0x43), ('UnusedOldAmber', 0x44), ('OldAmber', 0x45),
    ('UnusedGamblerAsleep1', 0x46), ('UnusedGamblerAsleep2', 0x47),
    ('GamblerAsleep', 0x48),
]
for name, val in sprite_vals:
    SPRITE_TO_U8[name] = val

TRAINER_TO_U8 = {
    'Nobody': 0, 'Youngster': 1, 'BugCatcher': 2, 'Lass': 3, 'Sailor': 4,
    'JrTrainerM': 5, 'JrTrainerF': 6, 'Pokemaniac': 7, 'SuperNerd': 8,
    'Hiker': 9, 'Biker': 10, 'Burglar': 11, 'Engineer': 12,
    'UnusedJuggler': 13, 'Fisher': 14, 'Swimmer': 15, 'CueBall': 16,
    'Gambler': 17, 'Beauty': 18, 'PsychicTr': 19, 'Rocker': 20,
    'Juggler': 21, 'Tamer': 22, 'BirdKeeper': 23, 'Blackbelt': 24,
    'Rival1': 25, 'ProfOak': 26, 'Chief': 27, 'Scientist': 28,
    'Giovanni': 29, 'Rocket': 30, 'CooltrainerM': 31, 'CooltrainerF': 32,
    'Bruno': 33, 'Brock': 34, 'Misty': 35, 'LtSurge': 36, 'Erika': 37,
    'Koga': 38, 'Blaine': 39, 'Sabrina': 40, 'Gentleman': 41,
    'Rival2': 42, 'Rival3': 43, 'Lorelei': 44, 'Channeler': 45,
    'Agatha': 46, 'Lance': 47,
}

ITEM_TO_U8 = {
    'NoItem': 0x00, 'MasterBall': 0x01, 'UltraBall': 0x02, 'GreatBall': 0x03,
    'PokeBall': 0x04, 'TownMap': 0x05, 'Bicycle': 0x06, 'Surfboard': 0x07,
    'SafariBall': 0x08, 'Pokedex': 0x09, 'MoonStone': 0x0A, 'Antidote': 0x0B,
    'BurnHeal': 0x0C, 'IceHeal': 0x0D, 'Awakening': 0x0E, 'ParlyzHeal': 0x0F,
    'FullRestore': 0x10, 'MaxPotion': 0x11, 'HyperPotion': 0x12,
    'SuperPotion': 0x13, 'Potion': 0x14, 'EscapeRope': 0x1D, 'Repel': 0x1E,
    'OldAmber': 0x1F, 'FireStone': 0x20, 'ThunderStone': 0x21,
    'WaterStone': 0x22, 'HpUp': 0x23, 'Protein': 0x24, 'Iron': 0x25,
    'Carbos': 0x26, 'Calcium': 0x27, 'RareCandy': 0x28, 'DomeFossil': 0x29,
    'HelixFossil': 0x2A, 'SecretKey': 0x2B, 'BikeVoucher': 0x2D,
    'XAccuracy': 0x2E, 'LeafStone': 0x2F, 'CardKey': 0x30, 'Nugget': 0x31,
    'PokeDoll': 0x33, 'FullHeal': 0x34, 'Revive': 0x35, 'MaxRevive': 0x36,
    'GuardSpec': 0x37, 'SuperRepel': 0x38, 'MaxRepel': 0x39, 'DireHit': 0x3A,
    'Coin': 0x3B, 'FreshWater': 0x3C, 'SodaPop': 0x3D, 'Lemonade': 0x3E,
    'SsTicket': 0x3F, 'GoldTeeth': 0x40, 'XAttack': 0x41, 'XDefend': 0x42,
    'XSpeed': 0x43, 'XSpecial': 0x44, 'CoinCase': 0x45, 'OaksParcel': 0x46,
    'Itemfinder': 0x47, 'SilphScope': 0x48, 'PokeFlute': 0x49, 'LiftKey': 0x4A,
    'ExpAll': 0x4B, 'OldRod': 0x4C, 'GoodRod': 0x4D, 'SuperRod': 0x4E,
    'PpUp': 0x4F, 'Ether': 0x50, 'MaxEther': 0x51, 'Elixer': 0x52,
    'MaxElixer': 0x53,
}

MOVEMENT_MAP = {'Stationary': 0, 'Wander': 1, 'FixedPath': 2, 'FacePlayer': 3}
FACING_MAP = {'Down': 0, 'Up': 1, 'Left': 2, 'Right': 3}


def npc_to_rust(npc):
    sprite_u8 = SPRITE_TO_U8.get(npc['sprite'], 0)
    trainer_u8 = TRAINER_TO_U8.get(npc['trainer_class'], 0)
    item_u8 = ITEM_TO_U8.get(npc['item'], 0)
    if npc['is_tm_hm']:
        tm_num = npc['tm_hm_num']
        if tm_num <= 50:
            item_u8 = 0xC8 + tm_num
        else:
            item_u8 = 0xC3 + (tm_num - 50)
    mv = MOVEMENT_MAP.get(npc['movement'], 0)
    face = FACING_MAP.get(npc['facing'], 0)
    return (
        f"    NpcEntry {{\n"
        f"        sprite_id: 0x{sprite_u8:02X}, x: {npc['x']}, y: {npc['y']},\n"
        f"        movement: NpcMovement({mv}), facing: NpcFacing({face}),\n"
        f"        range: {npc['range']}, text_id: {npc['text_id']},\n"
        f"        is_trainer: {'true' if npc['is_trainer'] else 'false'},\n"
        f"        trainer_class: {trainer_u8}, trainer_set: {npc['trainer_set']},\n"
        f"        item_id: 0x{item_u8:02X},\n"
        f"    }}"
    )


def sign_to_rust(sign):
    return (
        f"    SignEntry {{ x: {sign['x']}, y: {sign['y']}, text_id: {sign['text_id']} }}"
    )


def collect_all_maps():
    asm_files = sorted(glob.glob(os.path.join(ASM_DIR, '*.asm')))
    all_maps = []
    for fpath in asm_files:
        fname = os.path.basename(fpath)
        map_id = asm_filename_to_map_id(fname)
        npcs, signs = parse_file(fpath)
        all_maps.append((map_id, npcs, signs))
    return all_maps


PART_HEADER = (
    "use crate::npc_data::{NpcEntry, NpcMovement, NpcFacing};\n"
    "\n"
)


def write_npc_part(part_num, map_entries, out_dir):
    path = os.path.join(out_dir, f'npc_data_part{part_num}.rs')
    lines = [PART_HEADER]
    for map_id, npcs in map_entries:
        label = map_id.upper().replace(' ', '')
        if not npcs:
            lines.append(f"pub static NPCS_{label}: [NpcEntry; 0] = [];\n\n")
        else:
            entries = ',\n'.join(npc_to_rust(n) for n in npcs)
            lines.append(f"pub static NPCS_{label}: [NpcEntry; {len(npcs)}] = [\n{entries},\n];\n\n")
    with open(path, 'w') as f:
        f.write(''.join(lines))
    print(f"  wrote {path} ({len(map_entries)} maps)")


def write_sign_part(part_num, map_entries, out_dir):
    path = os.path.join(out_dir, f'sign_data_part{part_num}.rs')
    lines = ["use crate::sign_data::SignEntry;\n\n"]
    for map_id, signs in map_entries:
        label = map_id.upper()
        if not signs:
            lines.append(f"pub static SIGNS_{label}: [SignEntry; 0] = [];\n\n")
        else:
            entries = ',\n'.join(sign_to_rust(s) for s in signs)
            lines.append(f"pub static SIGNS_{label}: [SignEntry; {len(signs)}] = [\n{entries},\n];\n\n")
    with open(path, 'w') as f:
        f.write(''.join(lines))
    print(f"  wrote {path} ({len(map_entries)} maps)")


def generate_all(maps_per_part=50):
    all_maps = collect_all_maps()
    print(f"Parsed {len(all_maps)} maps total")

    total_npcs = sum(len(npcs) for _, npcs, _ in all_maps)
    total_signs = sum(len(signs) for _, _, signs in all_maps)
    print(f"  {total_npcs} NPCs, {total_signs} signs")

    npc_entries = [(mid, npcs) for mid, npcs, _ in all_maps]
    sign_entries = [(mid, signs) for mid, _, signs in all_maps]

    npc_parts = []
    for i in range(0, len(npc_entries), maps_per_part):
        chunk = npc_entries[i:i+maps_per_part]
        part_num = i // maps_per_part + 1
        write_npc_part(part_num, chunk, OUT_DIR)
        npc_parts.append((part_num, chunk))

    sign_parts = []
    for i in range(0, len(sign_entries), maps_per_part):
        chunk = sign_entries[i:i+maps_per_part]
        part_num = i // maps_per_part + 1
        write_sign_part(part_num, chunk, OUT_DIR)
        sign_parts.append((part_num, chunk))

    write_npc_mod(npc_parts, all_maps, OUT_DIR)
    write_sign_mod(sign_parts, all_maps, OUT_DIR)


def write_npc_mod(parts, all_maps, out_dir):
    path = os.path.join(out_dir, 'npc_data.rs')
    lines = []
    for part_num, _ in parts:
        lines.append(f"#[path = \"npc_data_part{part_num}.rs\"]\n")
        lines.append(f"mod npc_data_part{part_num};\n")
    lines.append("\nuse crate::maps::MapId;\n\n")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n")
    lines.append("pub struct NpcMovement(pub u8);\n\n")
    lines.append("impl NpcMovement {\n")
    lines.append("    pub const STATIONARY: Self = Self(0);\n")
    lines.append("    pub const WANDER: Self = Self(1);\n")
    lines.append("    pub const FIXED_PATH: Self = Self(2);\n")
    lines.append("    pub const FACE_PLAYER: Self = Self(3);\n")
    lines.append("}\n\n")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n")
    lines.append("pub struct NpcFacing(pub u8);\n\n")
    lines.append("impl NpcFacing {\n")
    lines.append("    pub const DOWN: Self = Self(0);\n")
    lines.append("    pub const UP: Self = Self(1);\n")
    lines.append("    pub const LEFT: Self = Self(2);\n")
    lines.append("    pub const RIGHT: Self = Self(3);\n")
    lines.append("}\n\n")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n")
    lines.append("pub struct NpcEntry {\n")
    lines.append("    pub sprite_id: u8,\n")
    lines.append("    pub x: u8,\n")
    lines.append("    pub y: u8,\n")
    lines.append("    pub movement: NpcMovement,\n")
    lines.append("    pub facing: NpcFacing,\n")
    lines.append("    pub range: u8,\n")
    lines.append("    pub text_id: u8,\n")
    lines.append("    pub is_trainer: bool,\n")
    lines.append("    pub trainer_class: u8,\n")
    lines.append("    pub trainer_set: u8,\n")
    lines.append("    pub item_id: u8,\n")
    lines.append("}\n\n")
    lines.append("pub fn get_map_npcs(map: MapId) -> &'static [NpcEntry] {\n")
    lines.append("    match map {\n")
    for map_id, npcs, _ in all_maps:
        if npcs:
            label = map_id.upper()
            part_num = next(pn for pn, chunk in parts if any(m == map_id for m, _ in chunk))
            lines.append(f"        MapId::{map_id} => &npc_data_part{part_num}::NPCS_{label},\n")
    lines.append("        _ => &[],\n")
    lines.append("    }\n}\n")
    with open(path, 'w') as f:
        f.write(''.join(lines))
    print(f"  wrote {path}")


def write_sign_mod(parts, all_maps, out_dir):
    path = os.path.join(out_dir, 'sign_data.rs')
    lines = []
    for part_num, _ in parts:
        lines.append(f"#[path = \"sign_data_part{part_num}.rs\"]\n")
        lines.append(f"mod sign_data_part{part_num};\n")
    lines.append("\nuse crate::maps::MapId;\n\n")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n")
    lines.append("pub struct SignEntry {\n")
    lines.append("    pub x: u8,\n")
    lines.append("    pub y: u8,\n")
    lines.append("    pub text_id: u8,\n")
    lines.append("}\n\n")
    lines.append("pub fn get_map_signs(map: MapId) -> &'static [SignEntry] {\n")
    lines.append("    match map {\n")
    for map_id, _, signs in all_maps:
        if signs:
            label = map_id.upper()
            part_num = next(pn for pn, chunk in parts if any(m == map_id for m, _ in chunk))
            lines.append(f"        MapId::{map_id} => &sign_data_part{part_num}::SIGNS_{label},\n")
    lines.append("        _ => &[],\n")
    lines.append("    }\n}\n")
    with open(path, 'w') as f:
        f.write(''.join(lines))
    print(f"  wrote {path}")


if __name__ == '__main__':
    generate_all()
