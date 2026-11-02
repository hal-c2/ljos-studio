import QtQuick
import QtQuick.Controls
import Ljos.Design

Rectangle {
    id: screen
    property var model
    property int index
    property var stack
    readonly property var entry: model.get(index)

    color: Colors.canvas

    Header {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        title: "Note"
        back: true
        onBackTapped: screen.stack.pop()
        ToolButton {
            text: "Edit"
            onClicked: screen.stack.push(Qt.resolvedUrl("EditScreen.qml"), { model: screen.model, index: screen.index, stack: screen.stack })
        }
    }

    Column {
        anchors { left: parent.left; right: parent.right; top: header.bottom; margins: Spacing.lg }
        spacing: Spacing.sm

        Text {
            text: screen.entry.kind
            color: Colors.accent
            font.pixelSize: Typography.captionSize
            font.weight: Typography.bodyStrongWeight
        }
        Text {
            width: parent.width
            text: screen.entry.title
            wrapMode: Text.Wrap
            color: Colors.text
            font.pixelSize: Typography.headlineSize
            font.weight: Typography.headlineWeight
        }
        Text {
            text: screen.entry.place + " · " + screen.entry.date
            color: Colors.textMuted
            font.pixelSize: Typography.bodySize
        }
        Rectangle {
            width: parent.width
            height: 1
            color: Colors.border
        }
        Text {
            width: parent.width
            text: screen.entry.note
            wrapMode: Text.Wrap
            color: Colors.text
            font.pixelSize: Typography.bodySize
            lineHeight: Typography.bodyLineHeight
            lineHeightMode: Text.FixedHeight
        }
    }

    Button {
        anchors { left: parent.left; bottom: parent.bottom; margins: Spacing.lg }
        text: "Delete"
        onClicked: {
            screen.model.remove(screen.index)
            screen.stack.pop()
        }
    }
}
