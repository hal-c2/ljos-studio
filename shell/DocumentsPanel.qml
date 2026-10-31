// The project's documents; choosing one shows it everywhere.
import QtQuick
import QtQuick.Controls
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Documents"

    ListView {
        objectName: "documents"
        anchors.fill: parent
        clip: true
        model: panel.workspace.documents
        Accessible.role: Accessible.List
        Accessible.name: "Documents"

        delegate: ItemDelegate {
            required property string modelData
            width: ListView.view.width
            text: modelData
            Accessible.name: modelData
            highlighted: modelData === panel.workspace.doc
            onClicked: {
                panel.workspace.doc = modelData
                // The canvas shows what an agent's preview would.
                if (panel.workspace.previews)
                    panel.workspace.dispatch("preview_open", {doc: modelData})
            }
        }
    }
}
