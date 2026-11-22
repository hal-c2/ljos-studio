// A button that shows only an icon: `text` is its name for a reader and its
// tooltip for the eye, with `shortcut` beside it when there is one. Checked
// is a tinted fill and an accent icon, quiet enough to leave the accent's
// own fill to the one primary action.
import QtQuick
import QtQuick.Controls
import Ljos.Design

ToolButton {
    id: button
    required property string icon_
    // The keys that do the same, as the tooltip shows them: "Ctrl+Z".
    property string shortcut: ""
    property real iconSize: 16
    // Draws no icon and still takes the press: an item at opacity 0 takes
    // none, so a finger, which never hovers, could not reach it.
    property bool quiet: false
    // Runs out on the first frame, when what its user set has arrived:
    // fades are for what changes after that.
    Timer {
        id: opening
        interval: 0
        running: true
    }
    display: AbstractButton.IconOnly
    implicitWidth: 28
    implicitHeight: 28
    padding: 0
    hoverEnabled: true
    Accessible.name: text
    Accessible.description: shortcut

    // Below, not the attached tip's place above: along the top of the window
    // there is no room above, and a tip kept inside the window would lie on
    // the button and take its press.
    ToolTip {
        id: tip
        parent: button
        visible: button.hovered && button.text !== ""
        text: button.shortcut === "" ? button.text : button.text + "   " + button.shortcut
        delay: 500
        y: button.height + 6
        padding: 0
        contentItem: Row {
            spacing: Spacing.xs
            leftPadding: Spacing.xs
            rightPadding: Spacing.xs
            topPadding: Spacing.xxs
            bottomPadding: Spacing.xxs
            Text {
                text: button.text
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Text {
                visible: button.shortcut !== ""
                text: button.shortcut
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
            }
        }
        background: Rectangle {
            radius: 6
            color: Colors.surfaceRaised
            border.width: 1
            border.color: Colors.surfacePressed
        }
    }

    contentItem: Item {
        opacity: button.quiet && !button.visualFocus ? 0 : 1
        Icon {
            anchors.centerIn: parent
            size: button.iconSize
            name: button.icon_
            // The accent as a mark on a tinted fill: `link`, which the theme
            // keeps at text contrast on every ground, where `accent` is a fill.
            color: !button.enabled ? Colors.textDisabled
                : button.checked ? Colors.link : Colors.text
        }
    }
    background: Rectangle {
        radius: 6
        color: button.checked ? Colors.surfaceSelected
            : button.down ? Colors.surfacePressed
            : button.hovered ? Colors.surfaceHover : "transparent"
        border.width: button.visualFocus ? 2 : 0
        border.color: Colors.focusRing
        Behavior on color {
            // Not into what it opens as: nothing has moved then.
            enabled: !opening.running
            ColorAnimation {
                duration: Motion.enterSmallFrequent
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Motion.easeOut
            }
        }
    }
}
