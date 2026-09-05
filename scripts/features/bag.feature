Feature: Bag and party stores
  The debug protocol's write commands must land in the same save-data
  stores the game itself reads back.

  Scenario: given items reach the bag with their quantities
    Given a fresh game
    And the player has 10 POKE_BALL
    And the player has 3 POTION
    Then the bag contains 10 POKE_BALL
    And the bag contains 3 POTION

  Scenario: the engine rejects unknown items instead of guessing
    Given a fresh game
    When the driver attempts to give NOT_AN_ITEM
    Then the engine refuses the command

  Scenario: given Pokemon join the party in order
    Given a fresh game
    And the player has a Bulbasaur at level 5
    And the player has a Pidgey at level 4
    And the player has a Rattata at level 3
    Then the party has 3 Pokemon
    And the party contains a Pidgey
