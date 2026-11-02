# Sources:
#   docs/MCP.md (node_insert, property_set, node_arrange)
#   qml6 6.11, measured: what is not a member or type here stops a file loading

Feature: A tool does not write what would not load or would not show
  A tool's answer is what an agent goes on. A file that stops loading, or an
  object that shows something else, must be a refusal, naming why, and not an
  edit that went well. A type or member Studio has no description of is not
  refused: only running it could tell.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts
      import App.Native

      Item {
          id: root
          property int count: 0
          states: State { name: "on" }
          RowLayout { id: row; spacing: 4; uniformCellSizes: true }
          GridLayout { id: grid; uniformCellWidths: true }
          Rectangle { id: card; width: 10 }
          Text { id: label }
          Thing { id: thing }
      }
      """
    And the file "Plain.qml":
      """qml
      import QtQuick

      Item {
          id: plain
      }
      """
    And the file "Named.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts as L

      Item {
          id: named
          L.RowLayout { id: row }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  Scenario Outline: A member the object has no such thing as is refused
    When the agent calls the tool "<tool>" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "nothere", "value": <value>}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

    Examples:
      | tool         | value  |
      | property_set | 1      |
      | binding_set  | "1+1"  |

  Scenario Outline: What a toolkit type has by way of its bases is written
    Given the file "Wide.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      import QtQuick.Layouts

      Item {
          id: item
          Rectangle { id: rect }
          Text { id: text }
          MouseArea { id: area }
          Button { id: pick }
          Timer { id: tick }
          ListView { id: list }
          Image { id: pic }
          Flickable { id: flick }
          NumberAnimation { id: ani }
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Wide.qml", "node": {"id": "<node>"}, "name": "<name>", "value": "null"}
      """
    Then the file "Wide.qml" contains "<name>: null"

    Examples:
      | node  | name                   |
      | item  | layer.enabled          |
      | item  | focus                  |
      | item  | visibleChildren        |
      | item  | onWindowChanged        |
      | pick  | focusPolicy            |
      | pick  | onFocusPolicyChanged   |
      | list  | delegateModelAccess    |
      | item  | transform              |
      | item  | containmentMask        |
      | item  | childrenRect.width     |
      | item  | onChildrenRectChanged  |
      | item  | onParentChanged        |
      | item  | Keys.onPressed         |
      | item  | anchors.fill           |
      | item  | Layout.fillWidth       |
      | item  | onStateChanged         |
      | rect  | border.width           |
      | rect  | gradient               |
      | text  | onLinkActivated        |
      | text  | font.pixelSize         |
      | area  | onClicked              |
      | area  | drag.target            |
      | pick  | onClicked              |
      | pick  | background             |
      | pick  | palette.button         |
      | tick  | onTriggered            |
      | list  | onCurrentIndexChanged  |
      | list  | delegate               |
      | pic   | onStatusChanged        |
      | flick | contentItem            |
      | ani   | onFinished             |

  Scenario Outline: A handler for what the object does not say is refused
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "<name>", "value": "count = 1"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

    Examples:
      | name           |
      | onClicked      |
      | onHeightStolen |

  Scenario Outline: A handler for what the object says is written
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "<node>"}, "name": "<name>", "value": "count = 1"}
      """
    Then the file "Main.qml" contains "<name>: count = 1"

    Examples:
      | node  | name                 |
      | card  | onWidthChanged       |
      | card  | onVisibleChanged     |
      | root  | onCountChanged       |
      | root  | Component.onCompleted |
      | thing | onAnything           |

  Scenario Outline: What the object has is written
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "<node>"}, "name": "<name>", "value": 1}
      """
    Then the file "Main.qml" contains "<name>: 1"

    Examples:
      | node  | name                |
      | card  | width               |
      | card  | opacity             |
      | label | font.pixelSize      |
      | label | anchors.topMargin   |
      | card  | Layout.columnSpan   |
      | root  | count               |
      | thing | whatever            |

  Scenario Outline: A type nothing the document imports has is refused
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "<text>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "<why>"

    Examples:
      | text                     | why            |
      | RowLayout {}             | unknown_type   |
      | Item { RowLayout {} }    | unknown_type   |
      | Rectangle { nothere: 1 } | no_such_member |

  # qml6 6.11 loads all but the last two of these with these imports; Studio's
  # table has none of them, and what it cannot be sure of it lets through.
  Scenario Outline: A type Studio has no word on goes through
    Given the file "Wide.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      import QtQuick.Layouts

      Item {
          id: root
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Wide.qml", "parent": {"id": "root"}, "text": "<type> {}"}
      """
    Then the file "Wide.qml" contains "<type> {"

    Examples:
      | type               |
      | SearchField        |
      | FlexboxLayout      |
      | PathText           |
      | FunctionFilter     |
      | FunctionSorter     |
      | BorderImageMesh    |
      | HeaderViewDelegate |
      | Nowhere            |

  # qml6 6.11: `Cannot assign to non-existent property "radius"` on the Item.
  Scenario Outline: What only something else in the object says is not its own
    Given the file "Boxed.qml":
      """qml
      import QtQuick

      Item {
          id: box
          // radius: 3
          property string note: "radius: 3"
          Rectangle { radius: 3 }
      }
      """
    When the agent calls the tool "<tool>" with:
      """json
      {"doc": "Boxed.qml", "node": {"id": "box"}, "name": "radius", "value": <value>}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

    Examples:
      | tool         | value |
      | property_set | 3     |
      | binding_set  | "3"   |

  # qml6 6.11: Connections loads a made-up handler, with a warning, and refuses
  # a made-up property.
  Scenario: A Connections takes any handler and no made-up property
    Given the file "Wired.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Connections { id: wire; target: root }
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Wired.qml", "node": {"id": "wire"}, "name": "onClicked", "value": "root.x = 1"}
      """
    Then the file "Wired.qml" contains "onClicked: root.x = 1"
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Wired.qml", "node": {"id": "wire"}, "name": "nothere", "value": "1"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

  # qml6 6.11 loads a made-up role and a made-up change.
  Scenario Outline: What takes members of any name takes them
    Given the file "Roles.qml":
      """qml
      import QtQuick

      Item {
          id: root
          ListModel { ListElement { id: row } }
          states: State { PropertyChanges { id: change } }
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Roles.qml", "node": {"id": "<node>"}, "name": "madeUp", "value": "1"}
      """
    Then the file "Roles.qml" contains "madeUp: 1"

    Examples:
      | node   |
      | row    |
      | change |

  # qml6 6.11 loads `id: parent` and `id: property`.
  Scenario Outline: An id QML keeps for itself is refused wherever it is written
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Text { id: <word> }"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "not_an_id"

    Examples:
      | word     |
      | console  |
      | print    |
      | pragma   |

  Scenario Outline: A word that only reads like a kept one is an id
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Plain.qml", "nodes": [{"id": "plain"}], "name": "<word>"}
      """
    Then the file "Plain.qml" contains "id: <word>"

    Examples:
      | word     |
      | parent   |
      | property |

  Scenario Outline: A type the document has, or will, is inserted
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "<doc>", "parent": {"id": "<parent>"}, "text": "<type> {}"<more>}
      """
    Then the file "<doc>" contains "<type> {"

    Examples:
      | doc       | parent | type         | more                          |
      | Plain.qml | plain  | Rectangle    |                               |
      | Plain.qml | plain  | QtObject     |                               |
      | Plain.qml | plain  | RowLayout    | , "import": "QtQuick.Layouts" |
      | Main.qml  | root   | NotDescribed |                               |

  Scenario Outline: A type the toolkit's modules have is not called unknown
    Given the file "Wide.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      import QtQuick.Layouts

      Item {
          id: root
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Wide.qml", "parent": {"id": "root"}, "text": "<type> {}"}
      """
    Then the file "Wide.qml" contains "<type> {"

    Examples:
      | type |
      | ListModel |
      | Component |
      | Connections |
      | Binding |
      | Timer |
      | Instantiator |
      | ObjectModel |
      | DelegateModel |
      | Shortcut |
      | FontLoader |
      | Loader |
      | Repeater |
      | Canvas |
      | ShaderEffect |
      | DropArea |
      | TapHandler |
      | HoverHandler |
      | PointHandler |
      | DragHandler |
      | PinchHandler |
      | WheelHandler |
      | Behavior |
      | SmoothedAnimation |
      | SpringAnimation |
      | LayoutMirroring |
      | Popup |
      | Menu |
      | Dialog |
      | DialogButtonBox |
      | SplitView |
      | StackView |
      | SwipeView |
      | TabBar |
      | ToolBar |
      | ToolTip |
      | Drawer |
      | ScrollView |
      | QtObject |

  # qml6 6.11 loads each of these with `import QtQuick` alone.
  Scenario Outline: What comes with QtQuick is not called unknown
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "<type> {}"}
      """
    Then the file "Plain.qml" contains "<type> {"

    Examples:
      | type |
      | QtObject |
      | Timer |
      | ListModel |
      | ListElement |
      | WorkerScript |
      | Instantiator |
      | DelegateModel |
      | ObjectModel |
      | DelegateModelGroup |
      | ItemSelectionModel |
      | Package |
      | LoggingCategory |
      | StateGroup |
      | State |
      | Transition |
      | AnchorChanges |
      | ParentChange |
      | StateChangeScript |
      | Rotation |
      | Scale |
      | Translate |
      | Matrix4x4 |
      | TextMetrics |
      | FontMetrics |
      | Path |
      | PathLine |
      | PathArc |
      | PathCubic |
      | PathQuad |
      | PathSvg |
      | PathAttribute |
      | PathPercent |
      | PathInterpolator |
      | PathAnimation |
      | Row |
      | AnimatedImage |
      | AnimatedSprite |
      | Sprite |
      | SpriteSequence |
      | BorderImage |
      | IntValidator |
      | DoubleValidator |
      | RegularExpressionValidator |
      | SystemPalette |
      | ShaderEffectSource |
      | MultiPointTouchArea |
      | PinchArea |
      | TouchPoint |
      | Gradient |
      | GradientStop |
      | Vector3dAnimation |
      | ColorAnimation |
      | RotationAnimation |
      | SequentialAnimation |
      | ParallelAnimation |
      | PauseAnimation |
      | PropertyAction |
      | ScriptAction |
      | PropertyAnimation |
      | AnimationController |
      | TextInput |
      | TextEdit |
      | Window |

  # qml6 refuses even QtObject in a file that imports nothing.
  Scenario: In a file that imports nothing, no type is there
    Given the file "Bare.qml":
      """qml
      Item {
          id: bare
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Bare.qml", "parent": {"id": "bare"}, "text": "QtObject {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"

  Scenario: A paste into a document without the type brings its import
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import QtQuick.Layouts"
    And the file "Plain.qml" contains "RowLayout"

  # qml6 6.11: `RowLayout is not a type` beside a module without it, and loads
  # once QtQuick.Layouts is imported too.
  Scenario: A paste brings its import though another module may have the type
    Given the file "Native.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: native
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Native.qml", "parent": {"id": "native"}}
      """
    Then the file "Native.qml" contains "import QtQuick.Layouts"

  # qml6 6.11: `RowLayout { Thing {} }` needs both imports, whatever Studio can
  # say of `Thing`, and `Thing` may come from any import of the file it was in.
  Scenario Outline: A paste keeps the imports that may name what it holds
    Given the file "Mixed.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts
      import App.Native

      Item {
          id: mixed
          RowLayout { id: withthing; Thing {} }
          Thing { id: lone }
      }
      """
    And the file "Native.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: native
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Mixed.qml", "nodes": [{"id": "<node>"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "<doc>", "parent": {"id": "<parent>"}}
      """
    Then the file "<doc>" contains "<import>"

    Examples:
      | node      | doc        | parent | import                 |
      | withthing | Plain.qml  | plain  | import QtQuick.Layouts |
      | withthing | Plain.qml  | plain  | import App.Native      |
      | withthing | Native.qml | native | import QtQuick.Layouts |
      | lone      | Plain.qml  | plain  | import App.Native      |

  Scenario: A paste keeps the folder its file imported, from where it lands
    Given the file "lib/Far.qml":
      """qml
      import QtQuick

      Rectangle {}
      """
    And the file "From.qml":
      """qml
      import QtQuick
      import "lib"

      Item {
          id: from
          Far { id: far }
      }
      """
    And the file "deep/Land.qml":
      """qml
      import QtQuick

      Item {
          id: land
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "From.qml", "nodes": [{"id": "far"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "deep/Land.qml", "parent": {"id": "land"}}
      """
    Then the file "deep/Land.qml" contains "import \"../lib\""

  Scenario: A paste keeps the file beside the one it came from
    Given the file "sub/Beside.qml":
      """qml
      import QtQuick

      Rectangle {}
      """
    And the file "sub/Came.qml":
      """qml
      import QtQuick

      Item {
          id: came
          Beside { id: near }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "sub/Came.qml", "nodes": [{"id": "near"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import \"./sub\""

  # qml6 6.11: a bare import beside `as C` is valid, and `Button` needs it.
  Scenario Outline: A paste into a file that imports under another name still loads
    Given the file "Bare.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts
      import QtQuick.Controls

      Item {
          id: bare
          RowLayout { id: withbutton; Button {} }
          Item { id: withrow; RowLayout {} }
      }
      """
    And the file "Aliased.qml":
      """qml
      import QtQuick
      <import>

      Item {
          id: aliased
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Bare.qml", "nodes": [{"id": "<node>"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Aliased.qml", "parent": {"id": "aliased"}}
      """
    Then the file "Aliased.qml" contains:
      """
      <bare>

      Item {
      """
    And the file "Aliased.qml" contains "<text>"

    Examples:
      | node       | import                      | bare                   | text          |
      | withbutton | import QtQuick.Controls as C | import QtQuick.Controls | Button {}     |
      | withrow    | import QtQuick.Layouts as L  | import QtQuick.Layouts  | RowLayout {}  |

  # qml6 6.11: `Button` beside the file is the file under `import QtQuick`, and
  # the toolkit's once QtQuick.Controls is imported.
  Scenario: An import that would change what the file has stays out
    Given the file "Button.qml":
      """qml
      import QtQuick

      Item { objectName: "local" }
      """
    And the file "Holds.qml":
      """qml
      import QtQuick

      Item {
          id: holds
          Button { id: mine }
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: has
          Sprocket { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Holds.qml", "parent": {"id": "holds"}}
      """
    Then the file "Holds.qml" does not contain "import QtQuick.Controls"
    And the file "Holds.qml" contains "Sprocket"

  Scenario: An import that would change what the file has stays out, and an object that needs it stays out too
    Given the file "Button.qml":
      """qml
      import QtQuick

      Item { objectName: "local" }
      """
    And the file "Holds.qml":
      """qml
      import QtQuick

      Item {
          id: holds
          Button { id: mine }
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Holds.qml", "parent": {"id": "holds"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Holds.qml" does not contain "QtQuick.Controls"

  Scenario: A folder that would change what a module gave the file stays out
    Given the file "lib/Button.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Holds.qml":
      """qml
      import QtQuick.Controls

      Item {
          id: holds
          Button { id: mine; text: "x" }
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import "./lib"

      Item {
          id: has
          Gadget { id: g }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "g"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Holds.qml", "parent": {"id": "holds"}}
      """
    Then the file "Holds.qml" does not contain "lib"
    And the file "Holds.qml" contains "Gadget"

  # qml6 6.11: an import is found before the file beside the document.
  Scenario Outline: A pasted object stays what it was where a file beside has its name
    Given the file "Button.qml":
      """qml
      import QtQuick

      Item { objectName: "local" }
      """
    And the file "Holds.qml":
      """qml
      import QtQuick

      Item {
          id: holds
          <has>
      }
      """
    And the file "other/Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: has
          Button { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "other/Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Holds.qml", "parent": {"id": "holds"}}
      """
    Then <then>

    Examples:
      | has                 | then                                                        |
      | Item { id: inner }  | the file "Holds.qml" contains "import QtQuick.Controls"     |
      | Button { id: mine } | the answer's "/unvouched/0/was" is "QtQuick.Controls"       |

  # qml6 6.11: the folder imported last is found first, a folder before a module,
  # and a module before the file beside the document.
  Scenario: A pasted object stays the file beside the one it came from
    Given the file "Button.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "other/Button.qml":
      """qml
      import QtQuick

      Rectangle {}
      """
    And the file "other/Has.qml":
      """qml
      import QtQuick

      Item {
          id: has
          Button { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "other/Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import \"./other\""

  Scenario: A toolkit's object goes in where a folder the file imports has its name, and the paste says so
    Given the file "lib/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Lib.qml":
      """qml
      import QtQuick
      import "./lib"

      Item {
          id: holder
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Lib.qml", "parent": {"id": "holder"}}
      """
    Then the answer's "/unvouched/0/was" is "QtQuick.Controls"
    And the answer's "/unvouched/0/now" is "the folder lib"

  Scenario: A paste says what it imported
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the answer's "/imported" lists "QtQuick.Layouts"

  Scenario: A paste keeps the file a qualified type is in, beside the one it came from
    Given the file "sub/Other.qml":
      """qml
      import QtQuick

      Item { component Inner: Rectangle {} }
      """
    And the file "sub/Came.qml":
      """qml
      import QtQuick

      Item {
          id: came
          Other.Inner { id: near }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "sub/Came.qml", "nodes": [{"id": "near"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import \"./sub\""

  Scenario Outline: A paste copies a folder that is not relative as it is
    Given the file "sub/Came.qml":
      """qml
      import QtQuick
      import "<path>"

      Item {
          id: came
          Sprocket { id: near }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "sub/Came.qml", "nodes": [{"id": "near"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import \"<path>\""

    Examples:
      | path                   |
      | file:/opt/lib          |
      | https://example.org/q/ |

  # qml6 6.11 loads `Button` from `import "file:<dir>"`, which Studio can't read.
  Scenario: A toolkit's name may come from a folder Studio cannot read
    Given the file "sub/Came.qml":
      """qml
      import QtQuick
      import "file:/opt/lib"

      Item {
          id: came
          Slider { id: near }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "sub/Came.qml", "nodes": [{"id": "near"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import \"file:/opt/lib\""
    And the file "Plain.qml" contains "Slider"

  Scenario: A paste of a qualified type is not let through by a module nothing is known of
    Given the file "Native.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: native
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Named.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Native.qml", "parent": {"id": "native"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"

  # qml6 6.11 loads `Other.Inner` for a file beside it, and `Far.Inner` for a
  # folder it imports, and refuses `Nothing.Inner`.
  Scenario Outline: A qualified type is refused only when its qualifier is certainly nothing
    Given the file "Other.qml":
      """qml
      import QtQuick

      Item { component Inner: Rectangle {} }
      """
    And the file "lib/Far.qml":
      """qml
      import QtQuick

      Item { component Inner: Rectangle {} }
      """
    And the file "Where.qml":
      """qml
      import QtQuick
      <import>

      Item {
          id: where
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Where.qml", "parent": {"id": "where"}, "text": "<type> {}"}
      """
    Then the file "Where.qml" contains "<type> {"

    Examples:
      | type      | import       |
      | Other.Inner | import QtQml |
      | Far.Inner | import "lib" |
      | Thing.Inner | import App.Native |

  Scenario: A name Studio cannot vouch for is said to be so, not free
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "Sprocket"}
      """
    Then the file "Sprocket.qml" contains "Rectangle"
    And the answer's "/name_unsure" is true

  Scenario: A paste that needs what no plain import brings is refused
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Named.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"

  Scenario Outline: A component is not named what the document's imports have
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "<name>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "name_taken"
    And the file "Main.qml" contains "Rectangle { id: card"

    Examples:
      | name      |
      | Text      |
      | GridLayout |
      | Timer     |
      | QtObject  |
      | AnimatedSprite |
      | WorkerScript |
      | PathAnimation |

  Scenario Outline: A layout keeps only what its new kind has
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "relayout", "doc": "Main.qml", "nodes": [{"id": "<node>"}], "name": "<kind>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "says_other"

    Examples:
      | node | kind       |
      | row  | GridLayout |
      | grid | RowLayout  |

  Scenario: What a state changes is what the object has
    When the agent calls the tool "state_edit" with:
      """json
      {"action": "change", "doc": "Main.qml", "state": "on", "target": {"id": "card"}, "name": "nothere", "value": 1}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

  Scenario Outline: A toolkit's object stays one where the file beside the target has its name, with a module nobody can read in <case>
    Given the file "Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Nat.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: holder
      }
      """
    And the file "other/Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      <also>

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "other/Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "<doc>", "parent": {"id": "<parent>"}}
      """
    Then the file "<doc>" contains "import QtQuick.Controls"

    Examples:
      | case       | also              | doc       | parent |
      | the source | import App.Native | Plain.qml | plain  |
      | the target | import QtQuick    | Nat.qml   | holder |

  Scenario: A file that beat a module in its source stays that file
    Given the file "lib/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      import "./lib"

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import \"./lib\""
    And the file "Plain.qml" does not contain "QtQuick.Controls"

  Scenario: A module that beat the file beside its source stays that module
    Given the file "sub/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "sub/Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "sub/Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import QtQuick.Controls"
    And the file "Plain.qml" does not contain "./sub"

  Scenario: Two folders come in the order that keeps which one wins
    Given the file "a/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "a/Foo.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "b/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import "./a"
      import "./b"

      Item {
          id: has
          Item {
              id: both
              Slider {}
              Foo {}
          }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "both"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains:
      """
      import "./a"
      import "./b"
      """

  Scenario: A paste says so when the folder one part needs takes another part's name
    Given the file "a/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "a/Foo.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "c/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Target.qml":
      """qml
      import QtQuick
      import "./c"

      Item {
          id: holder
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import "./a"
      import "./c"

      Item {
          id: has
          Item {
              id: both
              Slider {}
              Foo {}
          }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "both"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Target.qml", "parent": {"id": "holder"}}
      """
    Then the file "Target.qml" contains "import \"./a\""
    And the answer's "/unvouched/0/type" is "Slider"
    And the answer's "/unvouched/0/was" is "the folder c"
    And the answer's "/unvouched/0/now" is "the folder a"

  Scenario: A folder the target already imports isn't moved, and the paste says what that cost
    Given the file "a/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "c/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Target.qml":
      """qml
      import QtQuick
      import "./a"
      import "./c"

      Item {
          id: holder
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import "./a"

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Target.qml", "parent": {"id": "holder"}}
      """
    Then the answer's "/unvouched/0/was" is "the folder a"
    And the answer's "/unvouched/0/now" is "the folder c"
    And the answer's "/imported" is absent

  Scenario: Objects copied together bring their folders in the source's order
    Given the file "a/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "a/Foo.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "b/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import "./a"
      import "./b"

      Item {
          id: has
          Slider { id: sl }
          Foo { id: fo }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "sl"}, {"id": "fo"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains:
      """
      import "./a"
      import "./b"
      """

  Scenario: An unread module under an alias can't have a bare name, so it changes no answer
    Given the file "a/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "a/Foo.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "c/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Target.qml":
      """qml
      import QtQuick
      import "./c"

      Item {
          id: holder
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import App.Native as N
      import "./a"
      import "./c"

      Item {
          id: has
          Item {
              id: both
              Slider {}
              Foo {}
          }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "both"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Target.qml", "parent": {"id": "holder"}}
      """
    Then the answer's "/unvouched/0/was" is "the folder c"
    And the answer's "/unvouched/0/now" is "the folder a"

  Scenario: What Studio can't read comes along, and the paste says it couldn't vouch for it
    Given the file "Has.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: has
          Thing { id: t }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "t"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import App.Native"
    And the answer's "/unvouched/0/type" is "Thing"
    And the answer's "/unvouched/0/was" is "can't tell"

  Scenario: A paste within a file vouches for itself, whatever it imports
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "thing"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Main.qml", "parent": {"id": "root"}}
      """
    Then the answer's "/unvouched" is absent

  # qml6 6.11: a file beside the document loses to a module, and any file to a
  # folder, so an import can move what the file has even where Studio can't say
  # what the file's name was.
  Scenario Outline: An import that may move what the target has is said, where the target can't be read
    Given the file "Button.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "lib/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "lib/Button.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Nat.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: nat
          Button { id: mine }
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import <import>

      Item {
          id: has
          Slider { id: s }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "s"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Nat.qml", "parent": {"id": "nat"}}
      """
    Then the answer's "/unvouched/0/type" is "Button"
    And the answer's "/unvouched/0/held" is true
    And the answer's "/unvouched/0/was" is "can't tell"
    And the answer's "/unvouched/0/now" is "<now>"

    Examples:
      | import           | now              |
      | QtQuick.Controls | QtQuick.Controls |
      | "./lib"          | the folder lib   |

  Scenario: An inline component's name doesn't hide a longer one's neighbour in a paste
    Given the file "Has.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: has
          Item {
              id: box
              component SliderHandle: Rectangle {}
              Slider { id: s }
          }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "box"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import QtQuick.Controls"

  Scenario: A paste's own inline component needs no import and is vouched for
    Given the file "Has.qml":
      """qml
      import QtQuick

      Item {
          id: has
          Item {
              id: box
              component Handle: Rectangle {}
              Handle { id: h }
          }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "box"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "Handle { id: h }"
    And the answer's "/unvouched" is absent

  Scenario Outline: A name is not known for an inline component whose name only starts with it, or for a word in a comment
    Given the file "Handled.qml":
      """qml
      import QtQuick

      Item {
          id: handled
          <beside>
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Handled.qml", "parent": {"id": "handled"}, "text": "Slider {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"

    Examples:
      | beside                                |
      | component SliderHandle: Rectangle {}  |
      | // component Slider is for later      |

  Scenario Outline: An inline component the file already declares is refused, since qml6 won't load the file
    Given the file "Dup.qml":
      """qml
      import QtQuick

      Item {
          id: dup
          Item {
              id: first
              component Twice: Rectangle {}
          }
      }
      """
    When the agent calls the tool "<tool>" with:
      """json
      <args>
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "duplicate_component"
    And the file "Dup.qml" does not contain "id: second"

    Examples:
      | tool         | args                                                                                                    |
      | node_insert  | {"doc": "Dup.qml", "parent": {"id": "dup"}, "text": "Item { id: second; component Twice: Item {} }"}    |
      | node_arrange | {"action": "duplicate", "doc": "Dup.qml", "nodes": [{"id": "first"}]}                                   |

  Scenario: A paste of an inline component the target declares is refused
    Given the file "Has.qml":
      """qml
      import QtQuick

      Item {
          id: has
          Item {
              id: second
              component Twice: Item {}
          }
      }
      """
    And the file "Dup.qml":
      """qml
      import QtQuick

      Item {
          id: dup
          component Twice: Rectangle {}
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "second"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Dup.qml", "parent": {"id": "dup"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "duplicate_component"
    And the file "Dup.qml" does not contain "id: second"

  Scenario: A pasted inline component taking a name the target uses is said, and the file is written
    Given the file "Has.qml":
      """qml
      import QtQuick

      Item {
          id: has
          Item {
              id: box
              component Button: Rectangle {}
              Button { id: h }
          }
      }
      """
    And the file "Own.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: own
          Button { id: mine; width: 3 }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "box"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Own.qml", "parent": {"id": "own"}}
      """
    Then the file "Own.qml" contains "component Button: Rectangle {}"
    And the answer's "/unvouched/0/type" is "Button"
    And the answer's "/unvouched/0/held" is true
    And the answer's "/unvouched/0/was" is "QtQuick.Controls"
    And the answer's "/unvouched/0/now" is "an inline component in this file"

  Scenario: A pasted name the target's own inline component takes is said, and brings no import
    Given the file "lib/Thing.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import "./lib"

      Item {
          id: has
          Thing { id: t }
      }
      """
    And the file "Own.qml":
      """qml
      import QtQuick

      Item {
          id: own
          component Thing: Rectangle {}
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "t"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Own.qml", "parent": {"id": "own"}}
      """
    Then the file "Own.qml" does not contain "./lib"
    And the answer's "/unvouched/0/type" is "Thing"
    And the answer's "/unvouched/0/was" is "the folder lib"
    And the answer's "/unvouched/0/now" is "an inline component in this file"

  Scenario: An import that can't have touched what the target has doesn't list it
    Given the file "Thing.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Nat.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: nat
          Thing { id: mine }
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts

      Item {
          id: has
          RowLayout { id: r }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "r"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Nat.qml", "parent": {"id": "nat"}}
      """
    Then the file "Nat.qml" contains "import QtQuick.Layouts"
    And the answer's "/unvouched" is absent

  Scenario: An added import that can't be read keeps what the target has listed
    Given the file "Thing.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Nat.qml":
      """qml
      import QtQuick
      import App.Native

      Item {
          id: nat
          Thing { id: mine }
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick
      import App.Other

      Item {
          id: has
          Zed { id: z }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "z"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Nat.qml", "parent": {"id": "nat"}}
      """
    Then the file "Nat.qml" contains "import App.Other"
    And the answer's "/unvouched/1/type" is "Thing"
    And the answer's "/unvouched/1/held" is true

  Scenario Outline: A pasted inline component that takes the head of a qualified name is refused, unless the head is an alias
    Given the file "Other.qml":
      """qml
      import QtQuick

      Item {
          component Inner: Rectangle {}
      }
      """
    And the file "Has.qml":
      """qml
      import QtQuick

      Item {
          id: has
          Item {
              id: box
              component <head>: Item {}
          }
      }
      """
    And the file "Own.qml":
      """qml
      import QtQuick
      <import>

      Item {
          id: own
          <object> { id: mine }
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Has.qml", "nodes": [{"id": "box"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Own.qml", "parent": {"id": "own"}}
      """
    Then <outcome>

    Examples:
      | head  | import                       | object      | outcome                                                |
      | Other | // nothing                   | Other.Inner | the agent is refused with "not_actionable"             |
      | C     | import QtQuick.Controls as C | C.Button    | the file "Own.qml" contains "component C: Item {}"     |

  Scenario: A pasted qualified name whose head the target's inline component takes is refused, since qml6 won't load it
    Given the file "lib/Other.qml":
      """qml
      import QtQuick

      Item {
          component Inner: Rectangle {}
      }
      """
    And the file "lib/Has.qml":
      """qml
      import QtQuick

      Item {
          id: has
          Other.Inner { id: t }
      }
      """
    And the file "Own.qml":
      """qml
      import QtQuick

      Item {
          id: own
          component Other: Rectangle {}
      }
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "lib/Has.qml", "nodes": [{"id": "t"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Own.qml", "parent": {"id": "own"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Own.qml" does not contain "./lib"
