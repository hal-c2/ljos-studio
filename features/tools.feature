# Sources:
#   docs/MCP.md (the tool list, capabilities)
#   ljos-ui docs/ARCHITECTURE.md section 19.7 (capabilities are enforced by the server)

Feature: Studio's tools and who may use them
  The tools come in a fixed order, so a client's prompt cache holds across
  sessions. Reading needs `inspect` and changing a document `mutate`; the
  server enforces it, and a refusal names the capability to ask for.
  `history` does both: listing it reads, undo and redo change documents.
  Starting a preview runs the project's code and needs `run`; looking at
  one that runs needs only `inspect`. Writing a bundle of the project
  outside its documents (`export`) needs `fs`; review comments are read
  with `inspect` and written with `mutate`.

  Scenario: Every tool, in order
    Given the studio tools are connected
    Then the agent is offered the tools "project_open, document_tree, document_text, document_new, document_edit, node_find, node_select, node_inspect, blocks_list, node_insert, node_move, node_remove, property_set, binding_set, state_edit, slot_contribute, history, theme_set, node_arrange, preview_screenshot, preview_events, review"

  Scenario: Previews are offered to whoever may run the project
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    Then the agent is offered the tools "project_open, document_tree, document_text, document_new, document_edit, node_find, node_select, node_inspect, blocks_list, node_insert, node_move, node_remove, property_set, binding_set, state_edit, slot_contribute, history, theme_set, node_arrange, preview_open, preview_state, preview_screenshot, preview_events, review"

  Scenario: A reader is offered only the reading tools
    Given the studio tools are connected with the capabilities "inspect"
    Then the agent is offered the tools "project_open, document_tree, document_text, node_find, node_select, node_inspect, blocks_list, history, node_arrange, preview_screenshot, preview_events, review"

  Scenario: Exports are offered to whoever may write files
    Given the studio tools are connected with the capabilities "inspect, mutate, fs"
    Then the agent is offered the tools "project_open, document_tree, document_text, document_new, document_edit, node_find, node_select, node_inspect, blocks_list, node_insert, node_move, node_remove, property_set, binding_set, state_edit, slot_contribute, history, theme_set, node_arrange, preview_screenshot, preview_events, export, review"

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

  Scenario: A reader lists the history but cannot undo
    Given the file "Main.qml":
      """qml
      Item {}
      """
    And the studio tools are connected with the capabilities "inspect"
    When the agent calls the tool "history" with:
      """json
      {"action": "list"}
      """
    Then the answer's "/done" has 0 entries
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the agent is refused with "capability_denied"
    And the refusal's "/details/required" is "mutate"
