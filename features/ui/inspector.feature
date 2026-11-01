# Sources:
#   docs/DOGFOOD.md (D15: the inspector shows colours as text and hides declared properties)

@ui
Feature: The inspector shows colours and every declared property
  A colour has a swatch, and the swatch opens a picker; what the picker sets
  is `property_set`, as the row's own field is. A property declared without a
  value has a row too, so it can be given one.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          property var stack
          property int count: 1
          property color tint: "#80ff0000"
          property color accentColor: Qt.rgba(0, 1, 0, 1)
          color: "#ff0000"
          width: 100
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle root" in the layers panel

  Scenario: A property declared without a value has a row
    When the person sets "stack" to "3" in the inspector
    Then the file "Main.qml" says "stack: 3"

  Scenario: A colour has a swatch and others do not
    Then the inspector has the button "Pick color"
    And the inspector has no button "Pick width"

  Scenario: The picker sets the colour
    When the person clicks the button "Pick color"
    And the person enters "#00ff00" in the colour picker
    Then the file "Main.qml" says "#00ff00"
    And the file "Main.qml" does not say "#ff0000"

  Scenario: The picker takes a colour's name
    When the person clicks the button "Pick color"
    And the person enters "green" in the colour picker
    Then the file "Main.qml" says "green"
    And the file "Main.qml" does not say "#ff0000"

  Scenario: A declared colour given a name keeps its swatch, and the sliders go on from it
    When the person clicks the button "Pick tint"
    And the person enters "#0000ff" in the colour picker
    And the person drags the picker's "Brightness" to its middle
    Then the file "Main.qml" says "#000080"
    When the person clicks the button "Pick tint"
    And the person enters "green" in the colour picker
    Then the file "Main.qml" says "green"
    When the person clicks the button "Pick tint"
    And the person enters "blue" in the colour picker
    Then the file "Main.qml" says "blue"

  Scenario: A value of null is refused for what is declared, and names why
    When the person replaces "property var stack" with "property Component maker: Component { Item {} }" in the code view
    And the person selects "Rectangle root" in the layers panel
    And the person sets "maker" to "null" in the inspector
    Then the window shows a refusal that says "holds an object"
    When the person sets "count" to "null" in the inspector
    Then the window shows a refusal that says "is declared with this value"
    And the file "Main.qml" says "property int count: 1"

  Scenario: The picker writes nothing for what is not a colour
    When the person clicks the button "Pick color"
    And the person enters "#12" in the colour picker
    Then the file "Main.qml" says "#ff0000"
    And the file "Main.qml" does not say "#12"

  Scenario: The picker writes nothing for the empty field of a colour it could not read
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    And the person selects "Rectangle top" in the layers panel
    And the person clicks the button "Pick accentColor"
    And the person enters "" in the colour picker
    Then the file "Main.qml" says "Qt.rgba(0, 1, 0, 1)"

  Scenario: A property worked out by a block has its row, and a value replaces the block
    When the person replaces "property var stack" with "property int worked: { return 1 }" in the code view
    And the person selects "Rectangle root" in the layers panel
    Then the inspector's "worked" holds "{ return 1 }"
    When the person sets "worked" to "3" in the inspector
    Then the file "Main.qml" says "property int worked: 3"
    And the file "Main.qml" does not say "return 1"

  Scenario: A property that holds an object has its row, and a value is refused
    When the person replaces "property var stack" with "property Component maker: Component { Item {} }" in the code view
    And the person selects "Rectangle root" in the layers panel
    Then the inspector's "maker" holds "Component { Item {} }"
    When the person sets "maker" to "3" in the inspector
    Then the window shows a refusal that says "holds an object"
    And the file "Main.qml" says "property Component maker: Component { Item {} }"
    And the file "Main.qml" does not say "maker: 3"
    And the inspector's "maker" holds "Component { Item {} }"

  Scenario: The picker's sliders leave a colour's alpha as it was written
    When the person clicks the button "Pick tint"
    And the person drags the picker's "Brightness" to its middle
    Then the file "Main.qml" says "#80800000"
    And the file "Main.qml" does not say "#80ff0000"

  Scenario: The picker of a colour the running app cannot say gives the reason
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    And the person selects "Rectangle top" in the layers panel
    And the person clicks the button "Pick accentColor"
    Then the window shows a refusal that says "top"
    And the colour picker waits on nothing

  Scenario: The picker of a colour an expression works out starts from what the preview shows
    When the person clicks the button "Pick accentColor"
    Then the colour picker holds "#00ff00"
    And the file "Main.qml" says "Qt.rgba(0, 1, 0, 1)"
