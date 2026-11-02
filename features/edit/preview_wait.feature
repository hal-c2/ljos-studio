# Sources:
#   docs/MCP.md (preview_events: wait_ms)
#   docs/DOGFOOD.md (D4: the picture follows what the app says when it rests)

Feature: A wait on the preview holds up no other call
  An agent may wait an hour on the running app. The server answers every
  other agent all the while, and a wait is over when its client or the server is.

  @preview
  Scenario: A second tool answers while a wait is parked
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle { width: 100; height: 100 }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "preview_events" with:
      """json
      {"wait_ms": 1000}
      """
    Then the tool "preview_events" answers while a wait past the answer's rests is parked, and the wait ends with the server

  @preview
  Scenario Outline: A wait that is no number of milliseconds is refused
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle { width: 100; height: 100 }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent asks the server to wait with:
      """json
      {"wait_ms": <wait>, "rests": 3}
      """
    Then the agent is refused with "invalid_argument"

    Examples:
      | wait   |
      | "5000" |
      | -1     |
