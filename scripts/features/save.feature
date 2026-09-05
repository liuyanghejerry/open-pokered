Feature: Save roundtrip
  A saved game must restore the exact world through the real boot path
  (main menu -> CONTINUE), not just deserialize internally.

  Scenario: CONTINUE restores map, party, bag and money exactly
    Given a booted game
    And the player has a Pidgey at level 6
    And the player has 5 POTION
    When the player saves the game
    And the game is rebooted from the save
    Then the saved state matches the pre-save snapshot
    And the screen is overworld
