# Sources:
#   docs/MCP.md (node_inspect's `members`, node_insert's `replace`,
#   node_remove's `member`, binding_set on a handler)
#   https://doc.qt.io/qt-6/qtqml-syntax-objectattributes.html

Feature: An agent changes a component's API
  A component's API is what an instance of it can set, listen to and call:
  its declared properties, its signals and its functions. `node_inspect`
  answers them in `members`, each with its kind, name, type, value or
  parameters and where it is written. `node_insert` with `replace` writes a
  member in place of the one of that name, so a property is renamed, given
  another type or another default in one edit; `node_remove` with `member`
  takes one out. A handler's value may be several statements, which Studio
  writes as a block. Each is one undo.

  Background:
    Given the file "HabitCard.qml":
      """qml
      import QtQuick

      Rectangle {
          id: card
          property string title: "Read"
          property bool done: false
          signal toggled(bool done)

          function flip() {
              card.done = !card.done
          }

          MouseArea {
              id: area
              anchors.fill: parent
          }
      }
      """
    And the studio tools are connected

  Scenario: The inspector reads the component's API
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "card"}}
      """
    Then the answer's "/members" has 4 entries
    And the answer's "/members/0/kind" is "property"
    And the answer's "/members/0/name" is "title"
    And the answer's "/members/0/type" is "string"
    And the answer's "/members/0/value" is "\"Read\""
    And the answer's "/members/2/kind" is "signal"
    And the answer's "/members/2/params" is "bool done"
    And the answer's "/members/3/kind" is "function"
    And the answer's "/members/3/name" is "flip"

  Scenario: A property is renamed and given another default in one edit
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "HabitCard.qml", "parent": {"id": "card"}, "replace": "title", "text": "property string name: \"Walk\""}
      """
    Then the file "HabitCard.qml" contains:
      """qml
          property string name: "Walk"
          property bool done: false
      """
    And the file "HabitCard.qml" does not contain "title"
    And the history has 1 entry

  Scenario: A property is given another type
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "HabitCard.qml", "parent": {"id": "card"}, "replace": "done", "text": "property int done: 0"}
      """
    Then the file "HabitCard.qml" contains:
      """qml
          property int done: 0
          signal toggled(bool done)
      """

  Scenario: A signal is given another parameter
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "HabitCard.qml", "parent": {"id": "card"}, "replace": "toggled", "text": "signal toggled(bool done, string title)"}
      """
    Then the file "HabitCard.qml" contains "    signal toggled(bool done, string title)"

  Scenario: A function is renamed, its body kept as written
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "HabitCard.qml", "parent": {"id": "card"}, "replace": "flip", "text": "function toggle() {\n    card.done = !card.done\n    card.toggled(card.done)\n}"}
      """
    Then the file "HabitCard.qml" contains:
      """qml
          function toggle() {
              card.done = !card.done
              card.toggled(card.done)
          }
      """

  Scenario Outline: Each kind of member is taken out, and undo puts it back
    When the agent calls the tool "node_remove" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "card"}, "member": "<name>"}
      """
    Then the file "HabitCard.qml" does not contain "<gone>"
    And the file "HabitCard.qml" contains "MouseArea"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "HabitCard.qml" contains "<gone>"

    Examples:
      | name    | gone                    |
      | title   | property string title   |
      | toggled | signal toggled          |
      | flip    | function flip()         |

  Scenario: A member the object does not have is refused, naming those it has
    When the agent calls the tool "node_remove" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "card"}, "member": "colour"}
      """
    Then the agent is refused with "not_found"
    And the refusal's "/details/kind" is "no_member"
    And the refusal's message matches "has no property, signal, function or handler `colour`"

  Scenario: A rename onto a name the object has is refused
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "HabitCard.qml", "parent": {"id": "card"}, "replace": "title", "text": "property bool done"}
      """
    Then the agent is refused with "invalid_argument"
    And the file "HabitCard.qml" contains "property string title: \"Read\""

  Scenario: A handler of several statements is written as a block
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "name": "onClicked", "value": "card.flip(); card.toggled(card.done)"}
      """
    Then the file "HabitCard.qml" contains "onClicked: { card.flip(); card.toggled(card.done) }"

  Scenario: A handler's last comment stays inside its block
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "name": "onClicked", "value": "card.flip(); card.toggled(card.done) // both"}
      """
    Then the file "HabitCard.qml" contains:
      """qml
              onClicked: {
                  card.flip(); card.toggled(card.done) // both
              }
      """

  Scenario: A handler's nested statements keep their indentation
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "name": "onClicked", "value": "if (card.done) {\n    card.flip()\n}\ncard.toggled(card.done)"}
      """
    Then the file "HabitCard.qml" contains:
      """qml
              onClicked: {
                  if (card.done) {
                      card.flip()
                  }
                  card.toggled(card.done)
              }
      """

  Scenario: An evenly indented handler is written level
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "name": "onClicked", "value": "    card.flip()\n    card.toggled(card.done)"}
      """
    Then the file "HabitCard.qml" contains:
      """qml
              onClicked: {
                  card.flip()
                  card.toggled(card.done)
              }
      """

  Scenario: A URL in a string is no comment
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "name": "onClicked", "value": "Qt.openUrlExternally(\"https://example.org\"); card.flip()"}
      """
    Then the file "HabitCard.qml" contains:
      """qml
              onClicked: { Qt.openUrlExternally("https://example.org"); card.flip() }
      """

  Scenario: A handler is changed and taken out by its name
    Given the agent has called the tool "binding_set" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "name": "onClicked", "value": "card.flip()"}
      """
    When the agent calls the tool "node_remove" with:
      """json
      {"doc": "HabitCard.qml", "node": {"id": "area"}, "member": "onClicked"}
      """
    Then the file "HabitCard.qml" does not contain "onClicked"
