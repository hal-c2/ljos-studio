# Sources:
#   docs/MCP.md (node_arrange's `component_extract`)

Feature: An object becomes a component of the project
  `node_arrange`'s `component_extract` writes the object's text to a file beside
  the document and leaves an instance of it where it was, with the id and the
  place. One undo takes back both files. An id only the new file can see would
  break what reads it, so a read of a property becomes an alias, and what no
  alias can carry is refused, naming it.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: root
          property int count: 0

          // The card.
          Rectangle {
              id: card
              x: 10
              y: 20
              width: 100
              color: "gold"

              Text {
                  id: label
                  text: "Hi"
              }
          }
      }
      """
    And the studio tools are connected

  Scenario: The object goes to a file and its instance keeps the id and the place
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the answer's "/component" is "Card.qml"
    And the answer's "/aliases" is absent
    And the file "Card.qml" is:
      """qml
      import QtQuick
      import QtQuick.Controls

      Rectangle {
          id: card
          width: 100
          color: "gold"

          Text {
              id: label
              text: "Hi"
          }
      }
      """
    And the file "Main.qml" is:
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: root
          property int count: 0

          // The card.
          Card {
              id: card
              x: 10
              y: 20
          }
      }
      """
    And the history has 1 entry

  Scenario: One undo takes back both files, and one redo brings them again
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Card.qml" does not exist
    And the file "Main.qml" contains "Rectangle {"
    And the file "Main.qml" does not contain "Card {"
    And the answer's "/entry/label" is "component_extract"
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    Then the file "Card.qml" exists
    And the file "Main.qml" contains "Card {"
    And the history has 1 entry
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Card.qml" does not exist

  Scenario: An undo that would take another writer's work with it leaves both files
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Card.qml", "node": {"id": "card"}, "name": "width", "value": 200}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/author" is "Ada"
    And the file "Card.qml" contains "width: 200"
    And the file "Main.qml" contains "Card {"

  Scenario: A new file taken away outside Studio leaves the undo to the document
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    When the file "Card.qml" is deleted outside Studio
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "Rectangle {"
    And the file "Card.qml" does not exist

  Scenario: Anchors stay on the instance, however they are written
    Given the file "Page.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              id: one
              anchors.fill: parent
              anchors.margins: 4
              color: "red"
          }
          Rectangle {
              id: two
              anchors { left: parent.left; top: parent.top }
              color: "blue"
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Page.qml", "nodes": [{"id": "one"}], "name": "One"}
      """
    And the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Page.qml", "nodes": [{"id": "two"}], "name": "Two"}
      """
    Then the file "Page.qml" contains "anchors.fill: parent"
    And the file "Page.qml" contains "anchors.margins: 4"
    And the file "Page.qml" contains "anchors { left: parent.left; top: parent.top }"
    And the file "One.qml" does not contain "anchors"
    And the file "Two.qml" does not contain "anchors"
    And the file "Two.qml" contains "color: \"blue\""

  Scenario: A property read from outside becomes an alias, only where it has to
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              id: card
              Text {
                  id: label
                  text: "Hi"
              }
          }
          Text {
              // label.text in a comment stays
              text: label.text + " label.text " + card.width
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the answer's "/aliases/0" is "labelText"
    And the answer's "/aliases" has 1 entry
    And the file "Card.qml" contains "property alias labelText: label.text"
    And the file "Reads.qml" contains "// label.text in a comment stays"
    And the file "Reads.qml" contains "text: card.labelText + \" label.text \" + card.width"

  Scenario: An alias is declared after the id, which stays first
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              id: card
              Text {
                  id: label
                  text: "Hi"
              }
          }
          Text {
              text: label.text
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the file "Card.qml" is:
      """qml
      import QtQuick

      Rectangle {
          id: card
          property alias labelText: label.text
          Text {
              id: label
              text: "Hi"
          }
      }
      """

  Scenario: An alias does not take the comment on the id's line
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              id: card // the card
              Text {
                  id: label
                  text: "Hi"
              }
          }
          Text {
              text: label.text
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the file "Card.qml" is:
      """qml
      import QtQuick

      Rectangle {
          id: card // the card
          property alias labelText: label.text
          Text {
              id: label
              text: "Hi"
          }
      }
      """

  Scenario: An object written on one line still takes an alias
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          Rectangle { id: card; color: "gold"; Text { id: label; text: "Hi" } }
          Text { text: label.text }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the file "Card.qml" is:
      """qml
      import QtQuick

      Rectangle { id: card;
          property alias labelText: label.text
          color: "gold"; Text { id: label; text: "Hi" } }
      """

  Scenario: An instance read through an alias gets an id if it had none
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              Text {
                  id: label
              }
          }
          Text {
              text: label.text
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"path": "root/0"}], "generation": 1, "name": "Card"}
      """
    Then the answer's "/component" is "Card.qml"
    And the file "Reads.qml" contains "id: card"
    And the file "Reads.qml" contains "text: card.labelText"

  Scenario Outline: A use of an inner id no alias can carry is refused, naming it
    Given the file "Uses.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              id: card
              Text {
                  id: label
              }
          }
          Item {
              <use>
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Uses.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "reference_out"
    And the refusal's message matches "label"
    And the file "Card.qml" does not exist

    Examples:
      | use                                    |
      | width: label.implicitWidth()           |
      | property var it: label                 |
      | Component.onCompleted: label.clicked.connect(go) |
      | width: true ? label : 0                |

  Scenario Outline: What the object reads that stays behind is refused, naming it
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          id: root
          property int count: 0
          Item {
              id: other
          }
          Rectangle {
              id: card
              <reads>
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "reference_in"
    And the refusal's message matches "<name>"
    And the file "Card.qml" does not exist

    Examples:
      | reads                 | name   |
      | width: other.width    | other  |
      | height: count         | count  |
      | width: parent.width   | parent |

  Scenario: The object's own place may read its parent
    Given the file "Reads.qml":
      """qml
      import QtQuick

      Item {
          Rectangle {
              id: card
              x: parent.width / 2
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Reads.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the file "Reads.qml" contains "x: parent.width / 2"
    And the file "Card.qml" does not contain "parent"

  Scenario Outline: A name that is no type, or a file there already, is refused
    Given the file "Taken.qml":
      """qml
      Item {}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "<name>"}
      """
    Then the agent is refused with "<code>"
    And the file "Main.qml" contains "Rectangle {"

    Examples:
      | name       | code             |
      | card       | invalid_argument |
      | ../Evil    | invalid_argument |
      | Taken      | not_actionable   |

  Scenario: The document's root is a component already
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "root"}], "name": "Whole"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "root"

  Scenario: A dry run writes nothing
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Card", "dry_run": true}
      """
    Then the answer's "/component" is "Card.qml"
    And the file "Card.qml" does not exist
    And the file "Main.qml" contains "Rectangle {"
    And the history has 0 entries

  Scenario: An object inside an inline component has another scope of ids
    Given the file "Inline.qml":
      """qml
      import QtQuick

      Item {
          component Tile: Rectangle {
              Text {
                  id: label
              }
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Inline.qml", "nodes": [{"id": "label"}], "name": "Label"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "in_component"

  Scenario: A use of the document's inline component stays behind with it
    Given the file "Inline.qml":
      """qml
      import QtQuick

      Item {
          component Tile: Rectangle {}
          Rectangle {
              id: card
              Tile {}
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Inline.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's message matches "Tile"

  Scenario: Several lines' worth of a string keep their spaces
    Given the file "Text.qml":
      """qml
      import QtQuick

      Item {
          Text {
              id: words
              text: "one
                  two"
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Text.qml", "nodes": [{"id": "words"}], "name": "Words"}
      """
    Then the file "Words.qml" is:
      """qml
      import QtQuick

      Text {
          id: words
          text: "one
                  two"
      }
      """

  Scenario: A locked object stays
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "card"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Card"}
      """
    Then the agent is refused with "not_actionable"
    And the file "Card.qml" does not exist

  Scenario: One object at a time
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}, {"id": "label"}], "name": "Card"}
      """
    Then the agent is refused with "invalid_argument"
