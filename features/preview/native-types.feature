# Sources:
#   docs/MCP.md (preview_open)
#   docs/DESIGN.md (S6: a block written in Rust; phase 3 / M1)
#   features/edit/blocks.feature (a project's ljostypes.json)
# `ljos run`'s stand-ins and `ljostypes.json` are Ljós's own: Qt has nothing
# to compare them with.

@preview
Feature: A project whose types are written in Rust previews with them
  An application that registers types from Rust describes them in a
  `ljostypes.json`, which may be anywhere in its project (beside the crate
  that writes it, most often). Studio has none of that Rust, so the
  preview runs with every dump the project keeps: each type it describes
  stands in with the members it declares, and the shell around it shows as
  the app's would. One dump is run as it is; several are run as one, their
  modules side by side. In the mocked state a mock module stands in for
  one a dump describes, as it does for a module of QML. A dump that does
  not read stops the preview with the reason, as it would stop the app.

  Background:
    Given the file "native/ljostypes.json":
      """json
      {"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
        {"name": "Player", "since": "1.0", "base": "QtObject", "baseModule": "QtQml",
         "creatable": true, "defaultProperty": null, "attached": null,
         "properties": [{"name": "volume", "type": "int", "kind": "value", "readonly": false,
           "final": true, "required": false, "constant": false, "group": false,
           "notify": "volumeChanged"}],
         "signals": [], "methods": []}]}]}
      """
    And the file "Main.qml":
      """qml
      import QtQuick
      import App.Media

      Column {
          id: root

          Player {
              id: player
              volume: 3
          }
          Text {
              Accessible.role: Accessible.StaticText
              text: "Volume " + player.volume
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: A type the project's dump describes stands in
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the preview shows "Volume 3"

  Scenario: An edit to what is given the stand-in shows at once
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "player"}, "name": "volume", "value": "7"}
      """
    Then the preview shows "Volume 7"

  Scenario: The dumps of a project run as one
    Given the file "sensors/ljostypes.json":
      """json
      {"format": 2, "modules": [{"uri": "App.Sensors", "versions": ["1.0"], "types": [
        {"name": "Meter", "since": "1.0", "base": "QtObject", "baseModule": "QtQml",
         "creatable": true, "defaultProperty": null, "attached": null,
         "properties": [{"name": "level", "type": "int", "kind": "value", "readonly": false,
           "final": true, "required": false, "constant": false, "group": false,
           "notify": "levelChanged"}],
         "signals": [], "methods": []}]}]}
      """
    And the file "Both.qml":
      """qml
      import QtQuick
      import App.Media
      import App.Sensors

      Column {
          Player {
              id: player
              volume: 4
          }
          Meter {
              id: meter
              level: 9
          }
          Text {
              Accessible.role: Accessible.StaticText
              text: "Volume " + player.volume + ", level " + meter.level
          }
      }
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Both.qml"}
      """
    Then the preview shows "Volume 4, level 9"

  Scenario: In the mocked state a mock stands in for a type a dump describes
    Given the file "mocks/App/Media/qmldir":
      """
      module App.Media
      Player 1.0 Player.qml
      """
    And the file "mocks/App/Media/Player.qml":
      """qml
      import QtQml

      QtObject {
          property int volume
          readonly property string from: "the mock"
      }
      """
    And the file "Mocked.qml":
      """qml
      import QtQuick
      import App.Media

      Text {
          Accessible.role: Accessible.StaticText
          text: "Volume " + player.volume + " from " + player.from

          Player {
              id: player
              volume: 5
          }
      }
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Mocked.qml", "state": "mocked"}
      """
    Then the answer's "/mocked" lists "App.Media"
    And the preview shows "Volume 5 from the mock"

  Scenario: A dump that does not read stops the preview with the reason
    Given the file "native/ljostypes.json":
      """json
      {"format": 2, "modules": [
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "start"
    And the refusal's message matches "native/ljostypes\.json"
