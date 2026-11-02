# Sources:
#   docs/DESIGN.md (S4: states as variants, breakpoints, slots)
#   features/edit/states.feature, features/edit/breakpoints.feature,
#   features/edit/slots.feature (the same edits over MCP; the file text is
#   the oracle for both)

@ui
Feature: A person edits variants and slots in Studio's window
  The inspector lists the selected object's states. A person adds,
  renames and removes them, picks one to edit the values it changes and
  the condition that turns it on, or gives it a breakpoint. The canvas
  says which breakpoint's state each frame shows. The palette lists the
  slots each block offers, and contributes to one of the selected block.
  Each gesture is the `state_edit` or `slot_contribute` call an agent
  makes.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      """
    And the file "App/Media/Player.qml":
      """qml
      import QtQuick
      import Ljos.Blocks

      Item {
          property list<Contribution> controls
      }
      """
    And the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Text {
              id: label
              text: "wide"
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Item root" in the layers panel

  Scenario: A state is added and listed
    When the person adds the state "compact" in the inspector
    Then the inspector lists the states "compact"
    And the history lists "add state compact"

  Scenario: A breakpoint is chosen for a state
    Given the person has added the state "compact" in the inspector
    When the person picks the state "compact" in the inspector
    And the person clicks the button "Breakpoint phone"
    Then the inspector shows "When" as "root.width < 600"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          states: [
              State {
                  name: "compact"
                  when: root.width < 600
              }
          ]

          Text {
              id: label
              text: "wide"
          }
      }
      """

  Scenario: A value the picked state changes
    Given the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    When the person changes "text" of "label" to "\"narrow\"" in the state
    Then the inspector shows "label.text" as "\"narrow\""
    When the person sets "label.text" to "\"small\"" in the inspector
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          states: [
              State {
                  name: "compact"
                  PropertyChanges {
                      target: label
                      text: "small"
                  }
              }
          ]

          Text {
              id: label
              text: "wide"
          }
      }
      """

  Scenario: A state is renamed and removed
    Given the person has added the state "compact" in the inspector
    When the person renames the state "compact" to "narrow" in the inspector
    Then the inspector lists the states "narrow"
    When the person clicks the button "Remove state narrow"
    Then the inspector lists the states ""
    And the history lists "remove state narrow"

  @preview
  Scenario: A frame says which breakpoint's state it shows
    Given the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the button "Breakpoint phone"
    When the person chooses the target "Phone"
    Then the frame "Phone" shows the state "compact"
    When the person chooses the target "Desktop"
    Then the frame "Desktop" shows no state

  @preview
  Scenario: A frame's button starts that frame's layout
    When the person chooses the target "Phone"
    And the person clicks the button "Phone layout"
    Then the file "Main.qml" says "when: root.width < 600"
    And the frame "Phone" shows the state "phone"
    And the window says "Editing state phone"
    When the person selects "Text label" in the layers panel
    And the person sets "text" to "\"narrow\"" in the inspector
    Then the file "Main.qml" says "target: label"
    And the file "Main.qml" says "text: \"wide\""
    And the file "Main.qml" says "text: \"narrow\""
    When the person clicks the button "Done editing state"
    Then the window does not say "Editing state phone"
    When the person clicks the button "Edit phone layout"
    Then the window says "Editing state phone"

  @preview
  # A hand-drawn document's root has no id, and a breakpoint compares the
  # root's width by its id: the button gives it one rather than a refusal.
  Scenario: A frame's button names a root that has no name
    Given an agent has called the tool "document_new" with:
      """json
      {"doc": "Bare.qml", "text": "import QtQuick\n\nItem {\n    Text {\n        id: label\n        text: \"wide\"\n    }\n}\n"}
      """
    And the person has opened "Bare.qml" in the documents list
    When the person chooses the target "Tablet"
    And the person clicks the button "Tablet layout"
    Then the file "Bare.qml" says "id: root"
    And the file "Bare.qml" says "when: root.width >= 600 && root.width < 1024"
    And the frame "Tablet" shows the state "tablet"

  @preview
  Scenario: A state of that name takes the frame's breakpoint
    Given the person has added the state "phone" in the inspector
    When the person chooses the target "Phone"
    And the person clicks the button "Phone layout"
    Then the inspector lists the states "phone"
    And the file "Main.qml" says "when: root.width < 600"
    And the frame "Phone" shows the state "phone"

  # A `when` written into a running app is shown without a new generation,
  # and the outline was read only at a new generation.
  @preview
  Scenario: The outline and the picture follow what a frame's state does
    Given the person has selected "Text label" in the layers panel
    And the person has looked at the frame "Desktop"
    And the person has clicked the button "Desktop layout"
    When the person sets "text" to "\"narrow\"" in the inspector
    Then the frame "Desktop" has drawn a different picture since
    When the person sets "width" to "100" in the inspector
    And the person sets "height" to "20" in the inspector
    And the person sets "x" to "40" in the inspector
    Then the canvas outlines the selection at 40, 0 sized 100 by 20
    When the person clicks the button "Done editing state"
    Then the canvas outlines the selection at 40, 0 sized 100 by 20

  @preview
  Scenario: The outline follows the frame's width to another state
    Given the person has selected "Text label" in the layers panel
    And the person has set "width" to "100" in the inspector
    And the person has set "height" to "20" in the inspector
    And the person has chosen the target "Phone"
    And the person has clicked the button "Phone layout"
    And the person has set "x" to "40" in the inspector
    And the person has clicked the button "Done editing state"
    When the person chooses the target "Desktop"
    Then the canvas outlines the selection at 0, 0 sized 100 by 20
    When the person chooses the target "Phone"
    Then the frame "Phone" outlines the selection at 40, 0 sized 100 by 20

  Scenario: The palette lists a block's slots
    Then the palette lists the slots "controls" of "Player"

  Scenario: A contribution to the selected block's slot
    Given an agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "block": "App.Media.Player"}
      """
    And the person has selected "Player player1" in the layers panel
    When the person contributes "Text { text: \"Shuffle\" }" to the slot "controls" from the palette
    Then the layers panel lists "Item root, Text label, Player player1, Contribution contribution1, Component, Text"
    And the history lists "contribute to controls"
