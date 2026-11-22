// The top of the window, in clusters: the file and its history; the target
// the canvas runs on; how it is shown (data, look, code); and export, the one
// accent. Every tool is an icon whose tip names it and its keys.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

ToolBar {
    id: bar
    required property Workspace workspace
    readonly property var preview: workspace.preview
    readonly property string shownTarget: preview !== null ? preview.target : ""
    // The target the canvas first ran on. The preview says it after the
    // window has opened, and only a switch from it fades.
    property string openedOn: ""
    property bool switched: false
    property bool leftOpen: true
    property bool rightOpen: true
    property bool codeOpen: true
    // How many problems the left panels hold, for when they are folded.
    property int problems: 0
    // The window shows the export panel when this is called.
    signal exportRequested

    onShownTargetChanged: {
        if (shownTarget === "")
            return
        if (openedOn === "")
            openedOn = shownTarget
        else if (shownTarget !== openedOn)
            switched = true
    }

    // A preview of the open document on another target or data, as an
    // agent starts one.
    function open(target, state) {
        workspace.dispatch("preview_open", {doc: workspace.doc, target: target, state: state})
    }

    implicitHeight: 44
    padding: 0
    background: Rectangle {
        color: Colors.canvas
    }

    // A cluster of tools on one soft well, so a group reads as one thing.
    component Well: Rectangle {
        default property alias tools: row.data
        implicitWidth: row.implicitWidth + 4
        implicitHeight: 32
        radius: 8
        color: Colors.surface
        border.width: 1
        border.color: Colors.surfacePressed
        Row {
            id: row
            anchors.centerIn: parent
            spacing: 2
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Spacing.xs
        anchors.rightMargin: Spacing.xs
        spacing: Spacing.xs

        // The file and its history.
        IconButton {
            icon_: "sidebarLeft"
            text: "Left panels"
            checkable: true
            checked: bar.leftOpen
            // The click ends the binding, and the problems' button opens too.
            onClicked: {
                bar.leftOpen = checked
                checked = Qt.binding(() => bar.leftOpen)
            }
        }
        RowLayout {
            spacing: Spacing.xs
            Layout.leftMargin: Spacing.xxs
            // Studio's mark: a spark of its accent.
            Rectangle {
                implicitWidth: 20
                implicitHeight: 20
                radius: 6
                color: Colors.accent
                Accessible.ignored: true
                Icon {
                    anchors.centerIn: parent
                    name: "component"
                    size: 14
                    color: Colors.textOnAccent
                }
            }
            Label {
                text: bar.workspace.doc === "" ? "Ljós Studio" : bar.workspace.doc
                color: Colors.text
                font.pixelSize: Typography.subtitleSize
                font.weight: Typography.subtitleWeight
                elide: Text.ElideRight
                Layout.maximumWidth: 220
            }
        }
        Well {
            IconButton {
                icon_: "undo"
                text: "Undo"
                shortcut: "Ctrl+Z"
                enabled: bar.workspace.done.length > 0
                onClicked: bar.workspace.dispatch("history", {action: "undo"})
            }
            IconButton {
                icon_: "redo"
                text: "Redo"
                shortcut: "Ctrl+Shift+Z"
                enabled: bar.workspace.undone.length > 0
                onClicked: bar.workspace.dispatch("history", {action: "redo"})
            }
        }

        Item {
            Layout.fillWidth: true
        }

        // Where the canvas runs: a breakpoint each.
        ButtonGroup {
            id: targets
        }
        Well {
            Repeater {
                model: [{name: "Desktop", icon: "desktop"}, {name: "Tablet", icon: "tablet"},
                        {name: "Phone", icon: "phone"}]
                delegate: IconButton {
                    required property var modelData
                    icon_: modelData.icon
                    text: modelData.name
                    checkable: true
                    checked: bar.preview !== null && bar.preview.target === modelData.name
                    fades: bar.switched || !checked
                        || (bar.openedOn !== "" && bar.openedOn !== modelData.name)
                    ButtonGroup.group: targets
                    enabled: bar.workspace.previews && bar.workspace.doc !== ""
                    onClicked: bar.open(modelData.name.toLowerCase(),
                                        bar.preview !== null ? bar.preview.state : "live")
                }
            }
        }
        // How it is shown: on live or mocked data, in which look, with its code.
        Well {
            // One name, read and seen alike; whether it is on is its check.
            TextButton {
                readonly property bool mocked: bar.preview !== null && bar.preview.state === "mocked"
                text: "Mocked data"
                Accessible.name: text
                checkable: true
                checked: mocked
                Accessible.description: mocked ? "The canvas runs on the project's mocks"
                    : "The canvas runs on live data"
                enabled: bar.preview !== null
                onClicked: {
                    bar.open(bar.preview.target.toLowerCase(), mocked ? "live" : "mocked")
                    // A click toggles a checkable button and ends its binding.
                    checked = Qt.binding(() => mocked)
                }
            }
            // Studio's own look: the canvas shows the app as the app has it.
            IconButton {
                icon_: Colors.dark ? "sun" : "moon"
                text: Colors.dark ? "Studio in light" : "Studio in dark"
                Accessible.description: "The canvas keeps the app's own look"
                onClicked: Colors.dark = !Colors.dark
            }
            IconButton {
                icon_: "code"
                text: "Code"
                checkable: true
                checked: bar.codeOpen
                onClicked: bar.codeOpen = checked
                Accessible.name: "Show code"
            }
        }

        Item {
            Layout.fillWidth: true
            // In the space, so the targets stay in the middle as it comes.
            TextButton {
                visible: !bar.leftOpen && bar.problems > 0
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, parent.width)
                text: "Problems: " + bar.problems
                Accessible.name: "Problems in the left panels"
                onClicked: bar.leftOpen = true
            }
        }

        // Out of Studio: a bundle, then the right panels' toggle at the edge,
        // as the left one is.
        TextButton {
            text: "Export"
            icon_: "export"
            accent: true
            Accessible.name: text
            Accessible.description: "Write a bundle (Ctrl+E)"
            onClicked: bar.exportRequested()
        }
        IconButton {
            icon_: "sidebarRight"
            text: "Right panels"
            checkable: true
            checked: bar.rightOpen
            onClicked: bar.rightOpen = checked
        }
    }
}
