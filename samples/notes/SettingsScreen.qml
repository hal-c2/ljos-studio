import QtQuick
import QtQuick.Controls
import Ljos.Design

Rectangle {
    id: screen
    property var stack

    color: Colors.canvas

    Header {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        title: "Settings"
        back: true
        onBackTapped: screen.stack.pop()
    }

    Column {
        anchors { left: parent.left; right: parent.right; top: header.bottom; margins: Spacing.lg }
        spacing: Spacing.md

        Rectangle {
            width: parent.width
            height: Spacing.xxxl
            radius: Spacing.xs
            color: Colors.surfaceRaised
            border.color: Colors.border
            Text {
                anchors { left: parent.left; leftMargin: Spacing.md; verticalCenter: parent.verticalCenter }
                text: "Dark theme"
                color: Colors.text
                font.pixelSize: Typography.bodySize
            }
            Switch {
                id: darkSwitch
                anchors { right: parent.right; rightMargin: Spacing.xs; verticalCenter: parent.verticalCenter }
                checked: Colors.dark
                onToggled: Colors.dark = checked
            }
        }

        Text {
            width: parent.width
            text: "Notes are read from entries.json when the app starts."
            wrapMode: Text.Wrap
            color: Colors.textMuted
            font.pixelSize: Typography.captionSize
        }
    }
}
