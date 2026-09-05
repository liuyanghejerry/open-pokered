Feature: Wild battle flow
  Battles can be entered deterministically (start_wild_battle) and must
  resolve through every exit path: escape, victory, capture, defeat.

  Scenario: running away leaves the player where they stood
    Given a booted game
    And the player has a Bulbasaur at level 5
    When a wild Pidgey at level 3 attacks
    And the player runs from the battle
    Then the screen is overworld
    And the player has not moved

  Scenario: winning a wild battle grants experience
    Given a booted game
    And the player has a Bulbasaur at level 10
    And the leader's experience is recorded
    When a wild Caterpie at level 2 attacks
    And the player fights until the battle ends
    Then the leader has gained experience
    And the screen is overworld

  Scenario: a thrown Poke Ball catches and joins the party
    Given a booted game
    And the player has a Bulbasaur at level 8
    And the player has 20 POKE_BALL
    When a wild Caterpie at level 3 attacks
    And the player throws up to 20 Poke Balls
    Then the party has 2 Pokemon
    And the party contains a Caterpie
    And the bag has fewer than 20 POKE_BALL
    And the screen is overworld

  Scenario: a total party knockout whites out to home, fully healed
    Given a booted game
    And the player has a Rattata at level 2
    When a wild Beedrill at level 30 attacks
    And the player fights until the battle ends
    And the whiteout settles
    Then the player is on PalletTown
    And the party is fully healed
    And the screen is overworld
