// The top of the window: the panels on or off, undo and redo, the target the
// canvas runs on, live or mocked data, the look, and export, the one accent.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

ToolBar {
    id: bar
    required property Workspace workspace
    readonly property var preview: workspace.preview
    property bool leftOpen: true
    property bool rightOpen: true
    property bool codeOpen: true
    // The window shows the export panel when this is called.
    signal exportRequested

    // A preview of the open document on another target or data, as an
    // agent starts one.
    function open(target, state) {
        workspace.dispatch("preview_open", {doc: workspace.doc, target: target, state: state})
    }

    implicitHeight: 44
    padding: 0
    background: Rectangle {
        color: Colors.surface
        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: 1
            color: Colors.surfacePressed
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Spacing.xs
        anchors.rightMargin: Spacing.xs
        spacing: Spacing.xxs

        IconButton {
            icon_: "sidebarLeft"
            text: "Left panels"
            checkable: true
            checked: bar.leftOpen
            onClicked: bar.leftOpen = checked
        }
        Label {
            text: bar.workspace.doc === "" ? "Ljós Studio" : bar.workspace.doc
            color: Colors.text
            font.pixelSize: Typography.bodySize
            font.weight: Typography.bodyStrongWeight
            elide: Text.ElideRight
            leftPadding: Spacing.xs
            rightPadding: Spacing.xs
            Layout.maximumWidth: 200
        }
        Hairline {}
        IconButton {
            icon_: "undo"
            text: "Undo"
            enabled: bar.workspace.done.length > 0
            onClicked: bar.workspace.dispatch("history", {action: "undo"})
        }
        IconButton {
            icon_: "redo"
            text: "Redo"
            enabled: bar.workspace.undone.length > 0
            onClicked: bar.workspace.dispatch("history", {action: "redo"})
        }

        Item {
            Layout.fillWidth: true
        }

        ButtonGroup {
            id: targets
        }
        Repeater {
            model: [{name: "Desktop", icon: "desktop"}, {name: "Tablet", icon: "tablet"},
                    {name: "Phone", icon: "phone"}]
            delegate: IconButton {
                required property var modelData
                icon_: modelData.icon
                text: modelData.name
                checkable: true
                checked: bar.preview !== null && bar.preview.target === modelData.name
                ButtonGroup.group: targets
                enabled: bar.workspace.previews && bar.workspace.doc !== ""
                onClicked: bar.open(modelData.name.toLowerCase(),
                                    bar.preview !== null ? bar.preview.state : "live")
            }
        }
        Hairline {}
        TextButton {
            text: bar.preview !== null && bar.preview.state === "mocked" ? "Mocked" : "Live"
            Accessible.name: "Data"
            Accessible.description: text
            enabled: bar.preview !== null
            onClicked: bar.open(bar.preview.target.toLowerCase(),
                                bar.preview.state === "mocked" ? "live" : "mocked")
        }

        Item {
            Layout.fillWidth: true
        }

        IconButton {
            icon_: Colors.dark ? "sun" : "moon"
            text: Colors.dark ? "Light look" : "Dark look"
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
        IconButton {
            icon_: "sidebarRight"
            text: "Right panels"
            checkable: true
            checked: bar.rightOpen
            onClicked: bar.rightOpen = checked
        }
        Hairline {}
        TextButton {
            text: "Export"
            accent: true
            Accessible.name: text
            Accessible.description: "Write a bundle (Ctrl+E)"
            onClicked: bar.exportRequested()
        }
    }
}
