import QtQuick
import Ljos.Design

Rectangle {
    id: bar
    property bool compact
    property real gutter

    width: parent.width
    height: Spacing.xxxl
    color: Colors.surface

    Text {
        anchors { left: parent.left; leftMargin: bar.gutter; verticalCenter: parent.verticalCenter }
        text: "Varða"
        color: Colors.text
        font.pixelSize: Typography.titleSize
        font.weight: Typography.titleWeight
    }

    Row {
        anchors { right: parent.right; rightMargin: bar.gutter; verticalCenter: parent.verticalCenter }
        spacing: Spacing.lg

        Repeater {
            model: bar.compact ? [] : ["Features", "Pricing", "Stories"]
            Text {
                required property string modelData
                anchors.verticalCenter: parent.verticalCenter
                text: modelData
                color: Colors.textMuted
                font.pixelSize: Typography.bodySize
            }
        }

        Rectangle {
            width: getLabel.implicitWidth + Spacing.lg
            height: Spacing.xl
            radius: Spacing.xxs
            color: Colors.accent
            Text {
                id: getLabel
                anchors.centerIn: parent
                text: "Get the app"
                color: Colors.textOnAccent
                font.pixelSize: Typography.bodyStrongSize
                font.weight: Typography.bodyStrongWeight
            }
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: Colors.border
    }
}
