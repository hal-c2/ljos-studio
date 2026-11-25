# Sources:
#   features/project/files.feature (`project_open` `files`, the same calls over MCP)
#   docs/DESIGN.md (goal 1: the file is the document)

@ui
Feature: A person edits the project's files in Studio's window
  The Files tab lists everything in the folder a walk keeps, documents and
  the rest. A document opens in the designer; any other text file opens in
  the file editor in the canvas's place, where an edit is held until it is
  saved, so the file changes on disk when the person says. Edits held are
  never dropped unasked: closing the file, opening another folder and
  closing the window each ask first. Another folder anywhere on the machine
  opens through the system's own folder dialog.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And the file "README.md":
      """text
      # Hello
      """
    And the file "src/logic.ts":
      """text
      export const answer = 42
      """

  Scenario: The Files tab lists the folder, folders first
    Given Studio's window shows the project
    Then the files panel lists "src, src/logic.ts, Main.qml, README.md"

  Scenario: A folder of any name is a folder in the Files tab
    Given the file "constructor/note.txt":
      """text
      a note
      """
    And Studio's window shows the project
    Then the files panel lists "constructor, constructor/note.txt, src, src/logic.ts, Main.qml, README.md"

  Scenario: A file is edited and saved when the person says
    Given Studio's window shows the project
    When the person opens "README.md" in the files panel
    Then the file editor holds "# Hello\n"
    When the person types "# Hello again" in the file editor
    Then the file editor says "Not saved"
    And the file "README.md" says "# Hello"
    And the file "README.md" does not say "again"
    When the person presses "Ctrl+S"
    Then the file "README.md" says "# Hello again"
    And the file editor says "Saved"

  Scenario: A document opens in the designer, not the file editor
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    When the person opens "Main.qml" in the files panel
    Then the window shows the document "Main.qml"
    And the file editor shows no file

  Scenario: A file with no edits held follows another program's save
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    When the file "README.md" is written outside Studio:
      """text
      # Saved elsewhere
      """
    Then the file editor holds "# Saved elsewhere\n" within 5 seconds

  Scenario: A dotfile with no edits held follows another program's save too
    Given the file ".env":
      """text
      A=1
      """
    And Studio's window shows the project
    And the person has opened ".env" in the files panel
    When the file ".env" is written outside Studio:
      """text
      A=2
      """
    Then the file editor holds "A=2\n" within 5 seconds

  Scenario: Edits held over another program's save are kept when the person says
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "mine" in the file editor
    When the file "README.md" is written outside Studio:
      """text
      theirs
      """
    Then the file editor says "Changed on disk" within 5 seconds
    When the person clicks the button "Keep mine"
    Then the file "README.md" says "mine"
    And the file editor says "Saved"

  Scenario: Edits typed back to the text read take what another program saved
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "mine" in the file editor
    When the file "README.md" is written outside Studio:
      """text
      theirs
      """
    Then the file editor says "Changed on disk" within 5 seconds
    When the person types "# Hello\n" in the file editor
    Then the file editor holds "theirs\n" within 5 seconds

  Scenario: Closing a file with edits held asks first
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "gone" in the file editor
    When the person clicks the button "Close file"
    Then the window asks "Save the changes to README.md?"
    When the person clicks the button "Don't save"
    Then the file editor shows no file
    And the file "README.md" says "# Hello"

  Scenario: A new file opens in the file editor
    Given Studio's window shows the project
    When the person enters "styles/site.css" as the new file's path
    And the person clicks the button "New file"
    Then the file "styles/site.css" exists
    And the file editor says "Saved"

  Scenario: Deleting a file asks first
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    When the person clicks the button "Delete file"
    Then the window asks "Delete README.md?"
    When the person clicks the button "Delete"
    Then the file "README.md" does not exist
    And the file editor shows no file

  Scenario: Another folder opens through the system's folder dialog
    Given the folder "elsewhere" beside the project holds the file "Page.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    When the person opens the "Files tab"
    And the person clicks the button "Open folder"
    And the person picks the folder "elsewhere" in the folder dialog
    Then the window shows the folder "elsewhere"
    And the window shows the document "Page.qml"
    And the files panel lists "Page.qml"

  Scenario: Another folder does not open over edits held until they are saved
    Given the folder "elsewhere" beside the project holds the file "Page.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "kept" in the file editor
    When the person clicks the button "Open folder"
    And the person picks the folder "elsewhere" in the folder dialog
    Then the window asks "Save the changes to README.md?"
    When the person clicks the button "Save all"
    Then the file "README.md" says "kept"
    And the window shows the folder "elsewhere"

  Scenario: A folder picked and then backed out of is not the person's choice
    Given the folder "elsewhere" beside the project holds the file "Page.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "kept" in the file editor
    When the person clicks the button "Open folder"
    And the person picks the folder "elsewhere" in the folder dialog
    Then the window asks "Save the changes to README.md?"
    When the person clicks the button "Cancel"
    Then an agent with fs that opens the folder "elsewhere" is refused "not_chosen"
    And the file editor shows a file

  Scenario: The window does not close over edits held until the person says
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "unsaved" in the file editor
    When the person closes the window
    Then the window asks "Save the changes to README.md?"
    And the window is open
    When the person clicks the button "Cancel"
    Then the window asks nothing
    And the window is open
    When the person closes the window
    And the person clicks the button "Don't save"
    Then the window is closed
    And the file "README.md" says "# Hello"

  Scenario: An edit not answered yet stops the window closing once another file is shown
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And an edit of "README.md" typed in the file editor is not answered yet
    When the person opens "Main.qml" in the files panel
    And the person closes the window
    Then the window asks "Save the changes to README.md?"
    And the window is open

  Scenario: A file named as a property every object has is saved like any other
    Given the file "constructor":
      """text
      plain
      """
    And Studio's window shows the project
    And the person has opened "constructor" in the files panel
    And the person has typed "changed" in the file editor
    When the person presses "Ctrl+S"
    Then the file "constructor" says "changed"
    When the person closes the window
    Then the window is closed

  Scenario: Edits held of a file another program deleted still stop the window closing
    Given Studio's window shows the project
    And the person has opened "README.md" in the files panel
    And the person has typed "unsaved" in the file editor
    When the file "README.md" is deleted outside Studio
    Then the file editor says "Changed on disk" within 5 seconds
    And the files panel lists "src, src/logic.ts, Main.qml, README.md"
    When the person closes the window
    Then the window asks "Save the changes to README.md?"
    And the window is open
