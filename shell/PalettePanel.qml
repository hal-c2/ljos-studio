// The project's blocks. "Place" puts one into the selected object.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Palette"

    ListView {
        objectName: "palette"
        anchors.fill: parent
        clip: true
        model: panel.workspace.palette
        Accessible.role: Accessible.List
        Accessible.name: "Palette"

        delegate: RowLayout {
            id: row
            required property var modelData
            width: ListView.view.width
            spacing: Spacing.xxs

            Label {
                text: row.modelData.name
                color: Colors.text
                Accessible.role: Accessible.StaticText
                Accessible.name: row.modelData.name
                Accessible.description: row.modelData.summary
                Layout.fillWidth: true
                leftPadding: Spacing.xs
            }
            Button {
                text: "Place"
                Accessible.name: "Place " + row.modelData.name
                enabled: panel.workspace.selected !== ""
                onClicked: panel.workspace.dispatch("node_insert", {
                    doc: panel.workspace.doc,
                    parent: {node: panel.workspace.selected},
                    block: row.modelData.module + "." + row.modelData.name
                })
            }
        }
    }
}
