export async function talkNurse() {
  const answer = await game.pokecenterNurseOffer(game.getPokecenterUsed(),
    "Welcome to our\n#MON CENTER!", "We heal your\n#MON back to\nperfect health!",
    "Shall we heal\nyour #MON?", "HEAL", "CANCEL");
  if (answer == 0) {
    await game.printFieldText("OK. We'll need\nyour #MON.", true);
    await game.faceNpc("1", "left");
    await game.delay(3);
    await game.heal();
    await game.animateHealingMachine();
    await game.printFieldText("Thank you!\nYour #MON are\nfighting fit!", true, "prompt");
    await game.setNpcFrame("1", 1);
    await game.delay(20);
  }
  await game.pokecenterFarewell("We hope to see\nyou again!");
  await game.faceNpc("1", "down");
}

export async function talkLinkReceptionist() {
  await game.showText("Welcome to the\nCable Club!");
}
