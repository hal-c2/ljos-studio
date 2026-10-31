# Sources:
#   docs/MCP.md (blocks_list, node_insert)
#   https://doc.qt.io/qt-6/qml-qtquick3d-view3d.html

@ui
Feature: A person places a 3D view from the palette
  A 3D view is a block like any other: it is placed from the palette, brings
  its import, and starts with a camera, a light and a model, because a view
  with nothing in it draws one colour and the person sees nothing happen.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Item root" in the layers panel

  Scenario: A 3D view starts with something to see
    When the person searches the palette for "View3D"
    And the person places "View3D" from the palette
    Then the file "Main.qml" says "import QtQuick3D"
    And the file "Main.qml" says "View3D {"
    And the file "Main.qml" says "PerspectiveCamera {"
    And the file "Main.qml" says "DirectionalLight {"
    And the file "Main.qml" says "#Cube"
    And the file "Main.qml" says "#40c080"
    And the history lists "insert View3D"

  Scenario: A model placed on its own has a shape and a material
    When the person searches the palette for "Model"
    And the person places "Model" from the palette
    Then the file "Main.qml" says "#Cube"
    And the file "Main.qml" says "materials: PrincipledMaterial {"

  @preview
  Scenario: The objects of a 3D scene have no box and nothing to line up
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    When the person selects "Model" in the layers panel
    Then the window says "This object has no box on the canvas"
    And the inspector has no button "Align Left"

  Scenario: A row that holds a vector stays an expression when it is edited
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    And the person has selected "Model" in the layers panel
    When the person sets "eulerRotation" to "Qt.vector3d(0, 60, 0)" in the inspector
    Then the file "Main.qml" says "eulerRotation: Qt.vector3d(0, 60, 0)"

  Scenario: A literal row still refuses an expression typed by mistake
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    And the person has selected "Model" in the layers panel
    When the person sets "source" to "cube" in the inspector
    Then the window says "cube is not a literal: bind it as an expression"
