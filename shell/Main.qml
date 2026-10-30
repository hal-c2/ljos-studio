// Studio's default shell: panels that skin the one block Studio binds,
// Studio.Workspace. Every gesture is a `dispatch` of a Studio tool, the
// call an agent makes over MCP.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

ApplicationWindow {
    id: window
    title: workspace.doc === "" ? "Ljós Studio" : workspace.doc + " · Ljós Studio"
    width: 1440
    height: 900
    visible: true
    color: Colors.canvas

    Workspace {
        id: workspace
    }

    // The person's own last refusal, until the next gesture is answered.
    footer: Label {
        objectName: "status"
        Accessible.role: Accessible.StaticText
        Accessible.name: "Status"
        padding: Spacing.xs
        text: workspace.error !== "" ? workspace.error : workspace.problem
        color: Colors.text
        visible: text !== ""
    }

    header: Toolbar {
        workspace: workspace
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        onActivated: workspace.dispatch("history", {action: "undo"})
    }
    Shortcut {
        sequences: [StandardKey.Redo]
        onActivated: workspace.dispatch("history", {action: "redo"})
    }

    SplitView {
        anchors.fill: parent
        orientation: Qt.Horizontal

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
                SplitView.preferredHeight: 260
            }
        }
    }
}
