# Sources:
#   docs/DOGFOOD.md (D15: the inspector shows colours as text and hides declared properties)

@ui
Feature: The inspector shows colours and every declared property
  A colour has a swatch, and the swatch opens a picker; what the picker sets
  is `binding_set`, as the row's own field's is. A property declared without a
  value has a row too, so it can be given one.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          property var stack
          property int count: 1
          property color tint: "#80ff0000"
          property color accentColor: Qt.rgba(0, 1, 0, 1)
          color: "#ff0000"
          width: 100
          Item { id: other }
      }
      """
    And the file "Other.qml":
      """qml
      import QtQuick

      Item { }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle root" in the layers panel

  Scenario: A property declared without a value has a row
    When the person sets "stack" to "3" in the inspector
    Then the file "Main.qml" says "stack: 3"

  Scenario: A row with nothing written still refuses an expression typed by mistake
    When the person sets "stack" to "cube" in the inspector
    Then the window says "cube is not a literal: bind it as an expression"
    And the file "Main.qml" does not say "cube"

  Scenario: A colour has a swatch and others do not
    Then the inspector has the button "Pick color"
    And the inspector has no button "Pick width"

  Scenario: The picker sets the colour
    When the person clicks the button "Pick color"
    And the person enters "#00ff00" in the colour picker
    Then the file "Main.qml" says "#00ff00"
    And the file "Main.qml" does not say "#ff0000"

  Scenario: The picker takes a colour's name
    When the person clicks the button "Pick color"
    And the person enters "green" in the colour picker
    Then the file "Main.qml" says "green"
    And the file "Main.qml" does not say "#ff0000"

  Scenario: A declared colour given a name keeps its swatch, and the sliders go on from it
    When the person clicks the button "Pick tint"
    And the person enters "#0000ff" in the colour picker
    And the person drags the picker's "Brightness" to its middle
    Then the file "Main.qml" says "#000080"
    When the person clicks the button "Pick tint"
    And the person enters "green" in the colour picker
    Then the file "Main.qml" says "green"
    When the person clicks the button "Pick tint"
    And the person enters "blue" in the colour picker
    Then the file "Main.qml" says "blue"

  Scenario: A value of null is refused for what is declared, and names why
    When the person replaces "property var stack" with "property Component maker: Component { Item {} }" in the code view
    And the person selects "Rectangle root" in the layers panel
    And the person sets "maker" to "null" in the inspector
    Then the window shows a refusal that says "holds an object"
    When the person sets "count" to "null" in the inspector
    Then the window shows a refusal that says "is declared with this value"
    And the file "Main.qml" says "property int count: 1"

  Scenario Outline: A row written as a literal in any of QML's forms refuses an expression
    When the person replaces "width: 100" with "<name>: <written>" in the code view
    And the person sets "<name>" to "cube" in the inspector
    Then the window says "cube is not a literal: bind it as an expression"
    And the file "Main.qml" says "<name>: <written>"

    Examples:
      | name       | written |
      | objectName | 'hello' |
      | opacity    | .5      |
      | width      | 0x10    |
      | x          | -1      |

  Scenario Outline: A literal typed in any of QML's forms is written as typed
    When the person replaces "width: 100" with "<name>: <written>" in the code view
    And the person sets "<name>" to "<typed>" in the inspector
    Then the file "Main.qml" says "<name>: <typed>"
    And the window does not say "<typed> is not a literal: bind it as an expression"
    And the window shows no refusal

    Examples:
      | name       | written | typed   |
      | objectName | 'hello' | 'world' |
      | opacity    | .5      | .7      |
      | width      | 0x10    | 0x20    |

  Scenario Outline: Enter on a row's own text changes nothing
    When the person replaces "width: 100" with "<name>: <written>" in the code view
    And the person sets "<name>" to "<written>" in the inspector
    Then the file "Main.qml" says "<name>: <written>"
    And the window does not say "<written> is not a literal: bind it as an expression"
    And the window shows no refusal
    When the person clicks the button "Undo"
    Then the file "Main.qml" says "width: 100"

    Examples:
      | name       | written |
      | objectName | 'hello' |
      | opacity    | .5      |
      | width      | 0x10    |

  Scenario: A row holding an expression stays one when another is typed
    When the person replaces "width: 100" with "width: 50 + 50" in the code view
    And the person sets "width" to "60 + 60" in the inspector
    Then the file "Main.qml" says "width: 60 + 60"

  Scenario: The picker writes nothing for what is not a colour
    When the person clicks the button "Pick color"
    And the person enters "#12" in the colour picker
    And the person enters "#12345" in the colour picker
    And the person enters "gren" in the colour picker
    Then the file "Main.qml" says "#ff0000"
    And the file "Main.qml" does not say "#12"
    And the file "Main.qml" does not say "gren"

  Scenario: The picker writes nothing for the empty field of a colour it could not read
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    And the person selects "Rectangle top" in the layers panel
    And the person clicks the button "Pick accentColor"
    And the person enters "" in the colour picker
    Then the file "Main.qml" says "Qt.rgba(0, 1, 0, 1)"

  Scenario: A property worked out by a block has its row, and a value replaces the block
    When the person replaces "property var stack" with "property int worked: { return 1 }" in the code view
    And the person selects "Rectangle root" in the layers panel
    Then the inspector's "worked" holds "{ return 1 }"
    When the person sets "worked" to "3" in the inspector
    Then the file "Main.qml" says "property int worked: 3"
    And the file "Main.qml" does not say "return 1"

  Scenario: A property that holds an object has its row, and a value is refused
    When the person replaces "property var stack" with "property Component maker: Component { Item {} }" in the code view
    And the person selects "Rectangle root" in the layers panel
    Then the inspector's "maker" holds "Component { Item {} }"
    When the person sets "maker" to "3" in the inspector
    Then the window shows a refusal that says "holds an object"
    And the file "Main.qml" says "property Component maker: Component { Item {} }"
    And the file "Main.qml" does not say "maker: 3"
    And the inspector's "maker" holds "Component { Item {} }"

  @preview
  Scenario: The picker of a value the running app holds that is no colour says so
    When the person replaces "property var stack" with "property string borderColor: \"nope\"" in the code view
    And the person selects "Rectangle root" in the layers panel
    And the person clicks the button "Pick borderColor"
    Then the window says "nope is not a colour"
    And the window does not say "Worked out by "nope": picking replaces it"
    And the colour picker waits on nothing
    And the colour picker goes on from grey, with no reading in its hex

  Scenario: Text that is no colour has no picker, however it looks
    When the person replaces "property var stack" with "property string code: \"#12345\"\n    property string label: \"red\"\n    property string objectTag: \"orange\"" in the code view
    And the person replaces "color: \"#ff0000\"" with "color: \"transparent\"" in the code view
    And the person selects "Rectangle root" in the layers panel
    Then the inspector has no button "Pick code"
    And the inspector has no button "Pick label"
    And the inspector has no button "Pick objectTag"
    And the inspector has the button "Pick color"

  # Every kind of row, and what it gets. The type or a `*color` name says a
  # row is meant to be a colour; text alone says so only as a `#` value that
  # the runtime reads. The swatch paints what is written by itself, and the
  # note is for what an expression works out.
  Scenario Outline: A row offers what its kind of value calls for: <kind>
    When the person replaces "property var stack" with "<declaration>" in the code view
    And the person selects "Rectangle root" in the layers panel
    Then the inspector's "<row>" has <picker>, painted "<swatch>", with the note "<note>"

    Examples:
      | kind                                | declaration                                    | row                  | picker    | swatch      | note                                                   |
      | declared colour                     | property color shade: "#336699"                | shade                | a picker  | #336699     |                                                        |
      | declared colour, named              | property color shade: "red"                    | shade                | a picker  | red         |                                                        |
      | declared colour, empty              | property color shade                           | shade                | a picker  |             |                                                        |
      | colour by name                      | property var borderColor: "red"                | borderColor          | a picker  | red         |                                                        |
      | colour by a longer name             | property var selectedTextColor: "red"          | selectedTextColor    | a picker  | red         |                                                        |
      | colour by a longer name, expression | property var placeholderTextColor: Colors.text | placeholderTextColor | a picker  |             | Worked out by Colors.text: picking replaces it         |
      | colour by name, transparent         | property var borderColor: "transparent"        | borderColor          | a picker  | transparent |                                                        |
      | colour by name, empty               | property var borderColor                       | borderColor          | a picker  |             |                                                        |
      | colour by name, not one             | property string borderColor: "nope"            | borderColor          | a picker  |             |                                                        |
      | hex text                            | property string code: "#123456"                | code                 | a picker  | #123456     |                                                        |
      | not quite hex text                  | property string code: "#12345"                 | code                 | no picker |             |                                                        |
      | colour-name text                    | property string label: "red"                   | label                | no picker |             |                                                        |
      | not a colour                        | property string label: "nope"                  | label                | no picker |             |                                                        |
      | number                              | property int size: 3                           | size                 | no picker |             |                                                        |
      | expression                          | property int size: 1 + 1                       | size                 | no picker |             |                                                        |
      | colour by expression                | property color shade: Qt.rgba(0, 1, 0, 1)      | shade                | a picker  |             | Worked out by Qt.rgba(0, 1, 0, 1): picking replaces it |
      | colour-named expression             | property var borderColor: Colors.text          | borderColor          | a picker  |             | Worked out by Colors.text: picking replaces it         |
      | single-quoted colour                | property color shade: 'red'                    | shade                | a picker  | red         |                                                        |
      | single-quoted hex text              | property string code: '#123456'                | code                 | a picker  | #123456     |                                                        |
      | single-quoted text                  | property string label: 'red'                   | label                | no picker |             |                                                        |
      | template colour                     | property color shade: `red`                    | shade                | a picker  | red         |                                                        |
      | colour by name, a number            | property var borderColor: 3                    | borderColor          | a picker  |             |                                                        |

  @preview
  Scenario: A new picker does not keep what the last one refused
    When the person replaces "property var stack" with "property string borderColor: \"nope\"" in the code view
    And the person selects "Rectangle root" in the layers panel
    And the person clicks the button "Pick borderColor"
    Then the window says "nope is not a colour"
    When the person clicks the button "Pick tint"
    Then the window does not say "nope is not a colour"
    And the colour picker holds "#80ff0000"

  Scenario: The colour transparent is written, not worked out
    When the person replaces "color: \"#ff0000\"" with "color: \"transparent\"" in the code view
    And the person selects "Rectangle root" in the layers panel
    And the person clicks the button "Pick color"
    Then the window does not say "Worked out by "transparent": picking replaces it"
    And the colour picker holds "transparent"

  Scenario: A colour in single quotes is written, not worked out
    When the person replaces "color: \"#ff0000\"" with "color: 'red'" in the code view
    And the person selects "Rectangle root" in the layers panel
    And the person clicks the button "Pick color"
    Then the window does not say "Worked out by 'red': picking replaces it"
    And the colour picker holds "red"

  Scenario: The picker's sliders leave a colour's alpha as it was written
    When the person clicks the button "Pick tint"
    And the person drags the picker's "Brightness" to its middle
    Then the file "Main.qml" says "#80800000"
    And the file "Main.qml" does not say "#80ff0000"

  Scenario: The picker of a colour the running app cannot say gives the reason
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    And the person selects "Rectangle top" in the layers panel
    And the person clicks the button "Pick accentColor"
    Then the window shows a refusal that says "top"
    And the colour picker waits on nothing
    And the colour picker goes on from grey, with no reading in its hex

  Scenario: The picker of a colour an expression works out starts from what the preview shows
    When the person clicks the button "Pick accentColor"
    Then the colour picker holds "#00ff00"
    And the window says "Worked out by Qt.rgba(0, 1, 0, 1): picking replaces it"
    And the file "Main.qml" says "Qt.rgba(0, 1, 0, 1)"

  Scenario: A refused value can be bound as the refusal says
    When the person sets "width" to "parent.width" in the inspector
    And the person clicks the button "Bind width to parent.width"
    Then the file "Main.qml" says "width: parent.width"
    And the window shows no refusal

  Scenario: A colour is written the way a person writes one
    When the person sets "color" to "#123456" in the inspector
    Then the file "Main.qml" says "color: \"#123456\""
    And the window shows no refusal

  Scenario: A word typed over a string can be written as one
    When the person sets "color" to "red" in the inspector
    And the person clicks the button "Write red as text"
    Then the file "Main.qml" says "color: \"red\""
    And the window shows no refusal

  Scenario: A number row does not offer to write a word as text
    When the person sets "width" to "parent.width" in the inspector
    Then the inspector has no button "Write parent.width as text"

  Scenario: A refused value is forgotten when something else is selected
    Given the person has set "width" to "parent.width" in the inspector
    When the person selects "Item other" in the layers panel
    Then the inspector has no button "Bind width to parent.width"
    And the window shows no refusal


  Scenario: A refusal goes with the object it was about
    Given the person has selected "Item other" in the layers panel
    And the person has set "stack" to "cube" in the inspector
    When an agent calls the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"id": "other"}}
      """
    Then the window does not say "cube is not a literal: bind it as an expression"

  Scenario: A refused value is forgotten when another document opens
    Given the person has set "width" to "parent.width" in the inspector
    When the person opens "Other.qml" in the documents list
    Then the inspector has no button "Bind width to parent.width"
    And the window shows no refusal

  Scenario: A refused value is forgotten when a state is picked
    Given the person has set "width" to "parent.width" in the inspector
    And the person has added the state "compact" in the inspector
    And the person has set "width" to "parent.width" in the inspector
    When the person picks the state "compact" in the inspector
    Then the inspector has no button "Bind width to parent.width"
    And the window shows no refusal
