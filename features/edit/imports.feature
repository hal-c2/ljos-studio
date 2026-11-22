# Sources:
#   docs/MCP.md (node_insert, binding_set, state_edit)
#   $S dogfood backlog P7-11 (an edit that writes a type or a singleton
#   leaves the file without its import, and the preview stops loading)
#   qml6 6.11, measured: `Colors.accent` without `import Ljos.Design` is a
#   ReferenceError, `ColumnLayout {}` without `import QtQuick.Layouts` stops
#   the file loading

Feature: An edit brings the imports what it writes needs
  An agent that writes `ColumnLayout {}` or `color: Colors.accent` means
  the toolkit's layout and the design module's colours. The edit that
  writes them adds the import each needs, once, after the file's others,
  so the file goes on loading. What the file can already name, or names
  under another word, it leaves as it is.

  Background:
    Given the file "Plain.qml":
      """qml
      import QtQuick

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  Scenario: A toolkit type the file does not import brings its module
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "ColumnLayout {}"}
      """
    Then the answer's "/id" is "columnLayout1"
    And the file "Plain.qml" is:
      """qml
      import QtQuick
      import QtQuick.Layouts

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
          ColumnLayout { id: columnLayout1 }
      }
      """

  Scenario: The answer tells the import and the object as two changes
    Given the file "Long.qml":
      """qml
      import QtQuick

      Item {
          id: long
          width: 200
          height: 100
          Rectangle { id: one; width: 10 }
          Rectangle { id: two; width: 20 }
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Long.qml", "parent": {"id": "long"}, "text": "ColumnLayout {}"}
      """
    Then the answer's "/changes" has 2 entries
    And the answer's "/changes/0/text" is "import QtQuick.Layouts\n"
    And the answer's "/changes/1/text" contains "ColumnLayout"
    And the answer's "/changes/1/text" does not contain "Rectangle"

  Scenario: An object holding types of several modules brings each
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "ColumnLayout { Button {} }"}
      """
    Then the file "Plain.qml" contains "import QtQuick.Layouts"
    And the file "Plain.qml" contains "import QtQuick.Controls"

  Scenario: The imports an agent names come as a list
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Rectangle { color: Colors.accent; RowLayout {} }", "import": ["QtQuick.Layouts", "Ljos.Design"]}
      """
    Then the file "Plain.qml" contains "import QtQuick.Layouts"
    And the file "Plain.qml" contains "import Ljos.Design"

  Scenario Outline: A binding that reads a design singleton brings the design module
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Plain.qml", "node": {"id": "card"}, "name": "<name>", "value": "<value>"}
      """
    Then the file "Plain.qml" contains:
      """qml
      import QtQuick
      import Ljos.Design

      Item {
      """

    Examples:
      | name         | value                          |
      | color        | Colors.accent                  |
      | radius       | Spacing.sm                     |
      | border.color | dark ? Colors.border : \"red\" |

  Scenario: An object put with a design colour brings the design module
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Rectangle { color: Colors.surface }"}
      """
    Then the file "Plain.qml" contains "import Ljos.Design"

  Scenario: A state's change that reads a design singleton brings the design module
    Given the agent has called the tool "state_edit" with:
      """json
      {"doc": "Plain.qml", "action": "add", "state": "on"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Plain.qml", "action": "change", "state": "on", "target": {"id": "card"}, "name": "color", "binding": "Colors.accent"}
      """
    Then the file "Plain.qml" contains "import Ljos.Design"

  Scenario Outline: Words that only read like a singleton bring nothing
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Plain.qml", "node": {"id": "card"}, "name": "<name>", "value": "<value>"}
      """
    Then the file "Plain.qml" does not contain "Ljos.Design"

    Examples:
      | name       | value                  |
      | objectName | \"Colors.accent\"      |
      | width      | card.Colors.size       |
      | width      | /* Colors.accent */ 10 |

  Scenario Outline: A file that names the module already keeps its imports
    Given the file "Named.qml":
      """qml
      import QtQuick
      <import>

      Rectangle {
          id: named
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Named.qml", "node": {"id": "named"}, "name": "color", "value": "<value>"}
      """
    Then the file "Named.qml" contains:
      """qml
      import QtQuick
      <import>

      Rectangle {
      """

    Examples:
      | import                  | value           |
      | import Ljos.Design      | Colors.accent   |
      | import Ljos.Design as D | D.Colors.accent |
