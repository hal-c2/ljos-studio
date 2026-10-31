# Sources:
#   docs/DOGFOOD.md (D7: a picked state did not take the inspector's values)
#   features/edit/states.feature (`state_edit` with `change`, the same edit over MCP)
#   features/preview/flows.feature (`preview_state` with `show`)

@ui
Feature: A person records values into a state in the inspector
  While a state is picked, what the inspector sets goes into that state as
  a change of the selected object, as `state_edit` writes it, and leaves
  the object's own value alone. The picked state stays through a change of
  selection until the person says they are done, and the window says it is
  being edited. A state with no condition is put on in the frame; one with a
  condition is the app's to turn on, so its frame shows it when that holds.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Text {
              id: label
              text: "wide"
          }
          Rectangle {
              width: 40
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Item root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector

  Scenario: The inspector's values go into the picked state
    When the person selects "Text label" in the layers panel
    And the person sets "text" to "\"narrow\"" in the inspector
    Then the file "Main.qml" says "target: label"
    And the file "Main.qml" says "wide"
    And the file "Main.qml" says "narrow"
    And the inspector shows "text" as "\"narrow\""
    And the window says "Editing state compact"

  Scenario: An object without an id cannot be recorded into a state
    When the person selects "Rectangle" in the layers panel
    And the person sets "width" to "10" in the inspector
    Then the window says "A state names the object it changes: give this one an id"
    And the file "Main.qml" says "width: 40"
    And the file "Main.qml" does not say "PropertyChanges"

  Scenario: Done ends the recording and values go to the object again
    When the person clicks the button "Done editing state"
    And the person selects "Text label" in the layers panel
    And the person sets "text" to "\"plain\"" in the inspector
    Then the file "Main.qml" says "plain"
    And the file "Main.qml" does not say "PropertyChanges"
