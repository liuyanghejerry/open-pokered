Feature: Constructed saves
  SaveBuilder mutates a canonical snapshot template so a scenario can
  boot directly into an arbitrary memory state — money, badges, event
  flags, bag, spawn position and party (Gen-1 stats computed offline,
  verified field-for-field against the engine's create_pokemon) —
  without walking or protocol-seeding.

  Scenario: a constructed save boots straight into the built world state
    Given a constructed save
    And the save has 65000 money
    And the save has 20 ULTRA_BALL
    And the save has the flag EVENT_BEAT_BROCK
    And the save starts on PalletTown at (5,6)
    When the game boots from the save
    Then the player is on PalletTown
    And the player is at (5,6)
    And the player has 65000 money
    And the bag contains 20 ULTRA_BALL
    And the flag EVENT_BEAT_BROCK is set
    And the screen is overworld

  Scenario: a constructed party matches the computed stats exactly
    Given a constructed save
    And the save has a Charizard at level 36
    And the save has a Squirtle at level 7
    When the game boots from the save
    Then the party has 2 Pokemon
    And the party contains a Squirtle
    And the party leader is level 36
    And the party leader has 109 max hp
