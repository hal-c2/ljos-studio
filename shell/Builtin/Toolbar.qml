// Undo, redo, the target the canvas runs on, and live or mocked data.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

ToolBar {
    id: bar
    required property Workspace workspace
    readonly property var preview: workspace.preview
    // The window shows the export panel when this is called.
    signal exportRequested

    // A preview of the open document on another target or data, as an
    // agent starts one.
    function open(target, state) {
        workspace.dispatch("preview_open", {doc: workspace.doc, target: target, state: state})
    }

    RowLayout {
        anchors.fill: parent
        spacing: Spacing.xs

        ToolButton {
            text: "Undo"
            Accessible.name: text
            enabled: bar.workspace.done.length > 0
            onClicked: bar.workspace.dispatch("history", {action: "undo"})
        }
        ToolButton {
            text: "Redo"
            Accessible.name: text
            enabled: bar.workspace.undone.length > 0
            onClicked: bar.workspace.dispatch("history", {action: "redo"})
        }

        ToolSeparator {}

        ButtonGroup {
            id: targets
        }
        Repeater {
            model: ["Desktop", "Tablet", "Phone"]
            delegate: ToolButton {
                required property string modelData
                text: modelData
                Accessible.name: text
                checkable: true
                checked: bar.preview !== null && bar.preview.target === modelData
                ButtonGroup.group: targets
                enabled: bar.workspace.previews && bar.workspace.doc !== ""
                onClicked: bar.open(modelData.toLowerCase(),
                                    bar.preview !== null ? bar.preview.state : "live")
            }
        }

        ToolSeparator {}

        ToolButton {
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

        ToolButton {
            text: "Export"
            Accessible.name: text
            Accessible.description: "Write a bundle (Ctrl+E)"
            onClicked: bar.exportRequested()
        }
    }
}
