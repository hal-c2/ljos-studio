# Sources:
#   docs/DESIGN.md (S3: selection sync; Studio's shell uses node_select like
#   an agent)
#   docs/MCP.md (node_select; previews)
#   ljos-ui ARCHITECTURE.md section 19.4 (the inspector's `at=X,Y` locator
#   and an object's `source`)

Feature: Studio's selection is set by address or by a point in the preview
  The selection is what the canvas outlines, the inspector shows and the
  code view selects. A person and an agent set it with the same tool, so a
  selection made by either shows to the other. A point in the previewed
  frame selects the object declared where the preview's object at that
  point is declared: the object itself, never one enclosing it.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"

          Rectangle {
              id: card
              x: 10; y: 10; width: 200; height: 120
              color: "grey"

              Rectangle {
                  id: badge
                  x: 10; y: 10; width: 100; height: 60
                  color: "red"
              }
          }
      }
      """

  Scenario: An object is selected by its address
    Given the studio tools are connected
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "card"}]}
      """
    Then the answer's "/selection/0/path" is "root/0"
    And the answer's "/selection/0/id" is "card"

  Scenario: An empty list clears the selection
    Given the studio tools are connected
    And the agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "card"}]}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": []}
      """
    Then the answer's "/selection" has 0 entries

  Scenario: A point needs a preview of the document
    Given the studio tools are connected
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "at": {"x": 30, "y": 30}}
      """
    Then the agent is refused with "not_actionable"

  Scenario: A point and addresses together are refused
    Given the studio tools are connected
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "card"}], "at": {"x": 30, "y": 30}}
      """
    Then the agent is refused with "invalid_argument"

  @preview
  Scenario: A point in the preview selects the object declared there
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "at": {"x": 30, "y": 30}}
      """
    Then the answer's "/selection/0/id" is "badge"
    And the answer's "/selection/0/path" is "root/0/0"

  @preview
  Scenario: A point on an object from another file holds it in that file
    Given the file "Tile.qml":
      """qml
      import QtQuick

      Item {
          width: 100; height: 100

          Rectangle {
              id: tile
              width: 100; height: 100
              color: "red"
          }
      }
      """
    And the file "Wall.qml":
      """qml
      import QtQuick

      Item {
          width: 200; height: 200

          Tile {}
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Wall.qml"}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Wall.qml", "at": {"x": 30, "y": 30}, "hold": true}
      """
    Then the answer's "/selection/0/doc" is "Tile.qml"
    When a second agent named "Ada" calls the tool "property_set" with:
      """json
      {"doc": "Tile.qml", "node": {"id": "tile"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "busy"
    And the refusal's "/details/kind" is "leased"
