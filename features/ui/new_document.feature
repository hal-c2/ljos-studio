# Sources:
#   docs/DOGFOOD.md (D3: a person could not start a document in the window)
#   features/edit/compose-shell.feature (`document_new`, the same tool over MCP)

@ui
Feature: A person starts a document in Studio's window
  The documents panel makes a document through `document_new`, the tool an
  agent calls. A directory with nothing in it opens with a first document,
  because a window with nothing to edit has nothing to offer; a directory
  that has other files keeps them to itself and only offers the button.

  Scenario: The panel makes a document and opens it
    Given the file "Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    When the person enters "Page" as the new document's name
    And the person clicks the button "New document"
    Then the file "Page.qml" says "import QtQuick"
    And the history lists "create"
    And the window shows the document "Page.qml"

  Scenario: A name already taken is refused where the person typed it
    Given the file "Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    When the person enters "Main.qml" as the new document's name
    And the person clicks the button "New document"
    Then the documents panel says why it made no document
    And the file "Main.qml" says "Item {}"

  Scenario: An empty directory opens with a first document
    Given Studio's window shows the project
    Then the file "Main.qml" says "import QtQuick"
    And the window shows the document "Main.qml"

  Scenario: A directory with other files is left as it is
    Given the file "notes.txt":
      """
      nothing to do with QML
      """
    And Studio's window shows the project
    Then the file "Main.qml" does not exist

  Scenario: An agent's document leaves the window where it is
    Given the file "Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    When an agent calls the tool "document_new" with:
      """json
      {"doc": "sub/Other.qml"}
      """
    Then the file "sub/Other.qml" says "import QtQuick"
    And the file "Main.qml" says "Item {}"
