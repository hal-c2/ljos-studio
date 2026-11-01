# Sources:
#   docs/DOGFOOD.md (a block placed by hand showed nothing to set)

@ui
Feature: An object of the project's own type says what it takes
  A person who places a block has not read its file. The inspector lists what
  the type declares, with nothing written, and a row is how one gets a value.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          Badge { id: badge; width: 80 }
          Text { text: badge.width }
      }
      """
    And the file "Plain.qml":
      """qml
      import QtQuick

      Rectangle {
          property string label: "new"
      }
      """
    And the file "Badge.qml":
      """qml
      import QtQuick

      Plain {
          property int count: 0
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: What the type declares is listed, and what its base declares
    When the person selects "Badge badge" in the layers panel
    Then the inspector's "count" holds ""
    And the inspector's "label" holds ""
    And the inspector's "width" holds "80"

  Scenario: A value goes in the file as any row's does
    Given the person has selected "Badge badge" in the layers panel
    When the person sets "count" to "3" in the inspector
    Then the file "Main.qml" says "count: 3"

  Scenario: The inspector's id names the object everywhere it is named
    Given the person has selected "Badge badge" in the layers panel
    When the person sets "id" to "chip" in the inspector
    Then the file "Main.qml" says "Badge { id: chip;"
    And the file "Main.qml" says "text: chip.width"
    And the window shows no refusal

  Scenario: An id that cannot be one is refused and the field goes back
    Given the person has selected "Badge badge" in the layers panel
    When the person sets "id" to "not an id" in the inspector
    Then the file "Main.qml" says "Badge { id: badge;"
    And the inspector's "id" holds "badge"
