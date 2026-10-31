import QtQuick
import Ljos.Design

Rectangle {
    id: hero
    property bool compact
    property real gutter

    width: parent.width
    height: copy.height + 2 * (compact ? Spacing.xxl : Spacing.xxxl * 2)
    color: Colors.surfaceSelected

    Column {
        id: copy
        anchors.centerIn: parent
        width: Math.min(parent.width - 2 * hero.gutter, 720)
        spacing: Spacing.lg

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: "Trail maps that keep working when the signal stops"
            color: Colors.text
            font.pixelSize: hero.compact ? Typography.headlineSize : Typography.displaySize
            font.weight: Typography.displayWeight
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: "Varða keeps every route, cairn and hut on your phone, so the way back is there when the bars are not."
            color: Colors.textMuted
            font.pixelSize: Typography.subtitleSize
        }
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: startLabel.implicitWidth + Spacing.xxl
            height: Spacing.xxl
            radius: Spacing.xs
            color: Colors.accent
            Text {
                id: startLabel
                anchors.centerIn: parent
                text: "Start free"
                color: Colors.textOnAccent
                font.pixelSize: Typography.subtitleSize
                font.weight: Typography.titleWeight
            }
        }
    }
}
