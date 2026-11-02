// Studio's own shell: panels that skin the one block Studio binds,
// Studio.Workspace. Every gesture is a `dispatch` of a Studio tool, the
// call an agent makes over MCP. The window (../Window.qml) shows it when
// the person has no shell of their own, or when theirs does not work; a
// person's shell starts as a copy of this directory.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Item {
    id: shell
    // What the window's title says.
    readonly property string title: workspace.doc === "" ? "Ljós Studio" : workspace.doc + " · Ljós Studio"

    Workspace {
        id: workspace
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        onActivated: workspace.dispatch("history", {action: "undo"})
    }
    Shortcut {
        sequences: [StandardKey.Redo]
        onActivated: workspace.dispatch("history", {action: "redo"})
    }

    Shortcut {
        sequence: "Ctrl+E"
        onActivated: exporter.show()
    }

    ExportPanel {
        id: exporter
        workspace: workspace
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Toolbar {
            id: toolbar
            workspace: workspace
            onExportRequested: exporter.show()
            Layout.fillWidth: true
        }

        SplitView {
            orientation: Qt.Horizontal
            handle: Rectangle {
                implicitWidth: 5
                color: SplitHandle.hovered || SplitHandle.pressed ? Colors.border : Colors.surfacePressed
            }
            Layout.fillWidth: true
            Layout.fillHeight: true

            // What the document is made of, and what can go in it.
            ColumnLayout {
                visible: toolbar.leftOpen
                spacing: 0
                SplitView.preferredWidth: 260
                SplitView.minimumWidth: 200

                TabStrip {
                    id: leftTabs
                    tabs: ["Layers", "Insert"]
                    Layout.fillWidth: true
                }
                ColumnLayout {
                    visible: leftTabs.current === 0
                    spacing: 0
                    Layout.fillWidth: true
                    Layout.fillHeight: true

                    Section {
                        title: "Documents"
                        Layout.fillWidth: true
                        DocumentsPanel {
                            workspace: workspace
                            Layout.fillWidth: true
                            Layout.preferredHeight: 150
                        }
                    }
                    LayersPanel {
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                }
                PalettePanel {
                    visible: leftTabs.current === 1
                    workspace: workspace
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                }
            }

            SplitView {
                orientation: Qt.Vertical
                SplitView.fillWidth: true
                handle: Rectangle {
                    implicitHeight: 5
                    color: SplitHandle.hovered || SplitHandle.pressed ? Colors.border : Colors.surfacePressed
                }

                CanvasPanel {
                    workspace: workspace
                    recording: inspector.recording
                    onDoneRecording: inspector.pick("", "")
                    SplitView.fillHeight: true
                }
                CodePanel {
                    visible: toolbar.codeOpen
                    workspace: workspace
                    SplitView.preferredHeight: 180
                    SplitView.minimumHeight: 80
                }
            }

            // What the selection is, and what the document's look and flow are.
            ColumnLayout {
                visible: toolbar.rightOpen
                spacing: 0
                SplitView.preferredWidth: 300
                SplitView.minimumWidth: 220

                TabStrip {
                    id: rightTabs
                    tabs: ["Design", "Tokens", "Flow"]
                    Layout.fillWidth: true
                }
                InspectorPanel {
                    id: inspector
                    visible: rightTabs.current === 0
                    workspace: workspace
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                }
                TokensPanel {
                    visible: rightTabs.current === 1
                    workspace: workspace
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                }
                FlowPanel {
                    visible: rightTabs.current === 2
                    workspace: workspace
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                }
            }
        }

        // The person's own last refusal, until the next gesture is answered.
        Label {
            objectName: "status"
            Accessible.role: Accessible.StatusBar
            Accessible.name: "Status"
            padding: Spacing.xs
            text: workspace.error !== "" ? workspace.error : workspace.problem
            color: Colors.text
            font.pixelSize: Typography.captionSize
            background: Rectangle {
                color: Colors.surfaceRaised
            }
            visible: text !== ""
            Layout.fillWidth: true
        }
    }
}
