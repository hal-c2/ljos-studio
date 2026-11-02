// Studio's text field: quiet at rest, its edge the border role under the
// pointer and the focus ring while typing.
import QtQuick
import QtQuick.Controls
import Ljos.Design

TextField {
    id: field
    implicitHeight: 28
    leftPadding: Spacing.xs
    rightPadding: Spacing.xs
    topPadding: 0
    bottomPadding: 0
    verticalAlignment: TextInput.AlignVCenter
    font.pixelSize: Typography.captionSize
    color: enabled ? Colors.text : Colors.textDisabled
    placeholderTextColor: Colors.textMuted
    selectByMouse: true

    background: Rectangle {
        radius: 6
        color: field.enabled ? Colors.surfaceRaised : Colors.surface
        border.width: field.activeFocus ? 2 : 1
        border.color: field.activeFocus ? Colors.focusRing
            : field.hovered ? Colors.border : Colors.surfacePressed
    }
}
