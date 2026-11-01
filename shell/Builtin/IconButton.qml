// A button that shows only an icon: `text` is its name for a reader and its
// tooltip for the eye. Checked is a fill and a coloured icon, quiet enough to
// leave the accent's own fill to the one primary action.
import QtQuick
import QtQuick.Controls
import Ljos.Design

ToolButton {
    id: button
    required property string icon_
    display: AbstractButton.IconOnly
    implicitWidth: 28
    implicitHeight: 28
    padding: 0
    Accessible.name: text

    // Below, not the attached tip's place above: along the top of the window
    // there is no room above, and a tip kept inside the window would lie on
    // the button and take its press.
    ToolTip {
        parent: button
        visible: button.hovered && button.text !== ""
        text: button.text
        delay: 500
        y: button.height + 4
    }

    contentItem: Item {
        Icon {
            anchors.centerIn: parent
            name: button.icon_
            color: !button.enabled ? Colors.textDisabled
                : button.checked ? Colors.accent : Colors.text
        }
    }
    background: Rectangle {
        radius: 6
        color: button.checked ? Colors.surfaceSelected
            : button.down ? Colors.surfacePressed
            : button.hovered ? Colors.surfaceHover : "transparent"
        border.width: button.visualFocus ? 2 : 0
        border.color: Colors.focusRing
    }
}
