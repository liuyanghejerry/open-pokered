export async function talkNurse() {
  await game.showText("Welcome to our\n#MON CENTER!\n\nWe heal your\n#MON back to\nperfect health!");
  await game.showText("Shall we heal\nyour #MON?");
  var choice = await game.showChoice(["Yes", "No"]);
  if (choice == 0) {
    await game.showText("OK. We'll need\nyour #MON.");
    await game.faceNpc("1", "right");
    await game.delay(8);
    await game.heal();
    await game.animateHealingMachine();
    await game.faceNpc("1", "down");
    await game.delay(8);
    await game.showText("Thank you!\nYour #MON are\nfighting fit!");
  }
  await game.showText("We hope to see\nyou again!");
}

export async function talkLinkReceptionist() {
  await game.showText("Welcome to the\nCable Club!");
}
