// A small picture of what a palette tile places, drawn from the theme's roles
// so it looks like the thing will: the tile shows a slider, not the idea of
// one. Decoration: the tile says the name. A type with no picture of its own
// has its kind's icon.
import QtQuick
import Ljos.Design

Item {
    id: sketch
    // `text`, `field`, `image`, `slider`… as the palette sorts types.
    required property string kind
    property color ink: Colors.text
    implicitWidth: 44
    implicitHeight: 24
    Accessible.ignored: true

    readonly property var drawn: ({
        text: textSketch, field: fieldSketch, image: imageSketch, frame: frameSketch,
        button: buttonSketch, checkbox: checkSketch, radio: radioSketch, toggle: toggleSketch,
        slider: sliderSketch, progress: progressSketch, combo: comboSketch, row: rowSketch,
        column: columnSketch, grid: gridSketch, list: listSketch, tabs: tabsSketch,
        window: windowSketch
    })

    Loader {
        anchors.centerIn: parent
        sourceComponent: sketch.drawn[sketch.kind] !== undefined ? sketch.drawn[sketch.kind] : iconSketch
    }

    Component {
        id: iconSketch
        Icon { name: sketch.kind; size: 24; color: sketch.ink }
    }
    Component {
        id: textSketch
        Text {
            text: "Aa"
            color: sketch.ink
            font.pixelSize: Typography.subtitleSize
            font.weight: Typography.bodyStrongWeight
        }
    }
    Component {
        id: fieldSketch
        Rectangle {
            width: 42; height: 18; radius: 3
            color: Colors.surface
            border.color: Colors.border
            Rectangle { x: 5; y: 4; width: 1; height: 10; color: sketch.ink }
        }
    }
    Component {
        id: imageSketch
        Rectangle {
            width: 36; height: 24; radius: 2; clip: true
            color: Colors.accent
            opacity: 0.85
            Rectangle { x: 23; y: 4; width: 7; height: 7; radius: 3.5; color: Colors.textOnAccent }
            Rectangle { x: -4; y: 15; width: 26; height: 26; radius: 13; color: sketch.ink; opacity: 0.55 }
            Rectangle { x: 14; y: 18; width: 30; height: 30; radius: 15; color: sketch.ink; opacity: 0.75 }
        }
    }
    Component {
        id: frameSketch
        Rectangle {
            width: 34; height: 22; radius: 3
            color: Colors.surfacePressed
            border.color: Colors.border
        }
    }
    Component {
        id: buttonSketch
        Rectangle {
            width: 40; height: 18; radius: 4
            color: Colors.accent
            Rectangle { anchors.centerIn: parent; width: 20; height: 2; radius: 1; color: Colors.textOnAccent }
        }
    }
    Component {
        id: checkSketch
        Rectangle {
            width: 18; height: 18; radius: 3
            color: Colors.accent
            Icon { anchors.centerIn: parent; name: "check"; size: 14; color: Colors.textOnAccent }
        }
    }
    Component {
        id: radioSketch
        Rectangle {
            width: 18; height: 18; radius: 9
            color: "transparent"
            border.color: Colors.accent
            border.width: 2
            Rectangle { anchors.centerIn: parent; width: 8; height: 8; radius: 4; color: Colors.accent }
        }
    }
    Component {
        id: toggleSketch
        Rectangle {
            width: 32; height: 18; radius: 9
            color: Colors.accent
            Rectangle { x: 16; y: 2; width: 14; height: 14; radius: 7; color: Colors.textOnAccent }
        }
    }
    Component {
        id: sliderSketch
        Item {
            width: 44; height: 16
            Rectangle { y: 7; width: parent.width; height: 2; radius: 1; color: Colors.border }
            Rectangle { y: 7; width: 26; height: 2; radius: 1; color: Colors.accent }
            Rectangle { x: 20; y: 2; width: 12; height: 12; radius: 6; color: Colors.accent }
        }
    }
    Component {
        id: progressSketch
        Rectangle {
            width: 44; height: 6; radius: 3
            color: Colors.border
            Rectangle { width: 28; height: 6; radius: 3; color: Colors.accent }
        }
    }
    Component {
        id: comboSketch
        Rectangle {
            width: 44; height: 18; radius: 3
            color: Colors.surface
            border.color: Colors.border
            Rectangle { x: 5; y: 8; width: 18; height: 2; radius: 1; color: sketch.ink }
            Icon { x: 29; y: 2; name: "chevronDown"; size: 14; color: sketch.ink }
        }
    }
    Component {
        id: rowSketch
        Row {
            spacing: 3
            Repeater {
                model: 3
                Rectangle { width: 11; height: 20; radius: 2; color: Colors.surfacePressed; border.color: Colors.border }
            }
        }
    }
    Component {
        id: columnSketch
        Column {
            spacing: 2
            Repeater {
                model: 3
                Rectangle { width: 34; height: 6; radius: 2; color: Colors.surfacePressed; border.color: Colors.border }
            }
        }
    }
    Component {
        id: gridSketch
        Grid {
            columns: 3
            spacing: 2
            Repeater {
                model: 6
                Rectangle { width: 10; height: 10; radius: 2; color: Colors.surfacePressed; border.color: Colors.border }
            }
        }
    }
    Component {
        id: listSketch
        Column {
            spacing: 3
            Repeater {
                model: 3
                Row {
                    spacing: 3
                    Rectangle { width: 6; height: 5; radius: 1; color: Colors.accent }
                    Rectangle { width: 28; height: 5; radius: 1; color: Colors.border }
                }
            }
        }
    }
    Component {
        id: tabsSketch
        Row {
            spacing: 2
            Repeater {
                model: 3
                Column {
                    required property int index
                    spacing: 2
                    Rectangle { width: 12; height: 4; radius: 1; color: Colors.border }
                    Rectangle { width: 12; height: 2; color: parent.index === 0 ? Colors.accent : "transparent" }
                }
            }
        }
    }
    Component {
        id: windowSketch
        Rectangle {
            width: 34; height: 24; radius: 3
            color: Colors.surface
            border.color: Colors.border
            Rectangle { x: 1; y: 1; width: 32; height: 5; radius: 2; color: Colors.surfacePressed }
        }
    }
}
