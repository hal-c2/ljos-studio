# Sources:
#   docs/DOGFOOD.md (D13: the Layers panel cannot reorder, rename, hide or lock)
#   docs/MCP.md (`node_arrange`)

Feature: An object is renamed, hidden and locked through `node_arrange`
  A rename writes the file: the `id` and what the document says of it. Hiding
  and locking are Studio's own and never reach the file: a person hides the
  footer to work on pricing, and locks the background so a drag does not take
  it. Both last as long as the session, and an agent has the same four actions.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 300
          height: 200
          Rectangle { id: backdrop; x: 0; width: 300; height: 200 }
          Text { id: footer; text: "Footer" }
          Text { text: footer.text + " footer: " + root.width }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: A rename follows through what the document says of the object
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 300
          height: 200
          Rectangle { id: backdrop; x: 0; width: 300; height: 200 }
          Text { id: credit; text: "Footer" }
          Text { text: credit.text + " footer: " + root.width }
      }
      """

  Scenario: An object with no id gets the one it is named
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"path": "root/2"}], "name": "line", "generation": 1}
      """
    Then the file "Main.qml" contains "id: line"

  Scenario Outline: A name that cannot be an id is refused
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "<name>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "<why>"

    Examples:
      | name     | why      |
      | backdrop | id_taken |
      | Credit   | not_an_id |
      | two word | not_an_id |

  Scenario: A locked object stays where it is, and the file does not change
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "backdrop"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "backdrop"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "locked"
    And the file "Main.qml" contains "id: backdrop"

  Scenario: An object unlocked moves again
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "backdrop"}]}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "unlock", "doc": "Main.qml", "nodes": [{"id": "backdrop"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "backdrop"}]}
      """
    Then the answer's "/nodes" has 1 entry

  Scenario: Only an object with an id can be hidden
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"path": "root/2"}], "generation": 1}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_id"

  @preview
  Scenario: A hidden object leaves the preview and comes back, and the file does not change
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" does not contain "Text#footer"
    And the file "Main.qml" contains "id: footer"
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "show", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#footer"

  @preview
  Scenario: A hidden object stays hidden through a write
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 320}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" does not contain "Text#footer"

  Scenario: A renamed object keeps its lock, and the handle the rename answers works
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    Then the agent keeps the answer's "/nodes/0" as "credit"
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"node": "$credit"}]}
      """
    Then the answer's "/selection/0/id" is "credit"
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "credit"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "locked"

  Scenario: A selected object stays selected through its rename
    Given the agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "credit"}], "add": true}
      """
    Then the answer's "/selection" has 0 entries

  Scenario: A dry run of a lock locks nothing
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "backdrop"}], "dry_run": true}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "backdrop"}]}
      """
    Then the answer's "/nodes" has 1 entry

  @preview
  Scenario: A dry run of a hide hides nothing
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}], "dry_run": true}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#footer"

  @preview
  Scenario: A hidden object stays hidden through its rename
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" does not contain "Text#credit"

  @preview
  Scenario: Hiding in one document leaves the same id in another showing
    Given the file "Other.qml":
      """qml
      import QtQuick

      Item {
          width: 300
          height: 200
          Text { id: footer; text: "Other" }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Other.qml"}
      """
    When the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#footer"

  @preview
  Scenario: A hidden object stays hidden when a mock is switched in
    Given the file "App/Tile/qmldir":
      """
      module App.Tile
      Tile 1.0 Tile.qml
      """
    And the file "App/Tile/Tile.qml":
      """qml
      import QtQuick

      Item { width: 100; height: 100 }
      """
    And the file "mocks/App/Tile/qmldir":
      """
      module App.Tile
      Tile 1.0 Tile.qml
      """
    And the file "mocks/App/Tile/Tile.qml":
      """qml
      import QtQuick

      Item {
          width: 50
          height: 50
          Rectangle { id: stand; width: 50; height: 50 }
      }
      """
    And the file "Wall.qml":
      """qml
      import QtQuick
      import App.Tile

      Item {
          width: 200
          height: 200
          Tile {}
          Text { id: note; text: "note" }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Wall.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Wall.qml", "nodes": [{"id": "note"}]}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Wall.qml", "state": "mocked"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Rectangle#stand"
    And the answer's "/outline" does not contain "Text#note"

  Scenario: A lock holds through the undo and redo of a rename, and unlocking lets go of every name
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    And the agent has called the tool "history" with:
      """json
      {"action": "undo"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "locked"
    Given the agent has called the tool "history" with:
      """json
      {"action": "redo"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "credit"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "locked"
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "unlock", "doc": "Main.qml", "nodes": [{"id": "credit"}]}
      """
    And the agent has called the tool "history" with:
      """json
      {"action": "undo"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    Then the answer's "/nodes" has 1 entry

  @preview
  Scenario: A hidden object stays hidden through an edit of the whole text
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 300
          height: 200
          Text { id: footer; text: "Footer" }
          Text { id: other; text: "Other" }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": 1, "start": 0, "end": 147, "text": "import QtQuick\n\nItem {\n    id: root\n    width: 300\n    height: 200\n    Text { id: footer; text: \"Credit\" }\n    Text { id: other; text: \"Other\" }\n}\n"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#other"
    And the answer's "/outline" does not contain "Text#footer"

  @preview
  Scenario: An edit of the whole text knows a hidden object by its id, as the session does
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 300
          height: 200
          Text { id: footer; text: "Footer" }
          Text { id: other; text: "Other" }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": 1, "start": 0, "end": 147, "text": "import QtQuick\n\nItem {\n    id: root\n    width: 300\n    height: 200\n    Text { id: other; text: \"Footer\" }\n    Text { id: footer; text: \"Other\" }\n}\n"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#other"
    And the answer's "/outline" does not contain "Text#footer"

  @preview
  Scenario: An id a hidden object gave up does not hide the object that takes it
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 300
          height: 200
          Text { id: footer; text: "Footer" }
          Text { id: other; text: "Other" }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "other"}], "name": "footer"}
      """
    When the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#footer"
    And the answer's "/outline" does not contain "Text#credit"
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent calls the tool "node_arrange" with:
      """json
      {"action": "show", "doc": "Main.qml", "nodes": [{"id": "credit"}]}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#credit"
    And the answer's "/outline" does not contain "Text#footer"
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "show", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent calls the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "credit"}]}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#footer"
    And the answer's "/outline" does not contain "Text#credit"

  @preview
  Scenario: A hidden object stays hidden through the undo and redo of its rename
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "hide", "doc": "Main.qml", "nodes": [{"id": "footer"}]}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "nodes": [{"id": "footer"}], "name": "credit"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" does not contain "Text#footer"
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" does not contain "Text#credit"
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "show", "doc": "Main.qml", "nodes": [{"id": "credit"}]}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "preview_screenshot" with:
      """json
      {"format": "outline"}
      """
    Then the answer's "/outline" contains "Text#footer"

  Scenario: A value replaces a property's block
    Given the file "Worked.qml":
      """qml
      import QtQuick

      Item {
          id: root
          property int worked: { return 1 }
      }
      """
    And the studio tools are connected
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Worked.qml", "node": {"id": "root"}, "name": "worked", "value": 3}
      """
    Then the file "Worked.qml" contains "property int worked: 3"
    And the file "Worked.qml" does not contain "return 1"

  Scenario Outline: A value does not replace a property's object
    Given the file "Held.qml":
      """qml
      import QtQuick

      Item {
          id: root
          property <declared>
      }
      """
    And the studio tools are connected
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Held.qml", "node": {"id": "root"}, "name": "maker", "value": 3}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "holds_objects"
    And the file "Held.qml" contains "property <declared>"
    And the file "Held.qml" does not contain "maker: 3"

    Examples:
      | declared                               |
      | Component maker: Component { Item {} } |
      | list<Item> maker: [Item {}, Item {}]   |
