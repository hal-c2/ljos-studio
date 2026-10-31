import QtQuick
import QtQuick.Controls
import Ljos.Design

Rectangle {
    id: header
    property string title
    property bool back: false
    signal backTapped
    default property alias actions: actionRow.data

    height: Spacing.xxxl
    color: Colors.surface

    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 1
        color: Colors.border
    }

    ToolButton {
        palette.buttonText: Colors.text
        id: backButton
        visible: header.back
        anchors { left: parent.left; leftMargin: Spacing.xs; verticalCenter: parent.verticalCenter }
        text: "‹ Back"
        onClicked: header.backTapped()
    }

    Text {
        anchors { left: header.back ? backButton.right : parent.left; leftMargin: Spacing.md; verticalCenter: parent.verticalCenter }
        text: header.title
        color: Colors.text
        font.pixelSize: Typography.titleSize
        font.weight: Typography.titleWeight
    }

    Row {
        id: actionRow
        anchors { right: parent.right; rightMargin: Spacing.xs; verticalCenter: parent.verticalCenter }
        spacing: Spacing.xxs
    }
}
