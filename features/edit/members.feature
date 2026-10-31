# Sources:
#   docs/MCP.md (node_insert)
#   https://doc.qt.io/qt-6/qtqml-syntax-objectattributes.html

Feature: An agent adds a function, a signal or a handler to an object
  `node_insert` takes one member as its text as well as one object: a
  function, a signal, a property declaration or a signal handler. It goes
  after the object's other members, gets no id, and moves no object's path.
  A name the object already has is refused, since QML would not load it.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          property int count: 0

          Text {
              id: label
          }
      }
      """
    And the studio tools are connected

  Scenario: A function goes after the object's members
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "function add() { count += 1 }"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          property int count: 0

          Text {
              id: label
          }
          function add() { count += 1 }
      }
      """

  Scenario Outline: Each kind of member is taken
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "label"}, "text": "<member>"}
      """
    Then the file "Main.qml" contains "<member>"

    Examples:
      | member                       |
      | signal picked(int index)     |
      | property string note         |
      | readonly property int two: 2 |
      | onTextChanged: root.count++  |

  Scenario: Undo takes the member out again
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "signal picked()"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" does not contain "signal picked"

  Scenario Outline: A name the object already has is refused
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "<first>"}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "<again>"}
      """
    Then the agent is refused with "invalid_argument"

    Examples:
      | first              | again                |
      | function f() {}    | function f(a) {}     |
      | signal s()         | signal s(int a)      |
      | onCountChanged: {} | onCountChanged: f()  |
      | signal t()          | property string t    |

  Scenario: A member that would not parse is refused
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "function broken( {"}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: A member has no child index and no import
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "index": 0, "text": "signal picked()"}
      """
    Then the agent is refused with "invalid_argument"
