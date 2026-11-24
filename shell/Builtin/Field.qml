// Studio's text field: a soft well at rest, its edge the border role under
// the pointer and the focus ring while typing. A `lead` names it inside its
// left edge ("X", "W"); with `scrub` a drag across the lead changes the number
// and lets go as Enter does. A `unit` sits at its right; `bound` marks what it
// holds as an expression with an accent ƒ, apart from a literal; `token` shows
// a design role it names as a chip. Anything beside the field may scrub it too,
// through scrubStart, scrubTo and scrubEnd. Left alone, a value too long for
// it shows its start and its end, cut in the middle: an expression's last
// term is as often what tells it apart as its first. Typed in, a narrow bound
// one gives its marks' room to the value, which scrolls with the caret.
import QtQuick
import QtQuick.Controls
import Ljos.Design

TextField {
    id: field
    property string lead: ""
    property string unit: ""
    property bool bound: false
    property bool token: false
    // An icon at the left edge, before the lead: a search field's glass.
    property string leadIcon: ""
    property bool scrub: false
    // Pixels a scrub moves per unit of the number.
    property real scrubStep: 1
    // Runs out on the first frame, when what its user set has arrived:
    // fades are for what changes after that.
    Timer {
        id: opening
        interval: 0
        running: true
    }

    property real scrubFrom: NaN
    property bool scrubbed: false
    function scrubStart() {
        // An empty field scrubs from what it shows in its place.
        scrubFrom = Number(text !== "" ? text : placeholderText)
        scrubbed = false
    }
    // `dx` is how far the pointer has gone since the start, in pixels.
    function scrubTo(dx) {
        if (isNaN(scrubFrom))
            return
        const by = Math.round(dx / scrubStep)
        if (by !== 0)
            scrubbed = true
        text = String(scrubFrom + by)
    }
    function scrubEnd() {
        if (scrubbed && !isNaN(scrubFrom))
            accepted()
        scrubFrom = NaN
    }

    implicitHeight: 28
    // Beside the pin pad a bound field's marks left its value two pixels. A
    // literal keeps them: its lead is the handle it is scrubbed by.
    readonly property bool cramped: bound && activeFocus && width < 80
    leftPadding: Spacing.xs + (leadBox.visible ? leadBox.width + 2 : 0)
    rightPadding: Spacing.xs + (unitText.visible ? unitText.implicitWidth + Spacing.xxs : 0)
    topPadding: 0
    bottomPadding: 0
    verticalAlignment: TextInput.AlignVCenter
    font.pixelSize: Typography.bodySize
    readonly property color ink: !enabled ? Colors.textDisabled : bound || token ? Colors.link : Colors.text
    // Not `cut`: that is the field's own method, and Ljós reads the name as
    // the method, not as a property declared over it. A method is true.
    readonly property bool shortened: !activeFocus && text !== ""
        && contentWidth > width - leftPadding - rightPadding
    // The shortened copy below draws it while it is; the field's own text is
    // still there for the caret, a selection and a reader.
    color: shortened ? "transparent" : ink
    placeholderTextColor: Colors.textMuted
    selectionColor: Colors.surfaceSelected
    selectedTextColor: Colors.text
    selectByMouse: true
    hoverEnabled: true
    // Left alone, a field shows how its text starts, not how it ends.
    onTextChanged: if (!activeFocus) cursorPosition = 0
    onActiveFocusChanged: if (!activeFocus) cursorPosition = 0

    // The lead: its words, and the ƒ of a binding after them; a handle to
    // scrub with.
    Item {
        id: leadBox
        visible: !field.cramped && (field.lead !== "" || field.bound || field.leadIcon !== "")
        x: Spacing.xxs
        anchors.verticalCenter: parent.verticalCenter
        width: leadRow.implicitWidth
        height: 20
        Accessible.ignored: true
        Row {
            id: leadRow
            height: parent.height
            spacing: 2
            Icon {
                visible: field.leadIcon !== ""
                anchors.verticalCenter: parent.verticalCenter
                name: field.leadIcon !== "" ? field.leadIcon : "search"
                size: 14
                color: Colors.textMuted
            }
            Text {
                visible: field.lead !== ""
                width: Math.max(14, implicitWidth + 4)
                height: parent.height
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: field.lead
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
                elide: Text.ElideRight
            }
            Rectangle {
                visible: field.bound
                width: 18
                height: parent.height
                radius: 4
                color: Colors.surfaceSelected
                Text {
                    anchors.centerIn: parent
                    text: "ƒ"
                    color: Colors.link
                    font.pixelSize: Typography.captionSize
                    font.weight: Typography.bodyStrongWeight
                    font.italic: true
                }
            }
        }
        MouseArea {
            id: scrubber
            objectName: "scrubber"
            anchors.fill: parent
            enabled: field.scrub && !field.bound && !field.token && field.enabled
            cursorShape: enabled ? Qt.SizeHorCursor : Qt.IBeamCursor
            property real startX: 0
            onPressed: mouse => {
                startX = mapToItem(null, mouse.x, 0).x
                field.scrubStart()
            }
            onPositionChanged: mouse => field.scrubTo(mapToItem(null, mouse.x, 0).x - startX)
            onReleased: field.scrubEnd()
            // The window took the press (a popup, the pointer gone): what the
            // scrub showed is kept, as letting go keeps it.
            onCanceled: field.scrubEnd()
        }
    }
    Text {
        visible: field.shortened
        x: field.leftPadding
        width: field.width - field.leftPadding - field.rightPadding
        anchors.verticalCenter: parent.verticalCenter
        text: field.text
        elide: Text.ElideMiddle
        font: field.font
        color: field.ink
        Accessible.ignored: true
    }
    Text {
        id: unitText
        visible: !field.cramped && field.unit !== ""
        anchors.right: parent.right
        anchors.rightMargin: Spacing.xs
        anchors.verticalCenter: parent.verticalCenter
        text: field.unit
        color: Colors.textMuted
        font.pixelSize: Typography.captionSize
        Accessible.ignored: true
    }

    background: Rectangle {
        // A token is a chip: rounder, and on the selected tint the link holds.
        radius: field.token && !field.activeFocus ? height / 2 : 6
        color: field.activeFocus ? Colors.surfaceRaised
            : field.token ? Colors.surfaceSelected
            : field.enabled ? Colors.surfaceHover : "transparent"
        border.width: field.activeFocus ? 2 : 1
        border.color: field.activeFocus ? Colors.focusRing
            : field.hovered ? Colors.border : "transparent"
        Behavior on border.color {
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
