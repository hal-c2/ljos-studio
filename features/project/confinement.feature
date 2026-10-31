# Sources:
#   docs/DESIGN.md (every path a tool takes is confined to the open project)
#   docs/MCP.md (`mutate` changes documents; `fs` writes anything else)

Feature: The document tools reach the project's documents and nothing else
  `mutate` is the right to change a document, not any file: `.git/config`
  sets what git runs next, and file contents may steer an agent into asking.
  So a document is a `.qml` file outside the directories a walk of the
  project leaves out, and it is judged where it really is: a link does not
  lead out of the project, or into a place a plain path could not go.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
      }
      """
    And the studio tools are connected

  Scenario Outline: A file that is not a document is neither read nor written
    Given the file "<path>":
      """
      secret
      """
    When the agent calls the tool "document_text" with:
      """json
      {"doc": "<path>"}
      """
    Then the agent is refused with "path_denied"
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "<path>", "generation": 1, "text": "changed"}
      """
    Then the agent is refused with "path_denied"
    And the file "<path>" is:
      """
      secret
      """

    Examples:
      | path                  |
      | .git/config           |
      | .env                  |
      | README.md             |
      | .git/Hook.qml         |
      | node_modules/a/B.qml  |
      | target/Gen.qml        |
      | build/Gen.qml         |
      | parts/.cache/Gen.qml  |

  Scenario Outline: A document is not made where a document cannot be
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "<path>"}
      """
    Then the agent is refused with "path_denied"
    And the file "<path>" does not exist

    Examples:
      | path             |
      | notes.txt        |
      | .git/Hook.qml    |
      | build/Gen.qml    |

  Scenario: A link to a directory outside the project is not followed
    Given the link "assets" leads to a directory outside the project holding "Theirs.qml"
    When the agent calls the tool "document_text" with:
      """json
      {"doc": "assets/Theirs.qml"}
      """
    Then the agent is refused with "path_denied"
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "assets/New.qml"}
      """
    Then the agent is refused with "path_denied"
    And nothing was written outside the project

  Scenario: A link to a file outside the project is not followed
    Given the link "Out.qml" leads to the file "Theirs.qml" outside the project
    When the agent calls the tool "document_text" with:
      """json
      {"doc": "Out.qml"}
      """
    Then the agent is refused with "path_denied"
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Out.qml", "generation": 1, "text": "changed"}
      """
    Then the agent is refused with "path_denied"
    And nothing was written outside the project

  Scenario: A link does not lead into a directory a plain path cannot go
    Given the file ".git/Hook.qml":
      """qml
      Item {}
      """
    And the link "parts" leads to ".git"
    When the agent calls the tool "document_text" with:
      """json
      {"doc": "parts/Hook.qml"}
      """
    Then the agent is refused with "path_denied"

  Scenario: A link inside the project to a document reaches that document
    Given the file "parts/Card.qml":
      """qml
      Item {}
      """
    And the link "cards" leads to "parts"
    When the agent calls the tool "document_text" with:
      """json
      {"doc": "cards/Card.qml"}
      """
    Then the answer's "/generation" is 1

  Scenario: A link and the document it leads to are one document
    Given the file "parts/Card.qml":
      """qml
      Item {
      }
      """
    And the link "Link.qml" leads to "parts/Card.qml"
    And the agent has called the tool "document_text" with:
      """json
      {"doc": "parts/Card.qml"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Link.qml", "generation": 1, "node": {"path": "root"}, "name": "width", "value": 10}
      """
    Then the answer's "/doc" is "parts/Card.qml"
    And the answer's "/generation" is 2
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "parts/Card.qml", "generation": 2, "node": {"path": "root"}, "name": "height", "value": 20}
      """
    Then the answer's "/generation" is 3
    When the agent calls the tool "history" with:
      """json
      {"action": "list"}
      """
    Then the answer's "/done" has 2 entries
    And the answer's "/done/0/doc" is "parts/Card.qml"
    And the answer's "/done/1/label" is "set height"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "parts/Card.qml" is:
      """qml
      Item {
      }
      """
