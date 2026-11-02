// A group of tools floating over the canvas, opaque so what is under it does
// not fight its icons.
import QtQuick
import Ljos.Design

Rectangle {
    default property alias content: row.data
    implicitWidth: row.implicitWidth + 2 * Spacing.xxs
    implicitHeight: 36
    radius: 8
    color: Colors.surfaceRaised
    border.width: 1
    border.color: Colors.border

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2
    }
}
