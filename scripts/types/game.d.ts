/**
 * dotzuki-engine-script Game API Type Definitions
 *
 * These declarations describe the global `game` object injected by the
 * dotzuki-engine-script Boa engine.  They are provided for editor autocompletion
 * and type-checking of map script files (PalletTown.js, OaksLab.js, …).
 *
 * Async methods return a Promise that resolves once the game loop has
 * finished executing the corresponding command (e.g. the text box has
 * been dismissed, the NPC has finished moving, the battle is over).
 *
 * Sync methods execute immediately inside the JS runtime and never yield
 * to the game loop.
 */

// ---------------------------------------------------------------------------
// Direction helpers (used by faceNpc / facePlayer)
// ---------------------------------------------------------------------------

/** Cardinal direction string accepted by facing commands. */
type Direction = "up" | "down" | "left" | "right";

/** Screen-fade type accepted by fadeScreen. */
type FadeType = "in" | "out";

// ---------------------------------------------------------------------------
// Global game object
// ---------------------------------------------------------------------------

declare const game: GameAPI;

interface GameAPI {
  // -----------------------------------------------------------------------
  // Text & Dialogue
  // -----------------------------------------------------------------------

  /**
   * Display a text box with the given string and wait for the player to
   * dismiss it.
   *
   * @param text - The dialogue text to display.  Supports control codes
   *               such as `<PLAYER>`, `<RIVAL>`, `<LINE>`, `<PAGE>`.
   */
  showText(text: string): Promise<void>;

  /**
   * Show a multiple-choice prompt and return the 0-based index of the
   * option the player selected.
   *
   * @param options - Array of choice labels (e.g. `["YES", "NO"]`).
   * @returns The index of the selected option.
   */
  showChoice(options: string[]): Promise<number>;

  // -----------------------------------------------------------------------
  // Items & Pokémon
  // -----------------------------------------------------------------------

  /**
   * Give an item to the player.
   *
   * @param itemId   - Item identifier string (e.g. `"POTION"`, `"TOWN_MAP"`).
   * @param quantity - Number of items to give (1-99).
   */
  giveItem(itemId: string, quantity: number): Promise<void>;

  /**
   * Remove an item from the player's bag.
   *
   * @param itemId   - Item identifier string.
   * @param quantity - Number of items to take.
   */
  takeItem(itemId: string, quantity: number): Promise<void>;

  /**
   * Give a Pokémon to the player.
   *
   * @param species - Species identifier string (e.g. `"BULBASAUR"`).
   * @param level   - Level of the Pokémon (1-100).
   */
  givePokemon(species: string, level: number): Promise<void>;

  // -----------------------------------------------------------------------
  // Event Flags
  // -----------------------------------------------------------------------

  /**
   * Check a flag's value synchronously.
   *
   * @param flag - Flag name (e.g. `"GOT_STARTER"`, `"BEAT_BROCK"`).
   * @returns `true` if the flag is set, `false` otherwise.
   */
  getFlag(flag: string): boolean;

  /**
   * Set a flag to `true` synchronously.
   *
   * @param flag - Flag name to set.
   */
  setFlag(flag: string): void;

  /**
   * Reset a flag to `false` synchronously.
   *
   * @param flag - Flag name to reset.
   */
  resetFlag(flag: string): void;

  // -----------------------------------------------------------------------
  // Object Visibility
  // -----------------------------------------------------------------------

  /**
   * Make a hidden map object (NPC / item ball) visible.
   *
   * @param objectIndex - 0-based object index within the current map.
   */
  showObject(objectIndex: number): Promise<void>;

  /**
   * Hide a visible map object.
   *
   * @param objectIndex - 0-based object index within the current map.
   */
  hideObject(objectIndex: number): Promise<void>;

  // -----------------------------------------------------------------------
  // NPC & Player Movement
  // -----------------------------------------------------------------------

  /**
   * Move an NPC along a path of waypoints.  Resolves when movement is done.
   *
   * @param npcId - NPC identifier string (e.g. `"OAK"`, `"MOM"`).
   * @param path  - Array of `[x, y]` coordinate pairs forming the path.
   */
  moveNpc(npcId: string, path: [number, number][]): Promise<void>;

  /**
   * Start moving an NPC along a path and resolve immediately.
   * Use `awaitNpcMove` to wait for completion.
   */
  startNpcMove(npcId: string, path: [number, number][]): Promise<void>;

  /** Wait until an NPC started by `startNpcMove` / `startNpcMoveTo` finishes moving. */
  awaitNpcMove(npcId: string): Promise<void>;

  /** Move the player along an explicit path of waypoints. */
  movePlayer(path: [number, number][]): Promise<void>;

  /**
   * Move the player by relative steps, resolved against the player's
   * live position when the command runs. Each entry is either a
   * `[dx, dy]` delta or a direction string ("up"/"down"/"left"/"right").
   *
   * @example
   *   await game.movePlayerRelative([[0, -1], [0, -1]]); // two steps up
   *   await game.movePlayerRelative(["down"]);           // one step down
   */
  movePlayerRelative(steps: ([number, number] | Direction)[]): Promise<void>;

  /**
   * Plan a terrain-aware path for an NPC and move there.
   *
   * @param npcId - NPC identifier string.
   * @param x     - Target tile X coordinate.
   * @param y     - Target tile Y coordinate.
   */
  moveNpcTo(npcId: string, x: number, y: number): Promise<void>;

  /**
   * Plan a terrain-aware path for an NPC, start movement immediately, and
   * resolve without waiting.
   */
  startNpcMoveTo(npcId: string, x: number, y: number): Promise<void>;

  /**
   * Plan a terrain-aware path for the player and move there.
   *
   * @param x - Target tile X coordinate.
   * @param y - Target tile Y coordinate.
   */
  movePlayerTo(x: number, y: number): Promise<void>;

  /**
   * Move an NPC to a target position, then have the player follow
   * automatically.  The entire sequence runs in Rust to avoid JS
   * async race conditions.
   *
   * @param npcId   - NPC identifier string.
   * @param targetX - Target tile X coordinate.
   * @param targetY - Target tile Y coordinate.
   */
  followNpc(npcId: string, targetX: number, targetY: number): Promise<void>;

  /**
   * Teleport an NPC to a tile instantly (no walking). Mirrors the
   * original's sprite-position overwrites used before escort cutscenes.
   *
   * @param npcId - NPC identifier string.
   * @param x     - Target tile X coordinate.
   * @param y     - Target tile Y coordinate.
   */
  setNpcPosition(npcId: string, x: number, y: number): Promise<void>;

  /**
   * Turn an NPC to face a direction.
   *
   * @param npcId     - NPC identifier string.
   * @param direction - Direction to face.
   */
  faceNpc(npcId: string, direction: Direction): Promise<void>;

  /**
   * Turn the player character to face a direction.
   *
   * @param direction - Direction to face.
   */
  facePlayer(direction: Direction): Promise<void>;

  // -----------------------------------------------------------------------
  // Audio
  // -----------------------------------------------------------------------

  /**
   * Start playing a background music track.
   *
   * @param musicId - Music identifier string (e.g. `"PALLET_TOWN"`, `"BATTLE_TRAINER"`).
   */
  playMusic(musicId: string): Promise<void>;

  /**
   * Play a one-shot sound effect.
   *
   * @param soundId - Sound effect identifier string (e.g. `"GET_ITEM"`, `"SAVE"`).
   */
  playSound(soundId: string): Promise<void>;

  /** Stop the currently playing music immediately. */
  stopMusic(): Promise<void>;

  /** Fade out the currently playing music over several frames. */
  fadeOutMusic(): Promise<void>;

  // -----------------------------------------------------------------------
  // Battle
  // -----------------------------------------------------------------------

  /**
   * Start a trainer battle.  Resolves when the battle is finished.
   *
   * @param trainerId - Trainer identifier string (e.g. `"RIVAL_1"`, `"BROCK"`).
   * @returns A result string indicating the battle outcome (e.g. `"win"`, `"lose"`).
   */
  startBattle(trainerId: string): Promise<string>;

  // -----------------------------------------------------------------------
  // Timing & Transitions
  // -----------------------------------------------------------------------

  /**
   * Pause script execution for a number of frames (1 frame ≈ 16.7 ms at 60 fps).
   *
   * @param frames - Number of frames to wait.
   */
  delay(frames: number): Promise<void>;

  /**
   * Warp the player to a different map location.
   *
   * @param map - Map identifier string (e.g. `"PalletTown"`, `"OaksLab"`).
   * @param x   - Destination tile X coordinate.
   * @param y   - Destination tile Y coordinate.
   */
  warpTo(map: string, x: number, y: number): Promise<void>;

  /**
   * Perform a screen fade (in or out).
   *
   * @param fadeType - `"in"` to fade from black, `"out"` to fade to black.
   */
  fadeScreen(fadeType: FadeType): Promise<void>;

  // -----------------------------------------------------------------------
  // Healing
  // -----------------------------------------------------------------------

  /** Fully heal all Pokémon in the player's party. */
  heal(): Promise<void>;

  // -----------------------------------------------------------------------
  // Map Script State
  // -----------------------------------------------------------------------

  /**
   * Set the current map's active script state by name.  The name must
   * match one of the entries in the map's JSON config `mapScripts` array.
   *
   * @param stateName - Name of the script state function to activate.
   */
  setMapScript(stateName: string): Promise<void>;

  // -----------------------------------------------------------------------
  // DSL Scene Management
  // -----------------------------------------------------------------------

  /**
   * Load and display a UI scene defined in DSL (e.g. a shop, menu, or HUD).
   *
   * @param sceneName - Name of the compiled DSL scene to display.
   */
  showScene(sceneName: string): Promise<void>;

  /**
   * Hide a UI scene that was previously shown with `showScene`.
   *
   * @param sceneName - Name of the DSL scene to hide.
   */
  hideScene(sceneName: string): Promise<void>;

  /**
   * Update data bindings for an active UI scene.
   *
   * @param sceneName - Name of the DSL scene to update.
   * @param data      - Arbitrary data object to bind into the scene's UI.
   */
  updateUI(sceneName: string, data: any): Promise<void>;

  // -----------------------------------------------------------------------
  // Player Position
  // -----------------------------------------------------------------------

  /**
   * Get the player's current tile position in the overworld.
   *
   * @returns An object with `x` and `y` tile coordinates.
   */
  getPlayerPosition(): { x: number; y: number };

  /**
   * Get the player's current tile X coordinate in the overworld.
   * Convenience accessor for DSL scripts, which have no member access.
   */
  getPlayerX(): number;

  /**
   * Get the player's current tile Y coordinate in the overworld.
   * Convenience accessor for DSL scripts, which have no member access.
   */
  getPlayerY(): number;

  // -----------------------------------------------------------------------
  // Input Control
  // -----------------------------------------------------------------------

  /**
   * Ignore joypad buttons matching the given bitmask.  Use this to
   * prevent the player from moving during cutscenes.
   *
   * @param mask - Bitmask of buttons to ignore (0xFF = ignore all).
   */
  setJoyIgnore(mask: number): Promise<void>;

  /** Clear the joypad ignore mask, restoring normal input. */
  clearJoyIgnore(): Promise<void>;
}

// ---------------------------------------------------------------------------
// JSON-binding architecture
//
// Map scripts no longer use fixed callback names.  Instead, each map has a
// companion .json config file that binds NPC IDs, signs, coord events, and
// map script states to arbitrarily-named JS functions.  The engine calls
// functions by name as declared in the JSON config.
//
// Example JSON (PalletTown.json):
//   {
//     "mapScripts": ["palletTownDefault", "palletTownOakHeyWait", ...],
//     "npcs": [{ "id": 1, "talk": "talkOak" }, ...],
//     "signs": [{ "id": 1, "talk": "signOakLab" }, ...],
//     "coordEvents": [{ "position": [4, 1], "trigger": "coordNorthExit" }]
//   }
//
// JS files declare exported async functions using ES6 module syntax:
//   export async function palletTownDefault() { ... }
//   export async function talkOak() { ... }
//
// Internal helper functions that are NOT referenced in JSON configs do
// not need the `export` keyword:
//   async function handlePokeBallInteraction(...) { ... }  // internal
// ---------------------------------------------------------------------------
