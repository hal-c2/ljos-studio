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
