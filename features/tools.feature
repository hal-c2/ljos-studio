# Sources:
#   docs/MCP.md (the tool list, capabilities)
#   ljos-ui docs/ARCHITECTURE.md section 19.7 (capabilities are enforced by the server)

Feature: Studio's tools and who may use them
  The tools come in a fixed order, so a client's prompt cache holds across
  sessions. Reading needs `inspect` and changing a document `mutate`; the
  server enforces it, and a refusal names the capability to ask for.

  Scenario: Every tool, in order
    Given the studio tools are connected
    Then the agent is offered the tools "project_open, document_tree, document_text, document_new, document_edit, node_find, node_select, node_inspect, blocks_list, node_insert, node_move, node_remove, property_set, binding_set, history"

  Scenario: A reader is offered only the reading tools
    Given the studio tools are connected with the capabilities "inspect"
    Then the agent is offered the tools "project_open, document_tree, document_text, node_find, node_select, node_inspect, blocks_list"

  Scenario: A reader cannot change a document
    Given the file "Main.qml":
      """qml
      Item {}
      """
    And the studio tools are connected with the capabilities "inspect"
    When the agent calls the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"path": "root"}}
      """
    Then the agent is refused with "capability_denied"
    And the refusal's "/details/required" is "mutate"
