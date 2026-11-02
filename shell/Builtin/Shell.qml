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
            workspace: workspace
            onExportRequested: exporter.show()
            Layout.fillWidth: true
        }

        SplitView {
            orientation: Qt.Horizontal
            Layout.fillWidth: true
            Layout.fillHeight: true

            SplitView {
                orientation: Qt.Vertical
                SplitView.preferredWidth: 260
                SplitView.minimumWidth: 160

                DocumentsPanel {
                    workspace: workspace
                    SplitView.preferredHeight: 220
                }
                LayersPanel {
                    workspace: workspace
                    SplitView.fillHeight: true
                }
                FlowPanel {
                    workspace: workspace
                    SplitView.preferredHeight: 220
                }
            }

            SplitView {
                orientation: Qt.Vertical
                SplitView.fillWidth: true

                CanvasPanel {
                    workspace: workspace
                    SplitView.fillHeight: true
                }
                CodePanel {
                    workspace: workspace
                    SplitView.preferredHeight: 260
                }
            }

            SplitView {
                orientation: Qt.Vertical
                SplitView.preferredWidth: 300
                SplitView.minimumWidth: 200

                InspectorPanel {
                    workspace: workspace
                    SplitView.fillHeight: true
                }
                PalettePanel {
                    workspace: workspace
                    SplitView.preferredHeight: 200
                }
                TokensPanel {
                    workspace: workspace
                    SplitView.preferredHeight: 240
                }
            }
        }

        // The person's own last refusal, until the next gesture is answered.
        Label {
            objectName: "status"
            Accessible.role: Accessible.StaticText
            Accessible.name: "Status"
            padding: Spacing.xs
            text: workspace.error !== "" ? workspace.error : workspace.problem
            color: Colors.text
            visible: text !== ""
            Layout.fillWidth: true
        }
    }
}
