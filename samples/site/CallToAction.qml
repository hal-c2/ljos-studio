import QtQuick
import Ljos.Design

Rectangle {
    id: cta
    property bool compact
    property real gutter

    width: parent.width
    height: body.height + 2 * Spacing.xxl
    color: Colors.accent

    Column {
        id: body
        anchors.centerIn: parent
        width: parent.width - 2 * cta.gutter
        spacing: Spacing.md

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: "Your next route is already on the map"
            color: Colors.textOnAccent
            font.pixelSize: cta.compact ? Typography.titleSize : Typography.headlineSize
            font.weight: Typography.headlineWeight
        }
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: downloadLabel.implicitWidth + Spacing.xxl
            height: Spacing.xxl
            radius: Spacing.xs
            color: Colors.surface
            Text {
                id: downloadLabel
                anchors.centerIn: parent
                text: "Download Varða"
                color: Colors.text
                font.pixelSize: Typography.subtitleSize
                font.weight: Typography.titleWeight
            }
        }
    }
}
