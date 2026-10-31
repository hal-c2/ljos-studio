# Sources:
#   docs/DESIGN.md (S1: an agent builds a shell from an empty project)
#   docs/MCP.md (project_open, document_tree, document_text)

Feature: An agent opens a project and reads and makes its documents
  `studio --mcp` serves one workspace. `project_open` lists a project's
  QML documents, below the workspace only. `document_tree` lists a
  document's objects with handles; a grouped property is a binding of its
  object, not an object. `document_new` makes a document, refusing one
  that exists or does not parse, and undo removes it again.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          font { bold: true }
          Text { font.pixelSize: 3 }
      }
      """
    And the file "parts/Card.qml":
      """qml
      import QtQuick

      Rectangle {}
      """
    And the studio tools are connected

  Scenario: The project's documents
    When the agent calls the tool "project_open"
    Then the answer's "/documents" lists "Main.qml, parts/Card.qml"

  Scenario: A project outside the workspace is refused
    When the agent calls the tool "project_open" with:
      """json
      {"path": "../elsewhere"}
      """
    Then the agent is refused with "path_denied"

  Scenario: A document's tree with its grouped properties as bindings
    When the agent calls the tool "document_tree" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the answer's "/nodes" has 2 entries
    And the answer's "/nodes/0/bindings" lists "id: root, font.bold: true"
    And the answer's "/nodes/1/path" is "root/0"
    And the answer's "/nodes/1/type" is "Text"
    And the answer's "/nodes/1/bindings" lists "font.pixelSize: 3"
    And the answer's "/generation" is 1

  Scenario: A long tree comes in pages
    When the agent calls the tool "document_tree" with:
      """json
      {"doc": "Main.qml", "limit": 1}
      """
    Then the answer's "/nodes" has 1 entry
    And the answer's "/next" is "1"

  Scenario: A new document, and undo removes it
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "Shell.qml"}
      """
    Then the file "Shell.qml" is:
      """qml
      import QtQuick

      Item {
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Shell.qml" does not exist

  Scenario: A document that exists is not made again
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "exists"

  Scenario: A document that would not parse is not made
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "Broken.qml", "text": "Item {"}
      """
    Then the agent is refused with "syntax"
    And the refusal's "/details/kind" is "broken"
    And the file "Broken.qml" does not exist

  Scenario: Documents are found by type across the project
    When the agent calls the tool "node_find" with:
      """json
      {"type": "Rectangle"}
      """
    Then the answer's "/nodes" has 1 entry
    And the answer's "/nodes/0/doc" is "parts/Card.qml"
    And the answer's "/broken" is []
    And the answer's "/unreadable" is []

  @disk
  Scenario: A document that cannot be read is named and the others are searched
    Given the file "Latin1.qml" holds bytes that are not UTF-8
    When the agent calls the tool "node_find" with:
      """json
      {"type": "Rectangle"}
      """
    Then the answer's "/nodes" has 1 entry
    And the answer's "/nodes/0/doc" is "parts/Card.qml"
    And the answer's "/unreadable" is ["Latin1.qml"]
    And the answer's "/broken" is []
