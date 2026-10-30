# Sources:
#   docs/DESIGN.md (seams: Edit; data flow; S1)
#   ljos-ui crates/ljos_ir ObjectKey (hot reload's object identity)
#   docs/MCP.md (every mutate tool takes the document's generation)

Feature: An object keeps its handle across everyone's edits
  A path like `root/2` is true of one version of a file. A person who
  selected an object, or an agent that listed a tree and edits it three
  calls later, means that object. So Studio hands out handles (`n7`) and
  keeps each pointing at its object through canvas edits, typing in the
  code view and undo. An object that is gone says so instead of naming a
  neighbour, and an edit made against an old generation is refused.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """
    And the studio tools are connected
    And the agent has called the tool "node_find" with:
      """json
      {"doc": "Main.qml", "type": "Text"}
      """
    And the agent keeps the answer's "/nodes/0/node" as "text"
    And the agent has called the tool "node_find" with:
      """json
      {"doc": "Main.qml", "type": "Rectangle"}
      """
    And the agent keeps the answer's "/nodes/0/node" as "rect"

  Scenario: Canvas and code view edits in turn
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "index": 0, "text": "Item {}"}
      """
    When a person types in the code view of "Main.qml", replacing "    Text {" with "    Text { text: \"new\" }\n    Text {"
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$text"}}
      """
    Then the answer's "/path" is "root/3"
    And the answer's "/bindings/0/value" is "\"hello\""
    When the agent calls the tool "node_move" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$rect"}, "parent": {"id": "root"}}
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$text"}}
      """
    Then the answer's "/path" is "root/2"
    And the answer's "/bindings/0/value" is "\"hello\""
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$rect"}}
      """
    Then the answer's "/path" is "root/3"
    And the answer's "/type" is "Rectangle"

  Scenario: A handle holds while the code view leaves the file not parsing
    When a person types in the code view of "Main.qml", replacing "    Text {" with "    Rectangle { Text {"
    And the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Item {}"}
      """
    Then the agent is refused with "syntax"
    And the refusal's "/details/kind" is "broken"
    When a person types in the code view of "Main.qml", replacing "    Rectangle { Text {" with "    Text {"
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$text"}}
      """
    Then the answer's "/type" is "Text"
    And the answer's "/path" is "root/1"

  Scenario: A removed object's handle is stale until undo brings it back
    Given the agent has called the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$rect"}}
      """
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$rect"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "gone"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$rect"}}
      """
    Then the answer's "/type" is "Rectangle"
    And the answer's "/path" is "root/0"

  Scenario: An edit against an old generation is refused and changes nothing
    Given the agent has called the tool "document_text" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent keeps the answer's "/generation" as "read"
    And the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "root"}, "name": "width", "value": 100}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "root"}, "name": "width", "value": 300}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "stale_generation"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 100

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """

  Scenario: The code view's edit needs the generation its offsets were read at
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Item {}"}
      """
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": 1, "start": 0, "end": 14, "text": "import QtQuick 2.15"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/given" is 1
    And the refusal's "/details/current" is 2

  Scenario: A dry run answers with the text and writes nothing
    When the agent calls the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$text"}, "dry_run": true}
      """
    Then the answer's "/history" is absent
    And the answer's "/generation" is 1
    And the answer's "/text" is:
      """qml
      import QtQuick

      Item {
          id: root

          Rectangle { color: "red" }
      }
      """
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """

  Scenario: An object the code view deletes is gone, not its sibling of the same type
    Given the file "Column.qml":
      """qml
      Column {
          Rectangle { width: 1 }
          Rectangle { width: 2 }
      }
      """
    And the agent has called the tool "node_find" with:
      """json
      {"doc": "Column.qml", "type": "Rectangle"}
      """
    And the agent keeps the answer's "/nodes/0/node" as "first"
    And the agent keeps the answer's "/nodes/1/node" as "second"
    When a person types in the code view of "Column.qml", replacing "    Rectangle { width: 1 }\n" with ""
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$first"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "gone"
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$second"}}
      """
    Then the answer's "/path" is "root/0"
    And the answer's "/bindings/0/value" is "2"

  Scenario: An undone insert's handle is gone until redo puts its object back
    Given the file "Column.qml":
      """qml
      Column {
          Rectangle { width: 1 }
      }
      """
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Column.qml", "parent": {"path": "root"}, "generation": 1, "index": 0, "text": "Rectangle { width: 2 }"}
      """
    And the agent keeps the answer's "/node" as "new"
    And the agent has called the tool "history" with:
      """json
      {"action": "undo"}
      """
    When the agent calls the tool "node_remove" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$new"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "gone"
    And the file "Column.qml" is:
      """qml
      Column {
          Rectangle { width: 1 }
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$new"}}
      """
    Then the answer's "/path" is "root/0"
    And the answer's "/bindings/1/value" is "2"

  Scenario: An object typed in the code view is gone after undo and back after redo
    Given the file "Column.qml":
      """qml
      Column {
          Rectangle { width: 1 }
      }
      """
    When a person types in the code view of "Column.qml", replacing "Column {\n" with "Column {\n    Rectangle { width: 2 }\n"
    And the agent calls the tool "node_find" with:
      """json
      {"doc": "Column.qml", "type": "Rectangle"}
      """
    And the agent keeps the answer's "/nodes/0/node" as "typed"
    And the agent keeps the answer's "/nodes/1/node" as "first"
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$typed"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "gone"
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$first"}}
      """
    Then the answer's "/path" is "root/0"
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Column.qml", "node": {"node": "$typed"}}
      """
    Then the answer's "/path" is "root/0"
    And the answer's "/bindings/0/value" is "2"
