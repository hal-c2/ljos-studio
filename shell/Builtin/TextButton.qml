// Studio's quiet button: words on a soft fill that deepens under the pointer.
// An `accent` one is the single primary action of what it stands on. Give it
// `icon_` for a mark before the words.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Button {
    id: button
    property bool accent: false
    property string icon_: ""
    implicitHeight: 28
    leftPadding: icon_ === "" ? Spacing.sm : Spacing.xs
    rightPadding: Spacing.sm
    topPadding: 0
    bottomPadding: 0
    hoverEnabled: true
    font.pixelSize: Typography.bodySize
    font.weight: Typography.bodyStrongWeight

    readonly property color ink: !button.enabled ? Colors.textDisabled
        : button.accent || button.checked ? Colors.textOnAccent : Colors.text

    contentItem: RowLayout {
        spacing: Spacing.xxs
        Icon {
            visible: button.icon_ !== ""
            name: button.icon_
            color: button.ink
        }
        Text {
            text: button.text
            font: button.font
            color: button.ink
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
    }
    background: Rectangle {
        radius: 6
        color: button.accent || button.checked
            ? (button.down ? Colors.accentPressed : button.hovered ? Colors.accentHover : Colors.accent)
            : button.down ? Colors.surfacePressed
            : button.hovered ? Colors.surfacePressed : Colors.surfaceHover
        border.width: button.visualFocus ? 2 : 0
        border.color: Colors.focusRing
        Behavior on color {
            ColorAnimation {
                duration: Motion.enterSmallFrequent
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Motion.easeOut
            }
        }
    }
}
