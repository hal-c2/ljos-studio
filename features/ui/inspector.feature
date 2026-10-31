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

  Scenario: The picker of a colour an expression works out starts from what the preview shows
    When the person clicks the button "Pick accentColor"
    Then the colour picker holds "#00ff00"
    And the file "Main.qml" says "Qt.rgba(0, 1, 0, 1)"
