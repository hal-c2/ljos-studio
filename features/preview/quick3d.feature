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
