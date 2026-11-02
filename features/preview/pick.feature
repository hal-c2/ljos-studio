# Sources:
#   docs/DESIGN.md (S3: selection sync)
#   docs/MCP.md (node_select; previews)

@preview
Feature: A point in a mocked preview names the file the object was compiled from
  A mock stands in for a module's own files and for nothing nested in it.
  Here, not with the rest of selecting, because the browser's run covers this
  directory: its compile moves a mock's files, which a native preview does not.

  Scenario: A point on a mocked object holds it in the mock's file
    Given the file "App/Tile/qmldir":
      """
      module App.Tile
      Tile 1.0 Tile.qml
      """
    And the file "App/Tile/Tile.qml":
      """qml
      import QtQuick

      Item { width: 100; height: 100; objectName: "the live one, shorter than the mock" }
      """
    And the file "mocks/App/Tile/qmldir":
      """
      module App.Tile
      Tile 1.0 Tile.qml
      """
    And the file "mocks/App/Tile/Tile.qml":
      """qml
      import QtQuick

      Item {
          width: 100; height: 100

          Rectangle {
              id: mocked
              width: 100; height: 100
              color: "red"
          }
      }
      """
    And the file "Wall.qml":
      """qml
      import QtQuick
      import App.Tile

      Item {
          width: 200; height: 200

          Tile {}
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Wall.qml", "state": "mocked"}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Wall.qml", "at": {"x": 30, "y": 30}}
      """
    Then the answer's "/selection/0/doc" is "mocks/App/Tile/Tile.qml"
    And the answer's "/selection/0/id" is "mocked"

  Scenario: A point on a module nested in a mocked one holds it in the live file
    Given the file "App/Tile/qmldir":
      """
      module App.Tile
      Tile 1.0 Tile.qml
      """
    And the file "App/Tile/Tile.qml":
      """qml
      import QtQuick

      Item { width: 100; height: 100 }
      """
    And the file "mocks/App/Tile/qmldir":
      """
      module App.Tile
      Tile 1.0 Tile.qml
      """
    And the file "mocks/App/Tile/Tile.qml":
      """qml
      import QtQuick

      Item { width: 100; height: 100 }
      """
    And the file "App/Tile/Inner/qmldir":
      """
      module App.Tile.Inner
      Inner 1.0 Inner.qml
      """
    And the file "App/Tile/Inner/Inner.qml":
      """qml
      import QtQuick

      Item {
          width: 100; height: 100

          Rectangle {
              id: live
              width: 100; height: 100
              color: "red"
          }
      }
      """
    And the file "Wall.qml":
      """qml
      import QtQuick
      import App.Tile
      import App.Tile.Inner

      Item {
          width: 200; height: 200

          Inner {}
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Wall.qml", "state": "mocked"}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Wall.qml", "at": {"x": 30, "y": 30}}
      """
    Then the answer's "/selection/0/doc" is "App/Tile/Inner/Inner.qml"
    And the answer's "/selection/0/id" is "live"
