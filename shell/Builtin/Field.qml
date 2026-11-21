// Studio's text field: a soft well at rest, its edge the border role under
// the pointer and the focus ring while typing. A `lead` names it inside its
// left edge ("X", "W"); with `scrub` a drag across the lead changes the number
// and lets go as Enter does. A `unit` sits at its right; `bound` marks what it
// holds as an expression with an accent ƒ, apart from a literal.
import QtQuick
import QtQuick.Controls
import Ljos.Design

TextField {
    id: field
    property string lead: ""
    property string unit: ""
    property bool bound: false
    property bool scrub: false
    // Pixels a scrub moves per unit of the number.
    property real scrubStep: 1

    implicitHeight: 28
    leftPadding: Spacing.xs + (leadBox.visible ? leadBox.width + 2 : 0)
    rightPadding: Spacing.xs + (unitText.visible ? unitText.implicitWidth + Spacing.xxs : 0)
    topPadding: 0
    bottomPadding: 0
    verticalAlignment: TextInput.AlignVCenter
    font.pixelSize: Typography.bodySize
    color: !enabled ? Colors.textDisabled : bound ? Colors.link : Colors.text
    placeholderTextColor: Colors.textMuted
    selectionColor: Colors.surfaceSelected
    selectedTextColor: Colors.text
    selectByMouse: true
    hoverEnabled: true

    // The lead: words, or the ƒ of a binding; a handle to scrub with.
    Rectangle {
        id: leadBox
        visible: field.lead !== "" || field.bound
        x: Spacing.xxs
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(18, leadText.implicitWidth + 8)
        height: 20
        radius: 4
        color: field.bound ? Colors.surfaceSelected : "transparent"
        Accessible.ignored: true
        Text {
            id: leadText
            anchors.centerIn: parent
            text: field.bound ? "ƒ" : field.lead
            color: field.bound ? Colors.link : Colors.textMuted
            font.pixelSize: Typography.captionSize
            font.weight: field.bound ? Typography.bodyStrongWeight : Typography.bodyWeight
            font.italic: field.bound
        }
        MouseArea {
            id: scrubber
            anchors.fill: parent
            enabled: field.scrub && !field.bound && field.enabled
            cursorShape: enabled ? Qt.SizeHorCursor : Qt.IBeamCursor
            property real from: 0
            property real startX: 0
            property bool moved: false
            onPressed: mouse => {
                from = Number(field.text)
                startX = mapToItem(null, mouse.x, 0).x
                moved = false
            }
            onPositionChanged: mouse => {
                if (isNaN(from))
                    return
                const by = Math.round((mapToItem(null, mouse.x, 0).x - startX) / field.scrubStep)
                if (by !== 0)
                    moved = true
                field.text = String(from + by)
            }
            onReleased: if (moved) field.accepted()
        }
    }
    Text {
        id: unitText
        visible: field.unit !== ""
        anchors.right: parent.right
        anchors.rightMargin: Spacing.xs
        anchors.verticalCenter: parent.verticalCenter
        text: field.unit
        color: Colors.textMuted
        font.pixelSize: Typography.captionSize
        Accessible.ignored: true
    }

    background: Rectangle {
        radius: 6
        color: field.activeFocus ? Colors.surfaceRaised
            : field.enabled ? Colors.surfaceHover : "transparent"
        border.width: field.activeFocus ? 2 : 1
        border.color: field.activeFocus ? Colors.focusRing
            : field.hovered ? Colors.border : "transparent"
        Behavior on border.color {
            ColorAnimation { duration: Motion.enterSmallFrequent; easing.type: Easing.BezierSpline; easing.bezierCurve: Motion.easeOut }
        }
    }
}
