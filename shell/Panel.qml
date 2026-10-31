// A titled pane of the shell.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Pane {
    id: panel
    required property string title
    default property alias content: body.data

    padding: 0
    Accessible.role: Accessible.Pane
    Accessible.name: title

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Label {
            text: panel.title
            font.pixelSize: Typography.captionSize
            font.weight: Typography.bodyStrongWeight
            color: Colors.text
            padding: Spacing.xs
            Layout.fillWidth: true
        }
        Item {
            id: body
            Layout.fillWidth: true
            Layout.fillHeight: true
        }
    }
}
