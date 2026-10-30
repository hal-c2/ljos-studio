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
