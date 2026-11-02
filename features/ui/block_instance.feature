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
          property int count: 0
      }
      """
    And the file "Badge.qml":
      """qml
      import QtQuick

      Plain {
          property int count: 0
          readonly property int total: 1
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

  Scenario: What only the type can say is not offered
    When the person selects "Badge badge" in the layers panel
    Then the inspector has no row "total"

  Scenario: A number's row with nothing written does not offer a word as text
    Given the person has selected "Badge badge" in the layers panel
    When the person sets "count" to "hi" in the inspector
    Then the inspector has no button "Write hi as text"

  # What each type takes is what qml6 loads, measured by writing a string to it.
  Scenario Outline: A type QML writes as a string still offers it in a row with nothing written
    Given the file "Plain.qml":
      """qml
      import QtQuick

      Rectangle {
          property color tint
          property date due
          property point spot
          property size box
          property rect frame
          property vector2d flat
          property vector3d far
          property vector4d wide
          property quaternion turn
          property font face
      }
      """
    And the person has selected "Badge badge" in the layers panel
    When the person sets "<row>" to "<word>" in the inspector
    And the person clicks the button "Write <word> as text"
    Then the file "Main.qml" says "<row>: \"<word>\""

    Examples:
      | row   | word       |
      | tint  | red        |
      | due   | 2020-01-02 |
      | spot  | 1,2        |
      | box   | 1x2        |
      | frame | 1,2,3x4    |
      | flat  | 1,2        |
      | far   | 1,2,3      |
      | wide  | 1,2,3,4    |
      | turn  | 1,2,3,4    |
      | face  | Arial      |

  Scenario Outline: A type QML takes no string for offers none
    Given the file "Plain.qml":
      """qml
      import QtQuick

      Rectangle {
          property int count
          property real ratio
          property bool on
          property matrix4x4 warp
      }
      """
    And the person has selected "Badge badge" in the layers panel
    When the person sets "<row>" to "hi" in the inspector
    Then the inspector has no button "Write hi as text"

    Examples:
      | row   |
      | count |
      | ratio |
      | on    |
      | warp  |

  # `list<int>` cannot be written inside an outline, which takes `<int>` for a column.
  Scenario: A list takes no string and offers none
    Given the file "Plain.qml":
      """qml
      import QtQuick

      Rectangle {
          property list<int> marks
      }
      """
    And the person has selected "Badge badge" in the layers panel
    When the person sets "marks" to "hi" in the inspector
    Then the inspector has no button "Write hi as text"

  # A dump writes a native list as `T` with `list` set, not as `list<T>`.
  Scenario: A list in a type's dump takes no string and offers none
    Given the file "native/ljostypes.json":
      """json
      {"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
        {"name": "Player", "since": "1.0", "base": "Item", "baseModule": "QtQuick",
         "creatable": true, "defaultProperty": null, "attached": null,
         "properties": [
           {"name": "title", "type": "string", "readonly": false, "final": true,
            "required": false, "constant": false, "group": false, "notify": "titleChanged"},
           {"name": "marks", "type": "string", "list": true, "readonly": false, "final": true,
            "required": false, "constant": false, "group": false, "notify": "marksChanged"}],
         "signals": [], "methods": []}]}]}
      """
    And the file "Plain.qml":
      """qml
      import App.Media

      Player {
      }
      """
    And the person has selected "Badge badge" in the layers panel
    When the person sets "marks" to "hi" in the inspector
    Then the inspector has no button "Write hi as text"
    When the person sets "title" to "hi" in the inspector
    And the person clicks the button "Write hi as text"
    Then the file "Main.qml" says "title: \"hi\""

  Scenario: A word in a row with nothing written can be text
    Given the person has selected "Badge badge" in the layers panel
    When the person sets "label" to "hi" in the inspector
    And the person clicks the button "Write hi as text"
    Then the file "Main.qml" says "label: \"hi\""
