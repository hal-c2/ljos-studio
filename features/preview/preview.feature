# Sources:
#   docs/DESIGN.md (S2: preview; data flow; latency budget)
#   docs/MCP.md (preview_open, preview_state, preview_screenshot, preview_events)
#   ljos-ui docs/ARCHITECTURE.md section 12.1 (hot reload keeps state) and
#   section 19.4 (the inspector of a running application)

@preview
Feature: A shell under edit runs in a preview
  The preview is the shell running as the shipped app runs it, in a process
  of its own. Every edit, by a person or an agent, and every undo shows in
  it with the app's state kept. The same shell previews with the app's real
  blocks (live) or with mock modules standing in for them (mocked); the
  shell's text is the same either way.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      """
    And the file "App/Media/Player.qml":
      """qml
      import QtQuick

      Item {
          property url source
          readonly property bool playing: false
          function play() {}
      }
      """
    And the file "mocks/App/Media/qmldir":
      """
      module App.Media
      Player 1.0 Player.qml
      """
    And the file "mocks/App/Media/Player.qml":
      """qml
      import QtQuick

      Item {
          property url source
          property bool playing: true
          function play() { playing = true }
      }
      """
    And the file "Shell.qml":
      """qml
      import QtQuick
      import App.Media

      Column {
          Player { id: player1 }
          Text { objectName: "status"; Accessible.role: Accessible.StaticText; text: player1.playing ? "Playing" : "Stopped" }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Rule: A preview runs the shell

    Scenario: A preview on a phone-sized target
      When the agent calls the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "target": "phone"}
        """
      Then the answer's "/target" is "phone"
      And the answer's "/size/width" is 390
      And the answer's "/size/height" is 844
      And the answer's "/state" is "live"
      And the preview shows "Stopped"

    Scenario: A document that is not there has no preview
      When the agent calls the tool "preview_open" with:
        """json
        {"doc": "Missing.qml"}
        """
      Then the agent is refused with "not_found"

    Scenario Outline: A size is 1 to 8192 pixels on each side
      When the agent calls the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "size": {"width": <width>, "height": <height>}}
        """
      Then the agent is refused with "invalid_argument"

      Examples:
        | width | height |
        | 0     | 600    |
        | 800   | 0      |
        | 8193  | 600    |
        | 800   | 100000 |

    Scenario: Previews need the capability to run the project
      Given the studio tools are connected with the capabilities "inspect, mutate"
      When the agent calls the tool "preview_open" with:
        """json
        {"doc": "Shell.qml"}
        """
      Then the agent is refused with "capability_denied"

  Rule: Edits and undo show in the preview

    Background:
      Given the agent has called the tool "preview_open" with:
        """json
        {"doc": "Shell.qml"}
        """

    Scenario: An agent's edit shows in the preview
      When the agent calls the tool "binding_set" with:
        """json
        {"doc": "Shell.qml", "generation": 1, "node": {"path": "root/1"}, "name": "text", "value": "\"Ready\""}
        """
      Then the preview shows "Ready"
      When the agent calls the tool "preview_events" with:
        """json
        {"since": 1}
        """
      Then the answer's "/events/0/kind" is "reloaded"
      And the answer's "/events/0/generation" is 1

    # A wait holds up nobody and asks the app nothing, so a window or an agent
    # can stay on it for as long as it wants the picture to be current.
    Scenario: Waiting on the preview says when its app wakes next
      When the agent calls the tool "preview_events" with:
        """json
        {"wait_ms": 1000}
        """
      Then the answer's "/open" is true
      And the answer's "/events" has 0 entries
      And the agent keeps the answer's "/rests" as "rests"

    Scenario: Undo shows in the preview
      Given the agent has called the tool "binding_set" with:
        """json
        {"doc": "Shell.qml", "generation": 1, "node": {"path": "root/1"}, "name": "text", "value": "\"Ready\""}
        """
      When the agent calls the tool "history" with:
        """json
        {"action": "undo"}
        """
      Then the preview shows "Stopped"

    Scenario: Text that does not parse keeps the last good preview
      When a person types in the code view of "Shell.qml", replacing "Column {" with "Column {{"
      Then the preview shows "Stopped"
      When the agent calls the tool "preview_events" with:
        """json
        {"since": 1}
        """
      Then the answer's "/events/0/kind" is "reload_failed"

    Scenario: A picture of the preview
      When the agent calls the tool "preview_screenshot" with:
        """json
        {"max_size": 200}
        """
      Then the answer has a PNG picture at most 200 pixels on its longest side

  Rule: Live and mocked state preview the same shell

    Scenario: Mock modules stand in for the app's blocks
      When the agent calls the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "state": "mocked"}
        """
      Then the answer's "/state" is "mocked"
      And the answer's "/mocked" lists "App.Media"
      And the preview shows "Playing"

    Scenario: Switching between live and mocked keeps the shell's text
      Given the agent has called the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "state": "mocked"}
        """
      When the agent calls the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "state": "live"}
        """
      Then the answer's "/started" is false
      And the preview shows "Stopped"
      And the file "Shell.qml" is:
        """qml
        import QtQuick
        import App.Media

        Column {
            Player { id: player1 }
            Text { objectName: "status"; Accessible.role: Accessible.StaticText; text: player1.playing ? "Playing" : "Stopped" }
        }
        """

    Scenario: A mocked block's state is set from outside
      Given the agent has called the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "state": "mocked"}
        """
      When the agent calls the tool "preview_state" with:
        """json
        {"node": {"id": "player1"}, "set": {"playing": false}}
        """
      Then the answer's "/block" is "App.Media.Player"
      And the answer's "/state/playing" is false
      And the preview shows "Stopped"

    Scenario: A live block's state is the app's own
      Given the agent has called the tool "preview_open" with:
        """json
        {"doc": "Shell.qml"}
        """
      When the agent calls the tool "preview_state" with:
        """json
        {"node": {"id": "player1"}}
        """
      Then the answer's "/state/playing" is false
      When the agent calls the tool "preview_state" with:
        """json
        {"node": {"id": "player1"}, "set": {"playing": true}}
        """
      Then the agent is refused with "not_actionable"
