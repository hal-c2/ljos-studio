# Sources:
#   docs/DESIGN.md (S4: tokens, shells and the self-reskin)
#   docs/MCP.md (#22 theme_set, preview_events)
#   ljos-ui features/design/theme.feature (a theme edit sets the roles
#   again and makes nothing again)

@preview
Feature: A token set shows in the open preview
  The preview runs with the theme file beside its entry, watched as Ljós
  watches it. A token `theme_set` writes reaches the running preview with
  its state kept, and the reload is reported like any other.

  Background:
    Given the file "app/Gap.qml":
      """qml
      import QtQuick
      import Ljos.Design

      Text {
          Accessible.role: Accessible.StaticText
          text: "gap " + Spacing.md
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "app/Gap.qml"}
      """

  Scenario: A token set shows in the preview and is reported as a reload
    Then the preview shows "gap 16"
    When the agent calls the tool "theme_set" with:
      """json
      {"token": "Spacing.md", "value": 20}
      """
    Then the answer's "/doc" is "app/theme.json"
    And Studio has reported the preview reloaded since it was read
    And the preview shows "gap 20"
    When the agent calls the tool "preview_events" with:
      """json
      {"since": 1}
      """
    Then the answer's "/events/0/kind" is "reloaded"

  Scenario: Undo of a token shows the module's value again
    Given the agent has called the tool "theme_set" with:
      """json
      {"token": "Spacing.md", "value": 20}
      """
    Then the preview shows "gap 20"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then Studio has reported the preview reloaded since it was read
    And the preview shows "gap 16"
