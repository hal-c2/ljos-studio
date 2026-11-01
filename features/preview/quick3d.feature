# Sources:
#   docs/MCP.md (node_insert, property_set, preview_screenshot)
#   ljos-ui docs/quick3d.md
#   https://doc.qt.io/qt-6/qml-qtquick3d-view3d.html

@preview
Feature: A 3D view in the preview
  A View3D is an object of the document like any other: an agent places it,
  edits what is inside it, and sees the scene in the preview's picture.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 320
          height: 240
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_insert" with:
      """json
      {
        "doc": "Main.qml",
        "parent": {"id": "root"},
        "import": "QtQuick3D",
        "text": "View3D { anchors.fill: parent\n environment: SceneEnvironment { backgroundMode: SceneEnvironment.Color; clearColor: \"#101010\" }\n PerspectiveCamera { z: 300 }\n DirectionalLight { eulerRotation.x: -30 }\n Model { id: cube; source: \"#Cube\"; materials: PrincipledMaterial { id: paint; baseColor: \"#40c080\" } } }"
      }
      """

  Scenario: The picture shows the scene
    When the agent calls the tool "preview_screenshot" with:
      """json
      {"max_size": 320}
      """
    Then the answer's picture has some green

  Scenario: An edit inside the scene reaches the picture
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "paint"}, "name": "baseColor", "value": "#c04040"}
      """
    When the agent calls the tool "preview_screenshot" with:
      """json
      {"max_size": 320}
      """
    Then the answer's picture has some red
    And the answer's picture has no green

  Scenario: The running scene answers for its objects
    When the agent calls the tool "preview_state" with:
      """json
      {"doc": "Main.qml", "node": {"id": "paint"}, "read": ["baseColor"]}
      """
    Then the answer's "/state/baseColor" is "#40c080"

  Scenario: A point on a model selects the model, not the view
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "at": {"x": 640, "y": 400}}
      """
    Then the answer's "/selection/0/id" is "cube"

  Scenario: A point on the view beside the model selects the view
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "at": {"x": 8, "y": 8}}
      """
    Then the answer's "/selection/0/type" is "View3D"

  Scenario: The face of the model ends where the picture shows it ending
    # 100 units across at 250 units from a camera of 60 degrees over the
    # window's 800 pixels: 277 pixels, so 138 either side of the middle.
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "at": {"x": 770, "y": 400}}
      """
    Then the answer's "/selection/0/id" is "cube"
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "at": {"x": 786, "y": 400}}
      """
    Then the answer's "/selection/0/type" is "View3D"

  Scenario: A model is told where its axes go on the picture
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "geometry", "doc": "Main.qml", "nodes": [{"id": "cube"}]}
      """
    Then the answer's "/boxes/0/scene/origin/2" is 0.0
    And the answer's "/boxes/0/scene/eye/position/2" is 300.0
    And the answer's "/boxes/0/scene/local/0" is 0.0

  Scenario: A model is moved along an axis as one undo step
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "cube"}, "x": 20, "z": -30}]}
      """
    Then the file "Main.qml" contains "x: 20"
    And the file "Main.qml" contains "z: -30"
    And the history has 2 entries

  Scenario: A model whose axis is an expression is not moved along it
    Given the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "cube"}, "name": "x", "value": "root.width / 10"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "cube"}, "x": 20}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "bound"
    And the file "Main.qml" contains "x: root.width / 10"
