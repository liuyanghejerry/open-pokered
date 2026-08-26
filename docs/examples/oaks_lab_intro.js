// oaks_lab_intro.js — example cutscene for the `script-boa` JS fallback
//
// The canonical scripting path is the `.scene` DSL on the native AST
// interpreter (see docs/SCRIPT_ENGINE_DESIGN.md); the Boa JS engine remains
// behind pokered-core's `script-boa` feature as a lower-barrier option for
// custom scripts. This file shows what a cutscene looks like on that path:
// an async function driving the game.* API — fade → warp → dialog →
// choice → givePokemon → flag.

export async function oaksLabIntro() {
  // Fade out, warp into the lab, fade in.
  await game.fadeScreen("out");
  await game.warpTo("OaksLab", 2, 6);
  await game.fadeScreen("in");

  // Oak's introductory dialog.
  await game.showText("OAK: Hello there, young trainer!");
  await game.showText("OAK: The world of POKEMON is vast...");
  await game.showText("OAK: Before you go, choose a partner!");

  // Show the starter selection choice.
  const choice = await game.showChoice([
    "CHARMANDER",
    "SQUIRTLE",
    "BULBASAUR",
  ]);

  // Based on the player's choice, give the corresponding starter.
  if (choice === 0) {
    await game.givePokemon("CHARMANDER", 5);
    await game.showText("OAK: CHARMANDER is a fire type!");
  } else if (choice === 1) {
    await game.givePokemon("SQUIRTLE", 5);
    await game.showText("OAK: SQUIRTLE is a water type!");
  } else {
    await game.givePokemon("BULBASAUR", 5);
    await game.showText("OAK: BULBASAUR is a grass type!");
  }

  // Set the event flag so Oak's lab starter sprites are hidden.
  game.setFlag("HIDE_OAK_LAB_STARTER");

  // Brief pause before the cutscene ends.
  await game.delay(30);
}
