# Sources:
#   studio7 dogfood backlog P7-21 (scrolling content needs hidden knowledge:
#   nothing asks a Flickable for its contentHeight, a picture has no section,
#   the root is a bare Item with no fill, a text's font only as property
#   chips)
#   docs/MCP.md (`binding_set`, node_arrange `relayout`)
#   features/edit/relayout.feature (an Item turned into a Rectangle by an agent)

@ui
Feature: The inspector shows what a kind of object is about
  A text's size, weight and wrapping, how a picture fits and the size it is
  decoded at, how much a scrolling area holds: each sits in a section named
  for the kind of object, whether the file says it yet or not, so nobody has
  to know the names to find them. A text takes one of the theme's styles, a
  scrolling area's content follows what it holds with one press, and a plain
  box can be given a fill.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import Ljos.Design

      Item {
          id: root
          width: 400; height: 300
          Text { id: title; text: "Hello"; wrapMode: Text.NoWrap }
          Image { id: hero; width: 100; height: 60 }
          Flickable {
              id: scroller
              y: 100
              width: 200; height: 100
              Column { id: content; Rectangle { width: 10; height: 400 } }
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: A text's size is there to write before the file says it
    When the person selects "Text title" in the layers panel
    Then the inspector offers the field "font.pixelSize"
    When the person sets "font.pixelSize" to "20" in the inspector
    Then the file "Main.qml" says "font.pixelSize: 20"

  Scenario: A text takes one of the theme's styles, its size and weight together
    When the person selects "Text title" in the layers panel
    And the person chooses "Title" in the inspector's "Text style"
    Then the file "Main.qml" says "font.pixelSize: Typography.titleSize"
    And the file "Main.qml" says "font.weight: Typography.titleWeight"

  Scenario: A text is aligned with a press, and a second press lets it go
    When the person selects "Text title" in the layers panel
    And the person clicks the button "Centre the text"
    Then the file "Main.qml" says "horizontalAlignment: Text.AlignHCenter"
    When the person clicks the button "Centre the text"
    Then the file "Main.qml" does not say "horizontalAlignment"

  Scenario: A text's wrapping is in its section, with its values to pick
    When the person selects "Text title" in the layers panel
    And the person clicks the button "Choices for wrapMode"
    And the person clicks the button "Choose Text.WordWrap for wrapMode"
    Then the file "Main.qml" says "wrapMode: Text.WordWrap"

  Scenario: How a picture fits is picked from a list
    When the person selects "Image hero" in the layers panel
    And the person clicks the button "Choices for fillMode"
    And the person clicks the button "Choose Image.PreserveAspectCrop for fillMode"
    Then the file "Main.qml" says "fillMode: Image.PreserveAspectCrop"

  Scenario: The size a picture is decoded at is written in its section
    When the person selects "Image hero" in the layers panel
    And the person sets "sourceSize.width" to "200" in the inspector
    Then the file "Main.qml" says "sourceSize.width: 200"

  Scenario: A scrolling area holds as much as its content, with one press
    When the person selects "Flickable scroller" in the layers panel
    And the person clicks the button "Fit the height to the content"
    Then the file "Main.qml" says "contentHeight: content.implicitHeight"

  Scenario: A plain box is given a fill
    When the person selects "Item root" in the layers panel
    And the person clicks the button "Give it a fill"
    Then the file "Main.qml" says "Rectangle {"
    And the file "Main.qml" says "color: Colors.canvas"
