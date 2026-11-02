// Studio's quiet button: words on the panel, a fill under the pointer. An
// `accent` one is the single primary action of what it stands on.
import QtQuick
import QtQuick.Controls
import Ljos.Design

Button {
    id: button
    property bool accent: false
    implicitHeight: 28
    leftPadding: Spacing.xs
    rightPadding: Spacing.xs
    topPadding: 0
    bottomPadding: 0
    font.pixelSize: Typography.captionSize
    font.weight: Typography.bodyStrongWeight

    contentItem: Text {
        text: button.text
        font: button.font
        color: !button.enabled ? Colors.textDisabled
            : button.accent || button.checked ? Colors.textOnAccent : Colors.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        radius: 6
        color: button.accent || button.checked
            ? (button.down ? Colors.accentPressed : button.hovered ? Colors.accentHover : Colors.accent)
            : button.down ? Colors.surfacePressed
            : button.hovered ? Colors.surfaceHover : Colors.surfaceRaised
        border.width: button.visualFocus ? 2 : button.accent || button.checked ? 0 : 1
        border.color: button.visualFocus ? Colors.focusRing : Colors.surfacePressed
    }
}
