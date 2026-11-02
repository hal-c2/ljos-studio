import QtQuick
import Ljos.Design

Rectangle {
    id: footer
    property bool compact
    property real gutter

    width: parent.width
    height: Spacing.xxxl + Spacing.lg
    color: Colors.surface

    Text {
        anchors { left: parent.left; leftMargin: footer.gutter; verticalCenter: parent.verticalCenter }
        text: "© 2026 Varða"
        color: Colors.textMuted
        font.pixelSize: Typography.captionSize
    }
    Text {
        anchors { right: parent.right; rightMargin: footer.gutter; verticalCenter: parent.verticalCenter }
        text: footer.compact ? "Contact" : "Contact · Privacy · Press"
        color: Colors.link
        font.pixelSize: Typography.captionSize
    }
}
