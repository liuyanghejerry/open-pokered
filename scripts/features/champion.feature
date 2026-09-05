Feature: Champion post-game roam
  The champion preset claims a finished first playthrough (story flags,
  8 badges, full dex). The acceptance contract: the world must behave
  as story-complete — no story script may hijack the player on routes
  that story beats used to gate.

  Scenario: the champion walks out to Route 1 without Oak's interception
    Given a champion save
    When the game boots from the save
    Then the player has 483700 money
    And the party has 6 Pokemon
    And the flag EVENT_FOLLOWED_OAK_INTO_LAB is set
    And the flag EVENT_BEAT_CHAMPION_RIVAL is set
    When the player walks north out of Pallet Town
    Then the player is on Route1
    And no story script intercepts the player
