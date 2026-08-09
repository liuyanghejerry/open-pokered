# Pokémon Red/Blue Move Animation Reference

This document describes all 203 move animations from the original Pokémon Red/Blue assembly.
Each animation is a sequence of commands that play subanimations or trigger special effects.

## Animation Command Format

Each command is one of:

### SubAnimation Command
```
battle_anim SOUND_ID, SUBANIM_ID, TILESET, DELAY
```
- **SOUND_ID**: Move sound to play (e.g., `POUND`, `FIRE_PUNCH`). `NO_MOVE` = no sound.
- **SUBANIM_ID**: Subanimation to play (e.g., `SUBANIM_0_STAR_TWICE`)
- **TILESET**: Which tileset to use (0 = normal, 1 = alternate, 2 = trade)
- **DELAY**: Frames to wait after each subanimation frame

### Special Effect Command
```
battle_anim SOUND_ID, SE_EFFECT
```
- **SOUND_ID**: Move sound to play. `NO_MOVE` = no sound.
- **SE_EFFECT**: Special effect ID (e.g., `SE_SHAKE_SCREEN`, `SE_DARK_SCREEN_FLASH`)

## Special Effects Reference

| Effect | ID | Description |
|--------|-----|-------------|
| SE_WAVY_SCREEN | $D8 | Screen distortion wave effect |
| SE_SUBSTITUTE_MON | $D9 | Transform mon into substitute doll |
| SE_SHAKE_BACK_AND_FORTH | $DA | Shake sprite back and forth |
| SE_SLIDE_ENEMY_MON_OFF | $DB | Slide enemy sprite off screen |
| SE_SHOW_ENEMY_MON_PIC | $DC | Show enemy mon sprite |
| SE_SHOW_MON_PIC | $DD | Show player mon sprite |
| SE_BLINK_ENEMY_MON | $DE | Blink enemy mon sprite |
| SE_HIDE_ENEMY_MON_PIC | $DF | Hide enemy mon sprite |
| SE_FLASH_ENEMY_MON_PIC | $E0 | Flash enemy mon (unused) |
| SE_DELAY_ANIMATION_10 | $E1 | Wait 10 frames |
| SE_SPIRAL_BALLS_INWARD | $E2 | Spiral balls inward effect |
| SE_SHAKE_ENEMY_HUD_2 | $E3 | Shake enemy HUD variant 2 (unused) |
| SE_SHAKE_ENEMY_HUD | $E4 | Shake enemy HUD |
| SE_SLIDE_MON_HALF_OFF | $E5 | Slide player mon halfway off |
| SE_PETALS_FALLING | $E6 | Petals falling effect |
| SE_LEAVES_FALLING | $E7 | Leaves falling effect |
| SE_TRANSFORM_MON | $E8 | Transform into another mon |
| SE_SLIDE_MON_DOWN_AND_HIDE | $E9 | Slide down and hide (Acid Armor) |
| SE_MINIMIZE_MON | $EA | Shrink mon sprite |
| SE_BOUNCE_UP_AND_DOWN | $EB | Bounce sprite up and down |
| SE_SHOOT_MANY_BALLS_UPWARD | $EC | Shoot many balls up (unused) |
| SE_SHOOT_BALLS_UPWARD | $ED | Shoot balls upward |
| SE_SQUISH_MON_PIC | $EE | Squish/compress mon sprite |
| SE_HIDE_MON_PIC | $EF | Hide player mon sprite |
| SE_LIGHT_SCREEN_PALETTE | $F0 | Light screen palette effect |
| SE_RESET_MON_POSITION | $F1 | Reset mon to original position |
| SE_MOVE_MON_HORIZONTALLY | $F2 | Move mon horizontally |
| SE_BLINK_MON | $F3 | Blink player mon sprite |
| SE_SLIDE_MON_OFF | $F4 | Slide player mon off screen |
| SE_FLASH_MON_PIC | $F5 | Flash player mon sprite |
| SE_SLIDE_MON_DOWN | $F6 | Slide mon downward |
| SE_SLIDE_MON_UP | $F7 | Slide mon upward |
| SE_FLASH_SCREEN_LONG | $F8 | Flash screen (long duration) |
| SE_DARKEN_MON_PALETTE | $F9 | Darken mon palette |
| SE_WATER_DROPLETS_EVERYWHERE | $FA | Water droplets everywhere |
| SE_SHAKE_SCREEN | $FB | Shake screen |
| SE_RESET_SCREEN_PALETTE | $FC | Reset screen palette |
| SE_DARK_SCREEN_PALETTE | $FD | Dark screen palette |
| SE_DARK_SCREEN_FLASH | $FE | Dark screen flash |

## SubAnimations Reference

| SubAnim | ID | Type | Description |
|---------|-----|------|-------------|
| SUBANIM_0_STAR | 00 | HFLIP | Single star hit |
| SUBANIM_0_STAR_TWICE | 01 | HFLIP | Two star hits |
| SUBANIM_0_STAR_THRICE | 02 | HFLIP | Three star hits |
| SUBANIM_0_STAR_DESCENDING | 03 | HFLIP | Stars descending |
| SUBANIM_1_STAR_BIG_MOVING | 04 | HFLIP | Big star moving across |
| SUBANIM_1_STAR_BIG | 05 | HFLIP | Single big star |
| SUBANIM_0_BALL_TOSS_HIGH | 06 | NORMAL | Pokéball toss (high arc) |
| SUBANIM_0_BALL_TOSS_MIDDLE | 07 | NORMAL | Pokéball toss (middle arc) |
| SUBANIM_0_BALL_TOSS_LOW | 08 | NORMAL | Pokéball toss (low arc) |
| SUBANIM_0_BALL_SHAKE_ENEMY | 09 | NORMAL | Pokéball shaking |
| SUBANIM_0_BALL_POOF_ENEMY | 0A | HFLIP | Pokéball poof |
| SUBANIM_0_BALL_BLOCK | 0B | NORMAL | Pokéball blocked |
| SUBANIM_1_FLAME_COLUMN_1 | 0C | HFLIP | Flame column stage 1 |
| SUBANIM_1_FLAME_COLUMN_2 | 0D | HFLIP | Flame column stage 2 |
| SUBANIM_1_FLAME_COLUMN_3 | 0E | HFLIP | Flame column stage 3 |
| SUBANIM_0_SCRATCHES | 0F | HFLIP | Scratch marks |
| SUBANIM_1_TORNADO | 10 | REVERSE | Tornado/spiral |
| SUBANIM_1_FLAMES | 11 | HFLIP | Flames |
| SUBANIM_0_HEART_1_MUSIC | 12 | COORDFLIP | Heart/music notes |
| SUBANIM_1_BLOB_TOSS | 13 | REVERSE | Blob toss (Acid) |
| SUBANIM_1_BLOB_DRIP_ENEMY | 14 | HFLIP | Blob dripping |
| SUBANIM_1_SHOUT | 15 | HVFLIP | Shout waves |
| SUBANIM_0_SLICE | 16 | HFLIP | Slice/cut effect |
| SUBANIM_0_BIRDIES_CIRCLING_ENEMY | 17 | HFLIP | Birdies circling |
| SUBANIM_1_SWORDS_CIRCLING | 18 | ENEMY | Swords circling |
| SUBANIM_1_CLOUD_TOSS | 19 | REVERSE | Cloud toss |
| SUBANIM_0_WATER_COLUMNS | 1A | HFLIP | Water columns |
| SUBANIM_1_SEED_TOSS | 1B | REVERSE | Seed toss |
| SUBANIM_1_SEED_LAND | 1C | HFLIP | Seed landing |
| SUBANIM_0_ROCKS_LIFT | 1D | ENEMY | Rocks lifting |
| SUBANIM_0_ROCKS_TOSS | 1E | NORMAL | Rocks toss |
| SUBANIM_1_FLAME_BEAM | 1F | REVERSE | Flame beam |
| SUBANIM_1_FLAME_STAR | 20 | HFLIP | Flame star |
| SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY | 21 | HFLIP | Circles/squares centering |
| SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK | 22 | REVERSE | Circle/square toss back |
| SUBANIM_0_BIND | 23 | HFLIP | Bind/wrap effect |
| SUBANIM_0_STATUS_PARALYZED | 24 | ENEMY | Paralysis status |
| SUBANIM_0_STATUS_CONFUSED | 25 | COORDFLIP | Confusion status (player) |
| SUBANIM_0_STATUS_CONFUSED_ENEMY | 26 | NORMAL | Confusion status (enemy) |
| SUBANIM_0_STATUS_POISONED | 27 | ENEMY | Poison status |
| SUBANIM_1_SAND | 28 | HVFLIP | Sand effect |
| SUBANIM_1_LIGHTNING_BALL | 29 | HFLIP | Lightning ball |
| SUBANIM_0_SLICE_BOTH_SIDES | 2A | HFLIP | Slice both sides |
| SUBANIM_1_LIGHTNING | 2B | HFLIP | Lightning bolt |
| SUBANIM_0_WATER_DROPLETS | 2C | HFLIP | Water droplets |
| SUBANIM_0_CIRCLES_CENTERING | 2D | COORDFLIP | Circles centering |
| SUBANIM_0_BEAM | 2E | HVFLIP | Beam effect |
| SUBANIM_0_ICE_RISE | 2F | HFLIP | Ice rising |
| SUBANIM_0_ROCKS_FALL_ENEMY | 30 | HFLIP | Rocks falling |
| SUBANIM_0_SOUND_WAVE | 31 | HVFLIP | Sound waves |
| SUBANIM_0_CIRCLE_1_SQUARE_HALF_TOSS | 32 | COORDFLIP | Circle/square half toss (unused) |
| SUBANIM_1_BARRIER | 33 | HVFLIP | Barrier effect |
| SUBANIM_1_SELFDESTRUCT | 34 | HVFLIP | Selfdestruct explosion |
| SUBANIM_0_WATER_BUBBLES | 35 | HVFLIP | Water bubbles |
| SUBANIM_0_CIRCLES_FALLING | 36 | HFLIP | Circles falling |
| SUBANIM_0_STRING_SHOT | 37 | HVFLIP | String shot |
| SUBANIM_0_ICE_FALL | 38 | HFLIP | Ice falling |
| SUBANIM_0_CIRCLE_1_SQUARE_APPEARS | 39 | COORDFLIP | Circle/square appears (unused) |
| SUBANIM_0_STATUS_SLEEP | 3A | COORDFLIP | Sleep status (player) |
| SUBANIM_0_STATUS_SLEEP_ENEMY | 3B | NORMAL | Sleep status (enemy) |
| SUBANIM_0_WATER_1_FIRE_BARRIER | 3C | COORDFLIP | Water/fire barrier (unused) |
| SUBANIM_0_WATER_1_FIRE_GEYSER | 3D | NORMAL | Water/fire geyser (unused) |
| SUBANIM_1_STAR_BIG_TOSS | 3E | REVERSE | Big star toss |
| SUBANIM_1_STARS_SMALL_TOSS | 3F | COORDFLIP | Small stars toss |
| SUBANIM_1_MUSIC_CIRCLING_ENEMY | 40 | HFLIP | Music notes circling |
| SUBANIM_1_CIRCLE_BLACK_TOSS | 41 | REVERSE | Black circle toss |
| SUBANIM_1_EXPLOSION_SMALL_ENEMY | 42 | HFLIP | Small explosion |
| SUBANIM_0_CIRCLE_1_SQUARE_CLOSING | 43 | ENEMY | Circle/square closing |
| SUBANIM_1_LEAVES_TOSS | 44 | HVFLIP | Leaves toss |
| SUBANIM_0_HORN_JAB_TWICE | 45 | HVFLIP | Horn jab twice |
| SUBANIM_0_HORN_JAB_THRICE | 46 | HVFLIP | Horn jab thrice |
| SUBANIM_0_BALL_POOF | 47 | HVFLIP | Ball poof |
| SUBANIM_2_TRADE_BALL_DROP | 48 | NORMAL | Trade ball drop |
| SUBANIM_2_TRADE_BALL_SHAKE | 49 | NORMAL | Trade ball shake |
| SUBANIM_2_TRADE_BALL_APPEAR | 4A | NORMAL | Trade ball appear |
| SUBANIM_2_TRADE_BALL_POOF | 4B | NORMAL | Trade ball poof |
| SUBANIM_0_EGG_SHAKING | 4C | COORDFLIP | Egg shaking |
| SUBANIM_1_TRIANGLE_TOSS | 4D | HVFLIP | Triangle toss |
| SUBANIM_1_SPHERE_BIG | 4E | HFLIP | Big sphere |
| SUBANIM_1_SPHERE_BIG_RISE | 4F | HFLIP | Big sphere rising |
| SUBANIM_1_SPHERE_BIG_FALL | 50 | HFLIP | Big sphere falling |
| SUBANIM_0_SHELL | 51 | ENEMY | Shell effect |
| SUBANIM_0_COIN_BOUNCE | 52 | HFLIP | Coin bounce |
| SUBANIM_0_SAFARI_ROCK | 53 | NORMAL | Safari rock |
| SUBANIM_0_SAFARI_BAIT | 54 | NORMAL | Safari bait |
| SUBANIM_0_STAR_HIGH | 55 | HFLIP | Star high |

---

## Move Animations (0x00-0xCA)

### 0x00 Pound / 0xA4 Struggle
```
battle_anim POUND, SUBANIM_0_STAR_TWICE, 0, 8
```
**Description**: Two star hits on the enemy.

---

### 0x01 Karate Chop
```
battle_anim KARATE_CHOP, SUBANIM_0_STAR_DESCENDING, 0, 8
```
**Description**: Stars descending onto the enemy.

---

### 0x02 Double Slap
```
battle_anim DOUBLESLAP, SUBANIM_0_STAR_TWICE, 0, 5
battle_anim DOUBLESLAP, SUBANIM_0_STAR_TWICE, 0, 5
```
**Description**: Two sets of star hits (slapping motion).

---

### 0x03 Comet Punch
```
battle_anim COMET_PUNCH, SUBANIM_0_STAR_THRICE, 0, 4
battle_anim COMET_PUNCH, SUBANIM_0_STAR_THRICE, 0, 4
```
**Description**: Two sets of three star hits (punching motion).

---

### 0x04 Mega Punch
```
battle_anim MEGA_PUNCH, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across screen.

---

### 0x05 Pay Day
```
battle_anim POUND, SUBANIM_0_STAR_TWICE, 0, 8
battle_anim PAY_DAY, SUBANIM_0_COIN_BOUNCE, 0, 4
```
**Description**: Star hits followed by coin bouncing.

---

### 0x06 Fire Punch
```
battle_anim FIRE_PUNCH, SUBANIM_0_STAR_THRICE, 0, 6
battle_anim NO_MOVE, SUBANIM_1_FLAMES, 1, 6
```
**Description**: Star hits followed by flames.

---

### 0x07 Ice Punch
```
battle_anim ICE_PUNCH, SUBANIM_0_STAR_THRICE, 0, 6
battle_anim NO_MOVE, SUBANIM_0_ICE_RISE, 0, 16
```
**Description**: Star hits followed by ice rising.

---

### 0x08 Thunder Punch
```
battle_anim THUNDERPUNCH, SUBANIM_0_STAR_THRICE, 0, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_1_LIGHTNING, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Star hits, dark screen, lightning bolt, reset palette.

---

### 0x09 Scratch
```
battle_anim SCRATCH, SUBANIM_0_SCRATCHES, 0, 6
```
**Description**: Scratch marks on enemy.

---

### 0x0A Vicegrip
```
battle_anim VICEGRIP, SUBANIM_0_SLICE_BOTH_SIDES, 0, 8
```
**Description**: Slicing from both sides.

---

### 0x0B Guillotine
```
battle_anim GUILLOTINE, SUBANIM_0_SLICE_BOTH_SIDES, 0, 6
```
**Description**: Slicing from both sides (faster).

---

### 0x0C Razor Wind
```
battle_anim RAZOR_WIND, SUBANIM_0_SLICE, 0, 4
```
**Description**: Slice effect.

---

### 0x0D Swords Dance
```
battle_anim SWORDS_DANCE, SUBANIM_1_SWORDS_CIRCLING, 1, 6
battle_anim SWORDS_DANCE, SUBANIM_1_SWORDS_CIRCLING, 1, 6
battle_anim SWORDS_DANCE, SUBANIM_1_SWORDS_CIRCLING, 1, 6
```
**Description**: Swords circling three times.

---

### 0x0E Cut
```
battle_anim CUT, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_0_SLICE, 0, 4
```
**Description**: Dark screen flash followed by slice.

---

### 0x0F Gust
```
battle_anim GUST, SUBANIM_1_TORNADO, 1, 6
battle_anim NO_MOVE, SUBANIM_0_STAR_THRICE, 0, 6
```
**Description**: Tornado followed by star hits.

---

### 0x10 Wing Attack
```
battle_anim WING_ATTACK, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x11 Whirlwind
```
battle_anim WHIRLWIND, SUBANIM_1_TORNADO, 1, 6
battle_anim NO_MOVE, SE_SLIDE_ENEMY_MON_OFF
```
**Description**: Tornado then enemy slides off.

---

### 0x12 Fly
```
battle_anim FLY, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Big star moving, then show mon.

---

### 0x13 Bind
```
battle_anim BIND, SUBANIM_0_BIND, 0, 4
battle_anim BIND, SUBANIM_0_BIND, 0, 4
```
**Description**: Bind effect twice.

---

### 0x14 Slam
```
battle_anim SLAM, SUBANIM_0_STAR_THRICE, 0, 6
```
**Description**: Three star hits.

---

### 0x15 Vine Whip
```
battle_anim VINE_WHIP, SUBANIM_0_SLICE, 0, 1
battle_anim NO_MOVE, SUBANIM_0_STAR_TWICE, 0, 8
```
**Description**: Slice followed by star hits.

---

### 0x16 Stomp
```
battle_anim STOMP, SUBANIM_1_STAR_BIG, 1, 8
```
**Description**: Single big star.

---

### 0x17 Double Kick
```
battle_anim DOUBLE_KICK, SUBANIM_0_STAR_TWICE, 0, 8
battle_anim DOUBLE_KICK, SUBANIM_0_STAR_TWICE, 0, 8
```
**Description**: Two sets of star hits.

---

### 0x18 Mega Kick
```
battle_anim MEGA_KICK, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x19 Jump Kick
```
battle_anim JUMP_KICK, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x1A Rolling Kick
```
battle_anim ROLLING_KICK, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Dark screen flash then big star.

---

### 0x1B Sand Attack
```
battle_anim SAND_ATTACK, SUBANIM_1_SAND, 1, 6
```
**Description**: Sand effect.

---

### 0x1C Headbutt
```
battle_anim HEADBUTT, SUBANIM_1_STAR_BIG, 1, 6
```
**Description**: Single big star.

---

### 0x1D Horn Attack
```
battle_anim HORN_ATTACK, SUBANIM_0_HORN_JAB_TWICE, 0, 6
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG, 1, 6
```
**Description**: Horn jab then big star.

---

### 0x1E Fury Attack
```
battle_anim FURY_ATTACK, SUBANIM_0_HORN_JAB_THRICE, 0, 2
battle_anim NO_MOVE, SUBANIM_0_HORN_JAB_THRICE, 0, 2
```
**Description**: Two sets of horn jabs.

---

### 0x1F Horn Drill
```
battle_anim HORN_DRILL, SUBANIM_1_STAR_BIG, 1, 2
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG, 1, 2
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG, 1, 2
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG, 1, 2
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG, 1, 2
```
**Description**: Five big star hits (drilling motion).

---

### 0x20 Tackle
```
battle_anim LEECH_SEED, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_RESET_MON_POSITION
```
**Description**: Move mon horizontally then reset.

---

### 0x21 Body Slam
```
battle_anim LEECH_SEED, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_MON_POSITION
```
**Description**: Move mon, two dark flashes, reset.

---

### 0x22 Wrap
```
battle_anim WRAP, SUBANIM_0_BIND, 0, 4
battle_anim WRAP, SUBANIM_0_BIND, 0, 4
battle_anim WRAP, SUBANIM_0_BIND, 0, 4
```
**Description**: Bind effect three times.

---

### 0x23 Take Down
```
battle_anim LEECH_SEED, SE_MOVE_MON_HORIZONTALLY
battle_anim TAKE_DOWN, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_MON_POSITION
```
**Description**: Move mon, dark flash, reset.

---

### 0x24 Thrash
```
battle_anim THRASH, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x25 Double Edge
```
battle_anim LEECH_SEED, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLES_CENTERING, 0, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
battle_anim NO_MOVE, SE_MOVE_MON_HORIZONTALLY
battle_anim DOUBLE_EDGE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_MON_POSITION
```
**Description**: Light palette, circles centering, reset, move mon, dark flash, reset.

---

### 0x26 Tail Whip
```
battle_anim AMNESIA, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim AMNESIA, SE_RESET_MON_POSITION
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim AMNESIA, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim AMNESIA, SE_RESET_MON_POSITION
```
**Description**: Shake back and forth three times with delays.

---

### 0x27 Poison Sting
```
battle_anim POISON_STING, SUBANIM_0_STAR, 0, 6
```
**Description**: Single star hit.

---

### 0x28 Twineedle
```
battle_anim TWINEEDLE, SUBANIM_0_STAR_TWICE, 0, 5
battle_anim TWINEEDLE, SUBANIM_0_STAR_TWICE, 0, 5
```
**Description**: Two sets of star hits.

---

### 0x29 Pin Missile
```
battle_anim PIN_MISSILE, SUBANIM_0_STAR_TWICE, 0, 3
```
**Description**: Star hits (fast).

---

### 0x2A Leer
```
battle_anim LEECH_SEED, SE_DARK_SCREEN_PALETTE
battle_anim LEER, SE_DARK_SCREEN_FLASH
battle_anim LEER, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, two dark flashes, reset.

---

### 0x2B Bite
```
battle_anim BITE, SUBANIM_0_STAR_THRICE, 0, 8
```
**Description**: Three star hits.

---

### 0x2C Growl
```
battle_anim GROWL, SUBANIM_0_HEART_1_MUSIC, 1, 6
```
**Description**: Heart/music notes.

---

### 0x2D Roar
```
battle_anim ROAR, SUBANIM_1_SHOUT, 1, 6
battle_anim ROAR, SUBANIM_1_SHOUT, 1, 6
battle_anim ROAR, SUBANIM_1_SHOUT, 1, 6
```
**Description**: Shout waves three times.

---

### 0x2E Sing
```
battle_anim SING, SUBANIM_0_HEART_1_MUSIC, 1, 6
battle_anim NO_MOVE, SUBANIM_1_MUSIC_CIRCLING_ENEMY, 1, 16
battle_anim NO_MOVE, SUBANIM_1_MUSIC_CIRCLING_ENEMY, 1, 16
```
**Description**: Heart/music then music circling twice.

---

### 0x2F Supersonic
```
battle_anim SUPERSONIC, SUBANIM_0_SOUND_WAVE, 0, 6
```
**Description**: Sound waves.

---

### 0x30 Sonic Boom
```
battle_anim ROAR, SUBANIM_1_SHOUT, 1, 6
battle_anim ROAR, SUBANIM_1_SHOUT, 1, 6
battle_anim GUST, SUBANIM_1_TORNADO, 1, 6
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG, 1, 6
```
**Description**: Shout waves, tornado, big star.

---

### 0x31 Disable
```
battle_anim LEECH_SEED, SE_DARK_SCREEN_PALETTE
battle_anim LEER, SE_DARK_SCREEN_FLASH
battle_anim LEER, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, two dark flashes, reset.

---

### 0x32 Acid
```
battle_anim ACID, SUBANIM_1_BLOB_TOSS, 1, 6
battle_anim ACID, SUBANIM_1_BLOB_DRIP_ENEMY, 1, 6
```
**Description**: Blob toss then blob drip.

---

### 0x33 Ember
```
battle_anim EMBER, SUBANIM_1_FLAMES, 1, 6
```
**Description**: Flames.

---

### 0x34 Flamethrower
```
battle_anim FLAMETHROWER, SUBANIM_1_FLAME_BEAM, 1, 6
battle_anim FLAMETHROWER, SUBANIM_1_FLAME_COLUMN_1, 1, 6
battle_anim FLAMETHROWER, SUBANIM_1_FLAME_COLUMN_2, 1, 6
```
**Description**: Flame beam then flame columns.

---

### 0x35 Mist
```
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim SURF, SE_WATER_DROPLETS_EVERYWHERE
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, water droplets, reset.

---

### 0x36 Water Gun
```
battle_anim WATER_GUN, SUBANIM_0_WATER_DROPLETS, 0, 6
```
**Description**: Water droplets.

---

### 0x37 Hydro Pump
```
battle_anim HYDRO_PUMP, SUBANIM_0_WATER_COLUMNS, 0, 6
battle_anim HYDRO_PUMP, SUBANIM_0_WATER_COLUMNS, 0, 6
```
**Description**: Water columns twice.

---

### 0x38 Surf
```
battle_anim SURF, SE_WATER_DROPLETS_EVERYWHERE
battle_anim HYDRO_PUMP, SUBANIM_0_WATER_COLUMNS, 0, 6
```
**Description**: Water droplets everywhere then water columns.

---

### 0x39 Ice Beam
```
battle_anim ICE_BEAM, SUBANIM_0_BEAM, 0, 3
battle_anim NO_MOVE, SUBANIM_0_ICE_RISE, 0, 16
```
**Description**: Beam then ice rising.

---

### 0x3A Blizzard
```
battle_anim BLIZZARD, SUBANIM_0_ICE_FALL, 0, 4
battle_anim HYDRO_PUMP, SUBANIM_0_ICE_FALL, 0, 4
```
**Description**: Ice falling twice.

---

### 0x3B Psybeam
```
battle_anim PSYBEAM, SUBANIM_0_BEAM, 0, 3
battle_anim NO_MOVE, SE_FLASH_SCREEN_LONG
```
**Description**: Beam then screen flash.

---

### 0x3C Bubblebeam
```
battle_anim BUBBLEBEAM, SUBANIM_0_WATER_BUBBLES, 0, 18
```
**Description**: Water bubbles.

---

### 0x3D Aurora Beam
```
battle_anim AURORA_BEAM, SUBANIM_0_BEAM, 0, 3
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
```
**Description**: Beam then delays.

---

### 0x3E Hyper Beam
```
battle_anim LEECH_SEED, SE_DARK_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim HYPER_BEAM, SUBANIM_0_BEAM, 0, 2
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim MEGA_PUNCH, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, spiral, beam, flashes, big star, reset.

---

### 0x3F Peck
```
battle_anim PECK, SUBANIM_0_STAR_TWICE, 0, 8
```
**Description**: Star hits.

---

### 0x40 Drill Peck
```
battle_anim DRILL_PECK, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x41 Submission
```
battle_anim SUBMISSION, SE_SLIDE_MON_OFF
battle_anim NO_MOVE, SUBANIM_0_STAR_TWICE, 0, 6
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Slide off, star hits, show mon.

---

### 0x42 Low Kick
```
battle_anim LOW_KICK, SE_SLIDE_MON_OFF
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Slide off, big star, show mon.

---

### 0x43 Counter
```
battle_anim COUNTER, SE_SLIDE_MON_OFF
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Slide off, big star, show mon.

---

### 0x44 Seismic Toss
```
battle_anim NO_MOVE, SE_BLINK_ENEMY_MON
battle_anim BARRAGE, SUBANIM_1_SPHERE_BIG, 1, 1
battle_anim NO_MOVE, SE_HIDE_ENEMY_MON_PIC
battle_anim NO_MOVE, SE_SLIDE_MON_OFF
battle_anim SEISMIC_TOSS, SUBANIM_1_SPHERE_BIG_RISE, 1, 2
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_SHOW_MON_PIC
battle_anim SEISMIC_TOSS, SUBANIM_1_SPHERE_BIG_FALL, 1, 1
battle_anim NO_MOVE, SE_SHOW_ENEMY_MON_PIC
battle_anim NO_MOVE, SE_SHAKE_SCREEN
```
**Description**: Blink, sphere, hide, slide, sphere rise, delays, show, sphere fall, show enemy, shake.

---

### 0x45 Strength
```
battle_anim LEECH_SEED, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_RESET_MON_POSITION
battle_anim FIRE_PUNCH, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Move mon, reset, big star.

---

### 0x46 Absorb
```
battle_anim ABSORB, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY, 0, 6
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK, 0, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, circles centering, toss back, reset.

---

### 0x47 Mega Drain
```
battle_anim MEGA_DRAIN, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY, 0, 6
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK, 0, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, flash, circles, toss back, flash, reset.

---

### 0x48 Leech Seed
```
battle_anim LEECH_SEED, SUBANIM_1_SEED_TOSS, 1, 6
battle_anim STUN_SPORE, SUBANIM_1_SEED_LAND, 1, 21
```
**Description**: Seed toss then seed land.

---

### 0x49 Growth
```
battle_anim GROWTH, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, spiral, reset.

---

### 0x4A Razor Leaf
```
battle_anim RAZOR_LEAF, SE_LEAVES_FALLING
battle_anim SWIFT, SUBANIM_1_LEAVES_TOSS, 1, 1
battle_anim RAZOR_WIND, SUBANIM_0_SLICE, 0, 1
```
**Description**: Leaves falling, leaves toss, slice.

---

### 0x4B Solar Beam
```
battle_anim SOLARBEAM, SUBANIM_0_BEAM, 0, 6
battle_anim NO_MOVE, SUBANIM_0_STAR_TWICE, 0, 6
```
**Description**: Beam then star hits.

---

### 0x4C Poison Powder
```
battle_anim POISONPOWDER, SUBANIM_0_CIRCLES_FALLING, 0, 6
```
**Description**: Circles falling.

---

### 0x4D Stun Spore
```
battle_anim STUN_SPORE, SUBANIM_0_CIRCLES_FALLING, 0, 6
```
**Description**: Circles falling.

---

### 0x4E Sleep Powder
```
battle_anim SLEEP_POWDER, SUBANIM_0_CIRCLES_FALLING, 0, 6
```
**Description**: Circles falling.

---

### 0x4F Petal Dance
```
battle_anim PETAL_DANCE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_PETALS_FALLING
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, petals falling, reset.

---

### 0x50 String Shot
```
battle_anim STRING_SHOT, SUBANIM_0_STRING_SHOT, 0, 8
```
**Description**: String shot effect.

---

### 0x51 Dragon Rage
```
battle_anim DRAGON_RAGE, SUBANIM_1_FLAME_BEAM, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_1, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_2, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_3, 1, 6
```
**Description**: Flame beam then flame columns.

---

### 0x52 Fire Spin
```
battle_anim FIRE_SPIN, SUBANIM_1_FLAME_COLUMN_1, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_2, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_3, 1, 6
```
**Description**: Flame columns.

---

### 0x53 Thunder Shock
```
battle_anim THUNDERSHOCK, SUBANIM_1_LIGHTNING_BALL, 1, 2
```
**Description**: Lightning ball.

---

### 0x54 Thunderbolt
```
battle_anim THUNDERBOLT, SUBANIM_1_LIGHTNING_BALL, 1, 1
battle_anim THUNDERBOLT, SUBANIM_1_LIGHTNING_BALL, 1, 1
```
**Description**: Lightning ball twice.

---

### 0x55 Thunder Wave
```
battle_anim THUNDER_WAVE, SUBANIM_1_LIGHTNING_BALL, 1, 2
battle_anim NO_MOVE, SUBANIM_0_BIND, 0, 2
battle_anim NO_MOVE, SUBANIM_0_BIND, 0, 4
```
**Description**: Lightning ball then bind.

---

### 0x56 Thunder
```
battle_anim THUNDER, SE_DARK_SCREEN_PALETTE
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_1_LIGHTNING, 1, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim THUNDERBOLT, SUBANIM_1_LIGHTNING_BALL, 1, 2
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, flash, lightning, flash, lightning ball, reset.

---

### 0x57 Rock Throw
```
battle_anim ROCK_THROW, SUBANIM_0_ROCKS_FALL_ENEMY, 0, 4
```
**Description**: Rocks falling.

---

### 0x58 Earthquake
```
battle_anim EARTHQUAKE, SE_SHAKE_SCREEN
battle_anim EARTHQUAKE, SE_SHAKE_SCREEN
```
**Description**: Screen shake twice.

---

### 0x59 Fissure
```
battle_anim FISSURE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_SHAKE_SCREEN
battle_anim FISSURE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_SHAKE_SCREEN
```
**Description**: Dark flash and shake twice.

---

### 0x5A Dig
```
battle_anim DIG, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_SLIDE_MON_UP
```
**Description**: Big star then slide up.

---

### 0x5B Toxic
```
battle_anim SURF, SE_WATER_DROPLETS_EVERYWHERE
battle_anim TOXIC, SUBANIM_1_BLOB_DRIP_ENEMY, 1, 6
```
**Description**: Water droplets then blob drip.

---

### 0x5C Confusion
```
battle_anim CONFUSION, SE_FLASH_SCREEN_LONG
```
**Description**: Screen flash.

---

### 0x5D Psychic
```
battle_anim PSYCHIC_M, SE_FLASH_SCREEN_LONG
battle_anim NO_MOVE, SE_WAVY_SCREEN
```
**Description**: Screen flash then wavy screen.

---

### 0x5E Hypnosis
```
battle_anim HYPNOSIS, SE_FLASH_SCREEN_LONG
```
**Description**: Screen flash.

---

### 0x5F Meditate
```
battle_anim MEDITATE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_CLOSING, 1, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, circle/square closing, flash, reset.

---

### 0x60 Agility
```
battle_anim AGILITY, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette then reset.

---

### 0x61 Quick Attack
```
battle_anim QUICK_ATTACK, SE_SLIDE_MON_OFF
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Slide off, big star, show mon.

---

### 0x62 Rage
```
battle_anim RAGE, SUBANIM_0_STAR_TWICE, 0, 6
```
**Description**: Star hits.

---

### 0x63 Teleport
```
battle_anim TELEPORT, SE_SQUISH_MON_PIC
battle_anim NO_MOVE, SE_SHOOT_BALLS_UPWARD
```
**Description**: Squish mon then balls upward.

---

### 0x64 Night Shade
```
battle_anim CONFUSION, SE_FLASH_SCREEN_LONG
battle_anim NO_MOVE, SE_WAVY_SCREEN
```
**Description**: Screen flash then wavy screen.

---

### 0x65 Mimic
```
battle_anim MIMIC, SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY, 1, 6
battle_anim MIMIC, SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK, 1, 6
```
**Description**: Circles centering then toss back.

---

### 0x66 Screech
```
battle_anim SCREECH, SUBANIM_0_HEART_1_MUSIC, 1, 6
```
**Description**: Heart/music notes.

---

### 0x67 Double Team
```
battle_anim NO_MOVE, SE_DARK_SCREEN_PALETTE
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
battle_anim DOUBLE_TEAM, SE_SHAKE_BACK_AND_FORTH
battle_anim NO_MOVE, SE_SHOW_MON_PIC
battle_anim BARRIER, SUBANIM_1_BARRIER, 1, 6
```
**Description**: Dark palette, delays, flashes, reset, shake, show mon, barrier.

---

### 0x68 Recover
```
battle_anim RECOVER, SE_BLINK_MON
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Blink, light palette, spiral, reset.

---

### 0x69 Harden
```
battle_anim HARDEN, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_CLOSING, 1, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, circle/square closing, flash, reset.

---

### 0x6A Minimize
```
battle_anim MINIMIZE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim NO_MOVE, SE_MINIMIZE_MON
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, spiral, minimize, reset.

---

### 0x6B Smokescreen
```
battle_anim SMOKESCREEN, SUBANIM_1_SAND, 1, 6
battle_anim NO_MOVE, SUBANIM_0_BALL_POOF_ENEMY, 0, 4
battle_anim NO_MOVE, SE_DARKEN_MON_PALETTE
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DARK_SCREEN_PALETTE
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_DARKEN_MON_PALETTE
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Sand, poof, darken, many delays, dark palette, more delays, darken, delay, reset.

---

### 0x6C Confuse Ray
```
battle_anim CONFUSE_RAY, SE_DARK_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_1_STAR_BIG_TOSS, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, big star toss, reset.

---

### 0x6D Withdraw
```
battle_anim DEFENSE_CURL, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SLIDE_MON_DOWN
battle_anim NO_MOVE, SUBANIM_0_SHELL, 0, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Light palette, slide down, shell, reset, show mon.

---

### 0x6E Defense Curl
```
battle_anim DEFENSE_CURL, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_CLOSING, 0, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, circle/square closing, flash, reset.

---

### 0x6F Barrier
```
battle_anim BARRIER, SUBANIM_1_BARRIER, 1, 6
battle_anim BARRIER, SUBANIM_1_BARRIER, 1, 6
```
**Description**: Barrier twice.

---

### 0x70 Light Screen
```
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim LIGHT_SCREEN, SUBANIM_1_BARRIER, 1, 6
battle_anim LIGHT_SCREEN, SUBANIM_1_BARRIER, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, barrier twice, reset.

---

### 0x71 Haze
```
battle_anim NO_MOVE, SE_DARKEN_MON_PALETTE
battle_anim SURF, SE_WATER_DROPLETS_EVERYWHERE
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Darken, water droplets, reset.

---

### 0x72 Reflect
```
battle_anim NO_MOVE, SE_DARK_SCREEN_PALETTE
battle_anim REFLECT, SUBANIM_1_BARRIER, 1, 6
battle_anim REFLECT, SUBANIM_1_BARRIER, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, barrier twice, reset.

---

### 0x73 Focus Energy
```
battle_anim FOCUS_ENERGY, SE_SPIRAL_BALLS_INWARD
```
**Description**: Spiral balls inward.

---

### 0x74 Bide
```
battle_anim BIDE, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x75 Metronome
```
battle_anim AMNESIA, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim AMNESIA, SE_RESET_MON_POSITION
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim AMNESIA, SE_MOVE_MON_HORIZONTALLY
battle_anim NO_MOVE, SE_DELAY_ANIMATION_10
battle_anim AMNESIA, SE_RESET_MON_POSITION
```
**Description**: Shake back and forth three times with delays.

---

### 0x76 Mirror Move
```
battle_anim MIRROR_MOVE, SUBANIM_0_STAR_TWICE, 0, 8
```
**Description**: Star hits.

---

### 0x77 Selfdestruct
```
battle_anim SELFDESTRUCT, SUBANIM_1_SELFDESTRUCT, 1, 3
```
**Description**: Selfdestruct explosion.

---

### 0x78 Egg Bomb
```
battle_anim EGG_BOMB, SUBANIM_1_CIRCLE_BLACK_TOSS, 1, 4
battle_anim EGG_BOMB, SUBANIM_1_EXPLOSION_SMALL_ENEMY, 1, 4
```
**Description**: Black circle toss then small explosion.

---

### 0x79 Lick
```
battle_anim SLUDGE, SUBANIM_1_BLOB_DRIP_ENEMY, 1, 6
```
**Description**: Blob drip.

---

### 0x7A Smog
```
battle_anim LEECH_SEED, SE_DARKEN_MON_PALETTE
battle_anim SMOG, SUBANIM_1_CLOUD_TOSS, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Darken, cloud toss, reset.

---

### 0x7B Sludge
```
battle_anim SLUDGE, SUBANIM_1_BLOB_TOSS, 1, 6
battle_anim SLUDGE, SUBANIM_1_BLOB_DRIP_ENEMY, 1, 6
```
**Description**: Blob toss then blob drip.

---

### 0x7C Bone Club
```
battle_anim BONE_CLUB, SUBANIM_0_STAR_THRICE, 0, 8
```
**Description**: Three star hits.

---

### 0x7D Fire Blast
```
battle_anim FIRE_BLAST, SUBANIM_1_FLAME_BEAM, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_STAR, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_STAR, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_1, 1, 6
battle_anim NO_MOVE, SUBANIM_1_FLAME_COLUMN_2, 1, 6
```
**Description**: Flame beam, flame stars, flame columns.

---

### 0x7E Waterfall
```
battle_anim LEECH_SEED, SE_SLIDE_MON_DOWN
battle_anim HYDRO_PUMP, SUBANIM_0_WATER_COLUMNS, 0, 6
battle_anim NO_MOVE, SUBANIM_0_STAR_THRICE, 0, 8
battle_anim NO_MOVE, SE_SLIDE_MON_UP
```
**Description**: Slide down, water columns, star hits, slide up.

---

### 0x7F Clamp
```
battle_anim CLAMP, SUBANIM_0_SLICE_BOTH_SIDES, 0, 8
battle_anim CONSTRICT, SUBANIM_0_BIND, 0, 6
battle_anim CONSTRICT, SUBANIM_0_BIND, 0, 6
```
**Description**: Slice both sides then bind twice.

---

### 0x80 Swift
```
battle_anim SWIFT, SUBANIM_1_STARS_SMALL_TOSS, 1, 3
```
**Description**: Small stars toss.

---

### 0x81 Skull Bash
```
battle_anim SKULL_BASH, SUBANIM_1_STAR_BIG, 1, 6
```
**Description**: Single big star.

---

### 0x82 Spike Cannon
```
battle_anim SPIKE_CANNON, SUBANIM_1_STAR_BIG_MOVING, 1, 4
```
**Description**: Big star moving across.

---

### 0x83 Constrict
```
battle_anim CONSTRICT, SUBANIM_0_BIND, 0, 6
battle_anim CONSTRICT, SUBANIM_0_BIND, 0, 6
battle_anim CONSTRICT, SUBANIM_0_BIND, 0, 6
```
**Description**: Bind three times.

---

### 0x84 Amnesia
```
battle_anim AMNESIA, SUBANIM_0_STATUS_CONFUSED, 0, 8
battle_anim AMNESIA, SUBANIM_0_STATUS_CONFUSED, 0, 8
```
**Description**: Confusion status twice.

---

### 0x85 Kinesis
```
battle_anim KINESIS, SUBANIM_0_STAR_TWICE, 0, 8
```
**Description**: Star hits.

---

### 0x86 Softboiled
```
battle_anim LEECH_SEED, SE_SLIDE_MON_HALF_OFF
battle_anim SOFTBOILED, SUBANIM_0_EGG_SHAKING, 0, 8
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Slide half off, egg shaking, light palette, spiral, reset, show mon.

---

### 0x87 Hi Jump Kick
```
battle_anim HI_JUMP_KICK, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Big star moving across.

---

### 0x88 Glare
```
battle_anim LEECH_SEED, SE_DARK_SCREEN_PALETTE
battle_anim GLARE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, two dark flashes, reset.

---

### 0x89 Dream Eater
```
battle_anim DREAM_EATER, SE_FLASH_SCREEN_LONG
battle_anim DREAM_EATER, SE_DARK_SCREEN_PALETTE
battle_anim DREAM_EATER, SUBANIM_0_STAR_THRICE, 0, 8
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Screen flash, dark palette, star hits, reset.

---

### 0x8A Poison Gas
```
battle_anim POISON_GAS, SUBANIM_1_CLOUD_TOSS, 1, 6
```
**Description**: Cloud toss.

---

### 0x8B Barrage
```
battle_anim BARRAGE, SUBANIM_1_CIRCLE_BLACK_TOSS, 1, 3
battle_anim NO_MOVE, SUBANIM_0_STAR_HIGH, 0, 5
```
**Description**: Black circle toss then star high.

---

### 0x8C Leech Life
```
battle_anim LEECH_LIFE, SUBANIM_0_STAR_THRICE, 0, 8
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY, 0, 6
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK, 0, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
```
**Description**: Star hits, flash, circles centering, toss back, flash.

---

### 0x8D Lovely Kiss
```
battle_anim LOVELY_KISS, SUBANIM_0_HEART_1_MUSIC, 0, 6
```
**Description**: Heart/music notes.

---

### 0x8E Sky Attack
```
battle_anim SKY_ATTACK, SE_SQUISH_MON_PIC
battle_anim NO_MOVE, SE_SHOOT_BALLS_UPWARD
battle_anim HI_JUMP_KICK, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Squish, balls upward, big star, show mon.

---

### 0x8F Transform
```
battle_anim TRANSFORM, SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY, 1, 6
battle_anim TRANSFORM, SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK, 1, 4
battle_anim NO_MOVE, SUBANIM_0_BALL_POOF, 0, 8
battle_anim NO_MOVE, SE_TRANSFORM_MON
```
**Description**: Circles centering, toss back, ball poof, transform.

---

### 0x90 Bubble
```
battle_anim BUBBLE, SUBANIM_0_WATER_BUBBLES, 0, 22
```
**Description**: Water bubbles.

---

### 0x91 Dizzy Punch
```
battle_anim DIZZY_PUNCH, SUBANIM_0_BIRDIES_CIRCLING_ENEMY, 0, 6
battle_anim DIZZY_PUNCH, SUBANIM_0_BIRDIES_CIRCLING_ENEMY, 0, 6
battle_anim DIZZY_PUNCH, SUBANIM_0_BIRDIES_CIRCLING_ENEMY, 0, 6
battle_anim DOUBLESLAP, SUBANIM_0_STAR_THRICE, 0, 6
```
**Description**: Birdies circling three times then star hits.

---

### 0x92 Spore
```
battle_anim SPORE, SUBANIM_0_CIRCLES_FALLING, 0, 6
```
**Description**: Circles falling.

---

### 0x93 Flash
```
battle_anim LEECH_SEED, SE_LIGHT_SCREEN_PALETTE
battle_anim GLARE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, two dark flashes, reset.

---

### 0x94 Psywave
```
battle_anim SUPERSONIC, SUBANIM_0_SOUND_WAVE, 0, 6
battle_anim CONFUSION, SE_WAVY_SCREEN
```
**Description**: Sound waves then wavy screen.

---

### 0x95 Splash
```
battle_anim SPLASH, SE_BOUNCE_UP_AND_DOWN
```
**Description**: Bounce up and down.

---

### 0x96 Acid Armor
```
battle_anim ACID_ARMOR, SE_SLIDE_MON_DOWN_AND_HIDE
```
**Description**: Slide down and hide.

---

### 0x97 Crabhammer
```
battle_anim CRABHAMMER, SUBANIM_1_STAR_BIG, 1, 6
battle_anim NO_MOVE, SUBANIM_0_SLICE_BOTH_SIDES, 0, 6
```
**Description**: Big star then slice both sides.

---

### 0x98 Explosion
```
battle_anim EXPLOSION, SUBANIM_1_SELFDESTRUCT, 1, 3
```
**Description**: Selfdestruct explosion.

---

### 0x99 Fury Swipes
```
battle_anim FURY_SWIPES, SUBANIM_0_SCRATCHES, 0, 4
```
**Description**: Scratch marks.

---

### 0x9A Bonemerang
```
battle_anim BONEMERANG, SUBANIM_0_STAR_THRICE, 0, 6
```
**Description**: Three star hits.

---

### 0x9B Rest
```
battle_anim REST, SUBANIM_0_STATUS_SLEEP, 0, 16
battle_anim REST, SUBANIM_0_STATUS_SLEEP, 0, 16
```
**Description**: Sleep status twice.

---

### 0x9C Rock Slide
```
battle_anim ROCK_SLIDE, SUBANIM_0_ROCKS_LIFT, 0, 4
battle_anim ROCK_SLIDE, SUBANIM_0_ROCKS_TOSS, 0, 3
battle_anim HYPER_FANG, SUBANIM_1_STAR_BIG_MOVING, 1, 6
```
**Description**: Rocks lift, rocks toss, big star.

---

### 0x9D Hyper Fang
```
battle_anim HYPER_FANG, SUBANIM_0_STAR_THRICE, 0, 6
```
**Description**: Three star hits.

---

### 0x9E Sharpen
```
battle_anim SHARPEN, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_CLOSING, 1, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, circle/square closing, flash, reset.

---

### 0x9F Conversion
```
battle_anim CONVERSION, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_0_CIRCLES_1_SQUARES_CENTERING_ENEMY, 1, 6
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_TOSS_BACK, 1, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
```
**Description**: Dark flash, circles centering, toss back, flash.

---

### 0xA0 Tri Attack
```
battle_anim TRI_ATTACK, SE_DARK_SCREEN_FLASH
battle_anim NO_MOVE, SUBANIM_1_TRIANGLE_TOSS, 1, 6
battle_anim NO_MOVE, SE_DARK_SCREEN_FLASH
```
**Description**: Dark flash, triangle toss, flash.

---

### 0xA1 Super Fang
```
battle_anim LEECH_SEED, SE_DARK_SCREEN_PALETTE
battle_anim SUPER_FANG, SUBANIM_1_STAR_BIG_MOVING, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Dark palette, big star, reset.

---

### 0xA2 Slash
```
battle_anim SLASH, SUBANIM_0_SCRATCHES, 0, 6
```
**Description**: Scratch marks.

---

### 0xA3 Substitute
```
battle_anim SUBSTITUTE, SE_SLIDE_MON_OFF
battle_anim NO_MOVE, SUBANIM_0_BALL_POOF, 0, 8
battle_anim NO_MOVE, SE_SUBSTITUTE_MON
```
**Description**: Slide off, ball poof, substitute.

---

### 0xA4 Struggle
(See 0x00 Pound - same animation)

---

### 0xA5 ShowPic
```
battle_anim NO_MOVE, SE_SHOW_ENEMY_MON_PIC
```
**Description**: Show enemy mon.

---

### 0xA6 EnemyFlash
```
battle_anim NO_MOVE, SE_SHOW_MON_PIC
```
**Description**: Show player mon.

---

### 0xA7 PlayerFlash
```
battle_anim NO_MOVE, SE_FLASH_MON_PIC
```
**Description**: Flash player mon.

---

### 0xA8 EnemyHUDShake
```
battle_anim NO_MOVE, SE_SHAKE_ENEMY_HUD
```
**Description**: Shake enemy HUD.

---

### 0xA9 Trade Ball Drop
```
battle_anim NO_MOVE, SUBANIM_2_TRADE_BALL_DROP, 2, 6
```
**Description**: Trade ball drop (tileset 2).

---

### 0xAA Trade Ball Appear 1
```
battle_anim NO_MOVE, SUBANIM_2_TRADE_BALL_SHAKE, 2, 4
```
**Description**: Trade ball shake (tileset 2).

---

### 0xAB Trade Ball Appear 2
```
battle_anim NO_MOVE, SUBANIM_2_TRADE_BALL_APPEAR, 2, 6
```
**Description**: Trade ball appear (tileset 2).

---

### 0xAC Trade Ball Poof
```
battle_anim NO_MOVE, SUBANIM_2_TRADE_BALL_POOF, 2, 6
```
**Description**: Trade ball poof (tileset 2).

---

### 0xAD X Stat Item
```
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, spiral, reset.

---

### 0xAE X Stat Item (duplicate)
(See 0xAD)

---

### 0xAF Shrinking Square
```
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_CLOSING, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, circle/square closing, reset.

---

### 0xB0 Shrinking Square (duplicate)
(See 0xAF)

---

### 0xB1 X Stat Item Black
```
battle_anim NO_MOVE, SE_DARKEN_MON_PALETTE
battle_anim NO_MOVE, SE_SPIRAL_BALLS_INWARD
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Darken, spiral, reset.

---

### 0xB2 X Stat Item Black (duplicate)
(See 0xB1)

---

### 0xB3 Shrinking Square Black
```
battle_anim NO_MOVE, SE_DARKEN_MON_PALETTE
battle_anim NO_MOVE, SUBANIM_0_CIRCLE_1_SQUARE_CLOSING, 1, 6
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Darken, circle/square closing, reset.

---

### 0xB4 Shrinking Square Black (duplicate)
(See 0xB3)

---

### 0xB5 Unused
```
battle_anim NO_MOVE, SE_LIGHT_SCREEN_PALETTE
battle_anim NO_MOVE, SE_SHOOT_MANY_BALLS_UPWARD
battle_anim NO_MOVE, SE_RESET_SCREEN_PALETTE
```
**Description**: Light palette, many balls upward, reset.

---

### 0xB6 Unused (duplicate)
(See 0xB5)

---

### 0xB7 Paralyze
```
battle_anim BIND, SUBANIM_0_STATUS_PARALYZED, 0, 4
battle_anim BIND, SUBANIM_0_STATUS_PARALYZED, 0, 4
```
**Description**: Paralysis status twice.

---

### 0xB8 Paralyze (duplicate)
(See 0xB7)

---

### 0xB9 Poison
```
battle_anim BIND, SUBANIM_0_STATUS_POISONED, 0, 8
battle_anim BIND, SUBANIM_0_STATUS_POISONED, 0, 8
```
**Description**: Poison status twice.

---

### 0xBA Poison (duplicate)
(See 0xB9)

---

### 0xBB Sleep Player
```
battle_anim REST, SUBANIM_0_STATUS_SLEEP, 0, 16
battle_anim REST, SUBANIM_0_STATUS_SLEEP, 0, 16
```
**Description**: Sleep status twice.

---

### 0xBC Sleep Enemy
```
battle_anim REST, SUBANIM_0_STATUS_SLEEP_ENEMY, 0, 16
battle_anim REST, SUBANIM_0_STATUS_SLEEP_ENEMY, 0, 16
```
**Description**: Sleep status (enemy) twice.

---

### 0xBD Confused Player
```
battle_anim AMNESIA, SUBANIM_0_STATUS_CONFUSED, 0, 8
battle_anim AMNESIA, SUBANIM_0_STATUS_CONFUSED, 0, 8
```
**Description**: Confusion status twice.

---

### 0xBE Confused Enemy
```
battle_anim AMNESIA, SUBANIM_0_STATUS_CONFUSED_ENEMY, 0, 8
battle_anim AMNESIA, SUBANIM_0_STATUS_CONFUSED_ENEMY, 0, 8
```
**Description**: Confusion status (enemy) twice.

---

### 0xBF Slide Down
```
battle_anim DIG, SE_SLIDE_MON_DOWN
```
**Description**: Slide mon down.

---

### 0xC0 Ball Toss
```
battle_anim NO_MOVE, SUBANIM_0_BALL_TOSS_HIGH, 0, 3
```
**Description**: Pokéball toss (high arc).

---

### 0xC1 Ball Shake
```
battle_anim NO_MOVE, SUBANIM_0_BALL_SHAKE_ENEMY, 0, 4
```
**Description**: Pokéball shaking.

---

### 0xC2 Ball Poof
```
battle_anim NO_MOVE, SUBANIM_0_BALL_POOF_ENEMY, 0, 4
```
**Description**: Pokéball poof.

---

### 0xC3 Ball Block
```
battle_anim NO_MOVE, SUBANIM_0_BALL_BLOCK, 0, 3
```
**Description**: Pokéball blocked.

---

### 0xC4 Great Toss
```
battle_anim NO_MOVE, SUBANIM_0_BALL_TOSS_MIDDLE, 0, 3
```
**Description**: Great ball toss (middle arc).

---

### 0xC5 Ultra Toss
```
battle_anim NO_MOVE, SUBANIM_0_BALL_TOSS_LOW, 0, 2
```
**Description**: Ultra ball toss (low arc).

---

### 0xC6 Shake Screen
```
battle_anim NO_MOVE, SE_SHAKE_SCREEN
```
**Description**: Screen shake.

---

### 0xC7 Hide Pic
```
battle_anim NO_MOVE, SE_HIDE_ENEMY_MON_PIC
```
**Description**: Hide enemy mon.

---

### 0xC8 Throw Rock
```
battle_anim BARRAGE, SUBANIM_0_SAFARI_ROCK, 0, 3
```
**Description**: Safari rock toss.

---

### 0xC9 Throw Bait
```
battle_anim BARRAGE, SUBANIM_0_SAFARI_BAIT, 0, 3
```
**Description**: Safari bait toss.

---

### 0xCA ZigZag Screen
```
battle_anim NO_MOVE, SE_WAVY_SCREEN
```
**Description**: Wavy screen effect.

---

## Notes

- The `battle_anim` macro encodes commands as:
  - 4-arg: `(tileset << 6) | delay`, `sound_id - 1`, `subanim_id`
  - 2-arg: `effect_id`, `sound_id - 1`
- Sound IDs are decremented by 1 in the ROM encoding
- Tileset 2 is used exclusively for trade animations
- Some animations share the same label (e.g., Pound/Struggle)
- Unused animations (0xB5, 0xB6) are preserved for ROM accuracy
