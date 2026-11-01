// A titled part of a panel that folds away: the chevron says which way, and
// the header is a button a reader can press.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

ColumnLayout {
    id: section
    required property string title
    property bool open: true
    default property alias content: body.data
    spacing: 0

    ItemDelegate {
        id: header
        Layout.fillWidth: true
        implicitHeight: 28
        padding: 0
        leftPadding: Spacing.xs
        text: section.title
        Accessible.name: section.title + " section"
        Accessible.description: section.open ? "Open" : "Folded"
        onClicked: section.open = !section.open

        contentItem: RowLayout {
            spacing: Spacing.xxs
            Icon {
                name: section.open ? "chevronDown" : "chevronRight"
                color: Colors.textMuted
            }
            Text {
                text: section.title
                color: Colors.text
                font.pixelSize: Typography.captionSize
                font.weight: Typography.bodyStrongWeight
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
        }
        background: Rectangle {
            color: header.hovered ? Colors.surfaceHover : "transparent"
        }
    }
    ColumnLayout {
        id: body
        visible: section.open
        spacing: Spacing.xxs
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.leftMargin: Spacing.xs
        Layout.rightMargin: Spacing.xs
        Layout.bottomMargin: Spacing.xs
    }
}
