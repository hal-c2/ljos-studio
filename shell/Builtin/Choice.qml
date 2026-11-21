// A box to pick one of a list in, quiet as a field: a muted word for what
// it picks and an optional mark before the pick, a chevron after it. The
// list it opens is the style's own.
import QtQuick
import QtQuick.Controls
import Ljos.Design

ComboBox {
    id: box
    // What it picks, muted before the pick: "Wire", "Entry".
    property string lead: ""
    property string leadIcon: ""

    implicitHeight: 28
    hoverEnabled: true
    leftPadding: Spacing.xs
    rightPadding: Spacing.lg

    contentItem: Row {
        spacing: Spacing.xxs
        clip: true
        Icon {
            visible: box.leadIcon !== ""
            name: box.leadIcon !== "" ? box.leadIcon : "pointer"
            size: 14
            color: Colors.textMuted
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            visible: box.lead !== ""
            text: box.lead
            color: Colors.textMuted
            font.pixelSize: Typography.captionSize
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            text: box.displayText
            color: box.enabled ? Colors.text : Colors.textDisabled
            font.pixelSize: Typography.bodySize
            elide: Text.ElideRight
            anchors.verticalCenter: parent.verticalCenter
            Accessible.ignored: true
        }
    }
    indicator: Icon {
        name: "chevronDown"
        size: 14
        color: Colors.textMuted
        x: box.width - width - Spacing.xs
        y: (box.height - height) / 2
    }
    background: Rectangle {
        radius: 6
        color: box.down || box.hovered ? Colors.surfacePressed : Colors.surfaceHover
        border.width: box.visualFocus ? 2 : 0
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
