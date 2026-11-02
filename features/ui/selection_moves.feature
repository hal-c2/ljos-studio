# Sources:
#   docs/MCP.md (node_select: an agent moves the selection the window shows)
#   features/ui/picker_object.feature (the colour picker does this its own way)

@ui
Feature: What is begun on one object does not go to the next
  The selection is shared, so it can move while a person is typing or has a
  list open. A write reads the selection when it is sent: what was begun for
  the object selected before would land on the one selected now. So what is
  begun goes when the selection moves.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Column { id: stackA }
          Column { id: stackB }
          MouseArea { id: areaA }
          MouseArea { id: areaB }
          Text { id: textA }
          Text { id: textB }
          Repeater {
              model: ListModel { ListElement { name: "Ada" } }
              Column {
                  Text { id: rowA }
                  Text { id: rowB }
              }
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario Outline: What was typed for one object is not left for the next: <field>
    Given the person has selected "<first>" in the layers panel
    When the person types "typed" in the inspector's "<field>" and leaves it
    And an agent selects "<next>"
    Then the inspector's "<field>" holds ""

    Examples:
      | field           | first         | next   |
      | New property    | Text textA    | textB  |
      | New value       | Text textA    | textB  |
      | Component name  | Text textA    | textB  |
      | New comment     | Text textA    | textB  |
      | Handle clicked  | MouseArea areaA | areaB |
      | Layout spacing  | Column stackA | stackB |
      | Bind role name to | Text rowA   | rowB   |

  Scenario: A list of choices opened on one object closes when the selection moves
    Given the person has selected "Text textA" in the layers panel
    And the person has typed "horizontalAlignment" in the inspector's "New property" and left it
    And the person has clicked the button "Choices for new property"
    When an agent selects "textB"
    Then the window does not say "Choose Text.AlignLeft for horizontalAlignment"
