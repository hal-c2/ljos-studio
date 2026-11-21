// A titled part of a panel that folds away: the chevron says which way, and
// the header is a button a reader can press. A hairline above parts it from
// the section before.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

ColumnLayout {
    id: section
    required property string title
    property bool open: true
    // Off for the first section of a panel, which has nothing above it.
    property bool ruled: true
    // How far the body sits in from the panel's edges; a list whose rows
    // carry their own padding takes less.
    property int inset: Spacing.sm
    default property alias content: body.data
    spacing: 0

    Rectangle {
        visible: section.ruled
        Layout.fillWidth: true
        implicitHeight: 1
        color: Colors.surfacePressed
        Accessible.ignored: true
    }
    ItemDelegate {
        id: header
        Layout.fillWidth: true
        implicitHeight: 32
        padding: 0
        leftPadding: Spacing.sm
        rightPadding: Spacing.xs
        hoverEnabled: true
        text: section.title
        Accessible.name: section.title + " section"
        Accessible.description: section.open ? "Open" : "Folded"
        onClicked: section.open = !section.open

        contentItem: RowLayout {
            spacing: Spacing.xxs
            Text {
                text: section.title
                color: Colors.text
                font.pixelSize: Typography.bodySize
                font.weight: Typography.bodyStrongWeight
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            Icon {
                name: section.open ? "chevronDown" : "chevronRight"
                color: header.hovered ? Colors.text : Colors.textMuted
            }
        }
        background: Rectangle {
            color: header.visualFocus ? Colors.surfaceHover : "transparent"
            border.width: header.visualFocus ? 2 : 0
            border.color: Colors.focusRing
        }
    }
    ColumnLayout {
        id: body
        visible: section.open
        spacing: Spacing.xs
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.leftMargin: section.inset
        Layout.rightMargin: section.inset
        Layout.bottomMargin: section.inset
    }
}
