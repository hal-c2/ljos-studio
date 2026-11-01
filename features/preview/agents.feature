# Sources:
#   docs/MCP.md (previews; more than one agent)

@preview
Feature: Two agents see one preview
  The preview belongs to the session, not to the agent that opened it. What
  another agent writes shows in it, and it can read what it shows, in the
  browser as in a process of its own.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 300; height: 200
          Text { id: label; text: "hello" }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: A second agent's edit shows in the preview the first opened
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When a second agent named "Ada" calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "from Ada"}
      """
    Then the preview shows "from Ada"
    When a second agent named "Ada" calls the tool "preview_events" with:
      """json
      {"since": 0}
      """
    Then the answer's "/events/0/kind" is "opened"
