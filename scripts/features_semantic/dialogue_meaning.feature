Feature: Dialogue assertions by meaning (TypeSafe System One)
  These scenarios assert what a line *means*, not how it is spelled, so a
  line-break reflow or a harmless rewording does not break them. They
  need a TypeSafe key (TYPESAFE_API_KEY, or the repo-root .env), which is
  why they live outside scripts/features/ and are run explicitly:

      python3 scripts/bdd.py --dir scripts/features_semantic

  The anchor dialogue is the Viridian Forest NPC that
  content_regression.py's forest-npc-position-dialogue case pins by exact
  substring. Both suites assert the same line; one by wording, one by
  meaning. The second scenario is the control: an assertion that cannot
  fail proves nothing, so a claim the line contradicts must be rejected.

  Scenario: the forest NPC's meaning is asserted, not its wording
    Given a constructed save
    And the save has a Squirtle at level 10
    And the save starts on ViridianForest at (16,44)
    When the game boots from the save
    And the player turns to face up
    And the player talks to the object ahead
    Then the dialogue should convey that the speaker came to the forest with friends
    And the dialogue should convey that the speaker's companions are battling Pokemon

  Scenario: a claim the line contradicts is rejected
    Given a constructed save
    And the save has a Squirtle at level 10
    And the save starts on ViridianForest at (16,44)
    When the game boots from the save
    And the player turns to face up
    And the player talks to the object ahead
    Then the dialogue should not convey that the speaker came to the forest alone
    And the dialogue should not convey that the speaker is selling items
