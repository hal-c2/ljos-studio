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

  @preview
  # The camera looks down z, so the z axis is a point on the picture: there
  # is nothing to drag, and its row in the inspector is where z is set.
  Scenario: A model of a 3D scene is moved by the arrows the picture can show
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    When the person selects "Model" in the layers panel
    Then the canvas shows 2 move arrows

  # The view is 320 by 240 at the corner and its camera 300 units away, so a
  # unit is 0.69 pixel at the model: the 40 pixels of the drag are 58 units.
  @preview
  Scenario: Dragging an arrow moves the model along its axis as one undo step
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    And the person has selected "Model" in the layers panel
    When the person drags in the frame "Desktop" from 200, 120 to 240, 120
    Then the file "Main.qml" says "x: 58"
    And the history lists "place"
    When the person presses "Ctrl+Z"
    Then the file "Main.qml" does not say "x: 58"

  @preview
  Scenario: A click on the picture of the model selects the model
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    When the person clicks the frame "Desktop" at 160, 120
    Then the layers panel has "Model" selected

  @preview
  Scenario: With the hand a drag over a 3D view writes nothing
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    And the person has clicked the button "Hand"
    When the person drags in the frame "Desktop" from 100, 100 to 200, 150
    Then the file "Main.qml" does not say " x: "

  @preview
  Scenario: A view has no arrows
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    When the person selects "View3D view3D1" in the layers panel
    Then the canvas shows 0 move arrows

  @preview
  Scenario: The line-up buttons come back with an object that has a box
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    And the person has selected "Model" in the layers panel
    When the person selects "View3D view3D1" in the layers panel
    Then the inspector has the button "Align Left"

  @preview
  Scenario: A selection with one object that has a box keeps the line-up buttons
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    When an agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "view3D1"}, {"path": "root/0/2"}]}
      """
    Then the inspector has the button "Align Left"

  Scenario: A material's colour is picked in its own row
    Given the person has searched the palette for "View3D"
    And the person has placed "View3D" from the palette
    And the person has selected "PrincipledMaterial" in the layers panel
    When the person clicks the button "Pick baseColor"
    And the person enters "#c04040" in the colour picker
    Then the file "Main.qml" says "#c04040"
    And the file "Main.qml" does not say "#40c080"

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
