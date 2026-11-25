# Sources:
#   docs/DESIGN.md (goal 1: the file is the document)
#   docs/MCP.md (`project_open` `files`; `fs` writes anything else)

Feature: An agent edits the project's other files
  A project is more than its documents: a README, a TypeScript module a
  document imports, a style sheet, a `package.json`. `project_open` `files`
  lists them and opens, edits, saves, makes, renames and removes them. A
  document is saved as it is edited, with its history; any other file is
  held as it is edited and written only when saved, so a save is the one
  moment it changes on disk. A file that changed on disk since it was read
  is not written over without being asked to, and one with no edits held
  just takes the new text. Only reading the list is `inspect`; the rest
  reads or writes files that are not documents, which is `fs`.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
      }
      """
    And the file "README.md":
      """text
      # Hello
      """
    And the file "src/logic.ts":
      """text
      export const answer: number = 42
      """
    And the file ".git/config":
      """text
      secret
      """
    And the studio tools are connected with the capabilities "inspect,mutate,fs"

  Scenario: The project's files, without those a walk leaves out
    When the agent calls the tool "project_open" with:
      """json
      {"files": "list"}
      """
    Then the answer's "/files" lists "Main.qml, README.md, src/logic.ts"

  Scenario: A reader lists the files but reads no file that is not a document
    Given the studio tools are connected with the capabilities "inspect"
    When the agent calls the tool "project_open" with:
      """json
      {"files": "list"}
      """
    Then the answer's "/files" lists "Main.qml, README.md, src/logic.ts"
    When the agent calls the tool "project_open" with:
      """json
      {"files": "read", "file": "README.md"}
      """
    Then the agent is refused with "capability_denied"

  Scenario: A file is read with what it holds
    When the agent calls the tool "project_open" with:
      """json
      {"files": "read", "file": "src/logic.ts"}
      """
    Then the answer's "/file/path" is "src/logic.ts"
    And the answer's "/file/text" is "export const answer: number = 42\n"
    And the answer's "/file/dirty" is false

  Scenario: An edit is held until it is saved
    When the agent calls the tool "project_open" with:
      """json
      {"files": "edit", "file": "README.md", "text": "# Hello again\n"}
      """
    Then the answer's "/file/dirty" is true
    And the answer's "/unsaved" lists "README.md"
    And the file "README.md" is:
      """text
      # Hello
      """
    When the agent calls the tool "project_open" with:
      """json
      {"files": "save", "file": "README.md"}
      """
    Then the answer's "/file/dirty" is false
    And the answer's "/unsaved" is absent
    And the file "README.md" is:
      """text
      # Hello again
      """

  Scenario: Every held edit is saved at once
    Given the agent has called the tool "project_open" with:
      """json
      {"files": "edit", "file": "README.md", "text": "one\n"}
      """
    And the agent has called the tool "project_open" with:
      """json
      {"files": "edit", "file": "src/logic.ts", "text": "two\n"}
      """
    When the agent calls the tool "project_open" with:
      """json
      {"files": "save_all"}
      """
    Then the answer's "/saved" lists "README.md, src/logic.ts"
    And the file "README.md" contains "one"
    And the file "src/logic.ts" contains "two"

  Scenario: An edit taken back is the text on disk again
    Given the agent has called the tool "project_open" with:
      """json
      {"files": "edit", "file": "README.md", "text": "gone\n"}
      """
    When the agent calls the tool "project_open" with:
      """json
      {"files": "revert", "file": "README.md"}
      """
    Then the answer's "/file/text" is "# Hello\n"
    And the answer's "/file/dirty" is false

  Scenario: A file with no edits held takes what another program saved
    Given the agent has called the tool "project_open" with:
      """json
      {"files": "read", "file": "README.md"}
      """
    When the file "README.md" is rewritten outside Studio:
      """text
      # Saved elsewhere
      """
    And the agent calls the tool "project_open" with:
      """json
      {"files": "read", "file": "README.md"}
      """
    Then the answer's "/file/text" is "# Saved elsewhere\n"
    And the answer's "/file/outside" is false

  Scenario: A save over a change made outside is refused until it is forced
    Given the agent has called the tool "project_open" with:
      """json
      {"files": "edit", "file": "README.md", "text": "mine\n"}
      """
    When the file "README.md" is rewritten outside Studio:
      """text
      theirs
      """
    And the agent calls the tool "project_open" with:
      """json
      {"files": "read", "file": "README.md"}
      """
    Then the answer's "/file/text" is "mine\n"
    And the answer's "/file/outside" is true
    When the agent calls the tool "project_open" with:
      """json
      {"files": "save", "file": "README.md"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "changed_outside"
    And the file "README.md" contains "theirs"
    When the agent calls the tool "project_open" with:
      """json
      {"files": "save", "file": "README.md", "force": true}
      """
    Then the file "README.md" contains "mine"

  Scenario: A new file, renamed, then removed
    When the agent calls the tool "project_open" with:
      """json
      {"files": "new", "file": "styles/site.css", "text": "body {}\n"}
      """
    Then the file "styles/site.css" contains "body {}"
    When the agent calls the tool "project_open" with:
      """json
      {"files": "rename", "file": "styles/site.css", "to": "styles/main.css"}
      """
    Then the file "styles/main.css" contains "body {}"
    And the file "styles/site.css" does not exist
    When the agent calls the tool "project_open" with:
      """json
      {"files": "remove", "file": "styles/main.css"}
      """
    Then the file "styles/main.css" does not exist

  Scenario: A file that exists is not made again, nor renamed over
    When the agent calls the tool "project_open" with:
      """json
      {"files": "new", "file": "README.md"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "exists"
    When the agent calls the tool "project_open" with:
      """json
      {"files": "rename", "file": "src/logic.ts", "to": "README.md"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "exists"
    And the file "README.md" contains "# Hello"

  Scenario: A document is edited as a document
    When the agent calls the tool "project_open" with:
      """json
      {"files": "edit", "file": "Main.qml", "text": "Item {}\n"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "document"

  Scenario Outline: A file a walk leaves out is not reached
    When the agent calls the tool "project_open" with:
      """json
      {"files": "<action>", "file": ".git/config", "text": "x"}
      """
    Then the agent is refused with "path_denied"
    And the file ".git/config" contains "secret"

    Examples:
      | action |
      | read   |
      | edit   |
      | new    |
      | remove |

  Scenario: A file that is not text is not opened as text
    Given the file "logo.png" holds bytes that are not UTF-8
    When the agent calls the tool "project_open" with:
      """json
      {"files": "read", "file": "logo.png"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "not_text"

  Scenario: Another project is not opened over unsaved edits
    Given the file "site/Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And the agent has called the tool "project_open" with:
      """json
      {"files": "edit", "file": "README.md", "text": "unsaved\n"}
      """
    When the agent calls the tool "project_open" with:
      """json
      {"path": "site"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "unsaved"
    And the refusal's "/details/files/0" is "README.md"
    When the agent calls the tool "project_open" with:
      """json
      {"path": "site", "force": true}
      """
    Then the answer's "/project" is "site"
    And the file "README.md" contains "# Hello"

  @disk
  Scenario: A folder anywhere opens as the workspace
    Given the folder "elsewhere" beside the workspace holds the file "Page.qml":
      """qml
      import QtQuick
      Item {}
      """
    When the agent opens the folder "elsewhere" beside the workspace
    Then the answer's "/documents" lists "Page.qml"
    And the answer's "/project" is "."
    And the answer's "/recent/0" names the folder "elsewhere" beside the workspace

  @disk
  Scenario: Only who may write files opens a folder outside the workspace
    Given the folder "elsewhere" beside the workspace holds the file "Page.qml":
      """qml
      import QtQuick
      Item {}
      """
    And the studio tools are connected with the capabilities "inspect,mutate"
    When the agent opens the folder "elsewhere" beside the workspace
    Then the agent is refused with "capability_denied"
