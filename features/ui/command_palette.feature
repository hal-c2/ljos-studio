# Sources:
#   docs/DESIGN.md (every gesture is a tool call an agent could make)
#   features/ui/canvas_view.feature (the zoom the palette runs)
#   features/ui/design.feature (undo from the window)

@ui @preview
Feature: The command palette finds and runs any of Studio's actions
  Ctrl+K, ⌘K or Ctrl+Shift+P opens the command palette. What is typed finds
  actions by their letters in order, best first, and those used lately
  come first. Each row names its shortcut. Enter runs the row picked and
  closes the palette; Escape closes it and gives the keys back. Studio's
  actions are its MCP tools, so every tool is there by name, beside the
  window's own: zooms, the targets, the documents, the types to insert,
  the look and the panels.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: Ctrl+K opens the command palette
    When the person presses "Ctrl+K"
    Then the window has a pane "Command palette"

  Scenario: Ctrl+Shift+P opens it too, and Escape closes it
    Given the person has pressed "Ctrl+Shift+P"
    When the person presses "Escape"
    Then the window has no pane "Command palette"

  Scenario: What is typed finds an action by its letters in order
    Given the person has pressed "Ctrl+K"
    When the person types "zm100" into the command palette
    Then the command palette's first row is "Zoom to 100%"
    And the command palette's row "Zoom to 100%" names the shortcut "Ctrl+1"

  Scenario: Enter runs the action found and closes the palette
    Given the person has pressed "Ctrl+K"
    And the person has typed "zoom to 100" into the command palette
    When the person presses "Return"
    And the canvas has finished moving
    Then the zoom reads "100%"
    And the window has no pane "Command palette"

  Scenario: An action is a tool call an agent could make
    Given the person has selected "Rectangle root" in the layers panel
    And the person has set "width" to "401" in the inspector
    And the person has pressed "Ctrl+K"
    And the person has typed "undo" into the command palette
    When the person presses "Return"
    Then the file "Main.qml" says "width: 400"

  Scenario: Every MCP tool is in the palette by name
    Given the person has pressed "Ctrl+K"
    When the person types "node_arrange" into the command palette
    Then the command palette lists "node_arrange"

  Scenario: A type is inserted from the palette
    Given the person has pressed "Ctrl+K"
    And the person has typed "insert rectangle" into the command palette
    When the person presses "Return"
    Then the file "Main.qml" says "width: 100"
    And the history lists "insert Rectangle"

  Scenario: What was used lately comes first
    Given the person has pressed "Ctrl+K"
    And the person has typed "zoom to 100" into the command palette
    And the person has pressed "Return"
    When the person presses "Ctrl+K"
    Then the command palette's first row is "Zoom to 100%"
