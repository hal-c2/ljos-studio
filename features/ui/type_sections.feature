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

  # A style is two writes, and the first may be refused while the second
  # goes through: what is refused is still said in the text's section, and
  # the second's answer does not take it off the window's line.
  Scenario: A style the file refuses in part says so in the text's section
    When the person replaces "wrapMode: Text.NoWrap" with "wrapMode: Text.NoWrap; font.weight: Font.Normal; font.pixelSize: Item { id: big }" in the code view
    And the person selects "Text title" in the layers panel
    And an agent named "Ada" calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "big"}], "hold": true}
      """
    And the person selects "Text title" in the layers panel
    And the person chooses "Title" in the inspector's "Text style"
    Then the file "Main.qml" says "font.weight: Typography.titleWeight"
    And the file "Main.qml" does not say "Typography.titleSize"
    And the inspector says something with "held by Ada"
    And the window shows a refusal that says "held by Ada"

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
    Then the file "Main.qml" says "contentHeight: content.height"

  # What it holds may be sized only by what it is given: a box's implicit
  # height is nothing.
  Scenario: A scrolling area holds as much as a sized box it holds
    When the person replaces "Column { id: content; Rectangle { width: 10; height: 400 } }" with "Rectangle { id: card; width: 320; height: 640 }" in the code view
    And the person selects "Flickable scroller" in the layers panel
    And the person clicks the button "Fit the height to the content"
    Then the file "Main.qml" says "contentHeight: card.height"

  Scenario: A plain box is given a fill
    When the person selects "Item root" in the layers panel
    And the person clicks the button "Give it a fill"
    Then the file "Main.qml" says "Rectangle {\n    id: root"
    And the file "Main.qml" says "color: Colors.canvas"

  # The theme's names are written the way the file imports them.
  Scenario: A style is written with the name the theme is imported as
    When the person replaces "import Ljos.Design" with "import Ljos.Design as Design" in the code view
    And the person selects "Text title" in the layers panel
    And the person chooses "Title" in the inspector's "Text style"
    Then the file "Main.qml" says "font.pixelSize: Design.Typography.titleSize"
    And the file "Main.qml" says "font.weight: Design.Typography.titleWeight"

  Scenario: A fill is written with the name the theme is imported as
    When the person replaces "import Ljos.Design" with "import Ljos.Design as Design" in the code view
    And the person selects "Item root" in the layers panel
    And the person clicks the button "Give it a fill"
    Then the file "Main.qml" says "color: Design.Colors.canvas"
