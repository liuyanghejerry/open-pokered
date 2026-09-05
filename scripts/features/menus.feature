Feature: Pause menu and options
  The START menu is driven blind (no protocol snapshot of the pause
  menu), so these specs pin the observable screen transitions instead.
  Remember: And/But inherit the previous keyword, so driving steps that
  follow an assertion are written as explicit When steps.

  Scenario: the party submenu opens and EXIT restores control
    Given a booted game
    And the player has a Bulbasaur at level 5
    And the player has 2 POTION
    Given the pause menu is open
    When the player presses a
    Then the screen is party
    When the player presses b
    Then the screen is start-menu
    When the player closes the menu via EXIT
    Then the screen is overworld

  Scenario: the text speed option takes effect and persists
    Given a booted game
    And the player has a Bulbasaur at level 5
    When the player opens OPTION
    And the player toggles the text speed
    Then the text speed has changed
    When the player leaves the options screen
    And the player opens OPTION
    Then the text speed is still changed

  Scenario: entering the overworld reports the map's NPCs
    Given a booted game
    And the player has a Bulbasaur at level 5
    When the player walks out of the house
    Then the engine reports NPCs on the current map
