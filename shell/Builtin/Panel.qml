// A pane of the shell. Its title is left to whoever shows it, a tab or a
// header, unless `headed`; a reader always hears it.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Pane {
    id: panel
    required property string title
    property bool headed: false
    property color fill: Colors.surface
    default property alias content: body.data

    padding: 0
    Accessible.role: Accessible.Pane
    Accessible.name: title
    background: Rectangle {
        color: panel.fill
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Label {
            visible: panel.headed
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
