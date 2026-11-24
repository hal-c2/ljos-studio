// Which of a sidebar's panels shows. The current tab is marked by a pill and
// by weight, so it does not rest on colour alone; the pill slides between
// tabs, and with reduced motion it is there at once while its colour fades.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Item {
    id: strip
    required property var tabs
    property int current: 0
    // Only a person's switch slides. Being laid out is no move, and sliding
    // then would have a window nobody touches ask for frames.
    property bool switched: false
    implicitHeight: 36

    // Under the current tab, behind its words.
    Rectangle {
        id: pill
        readonly property Item under: row.children[strip.current] || null
        x: row.x + (under ? under.x : 0)
        y: (strip.height - height) / 2
        width: under ? under.width : 0
        height: 26
        radius: 8
        color: Colors.surfaceHover
        visible: under !== null
        Behavior on x {
            enabled: strip.switched && !Motion.reduced
            NumberAnimation {
                duration: Motion.moveSmallFrequent
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Motion.easeInOut
            }
        }
        Behavior on width {
            enabled: strip.switched && !Motion.reduced
            NumberAnimation {
                duration: Motion.moveSmallFrequent
                easing.type: Easing.BezierSpline
                easing.bezierCurve: Motion.easeInOut
            }
        }
    }

    Row {
        id: row
        anchors.left: parent.left
        anchors.leftMargin: Spacing.xs
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Repeater {
            model: strip.tabs
            delegate: AbstractButton {
                id: tab
                required property string modelData
                required property int index
                readonly property bool shown: strip.current === index
                // Runs out on the first frame, when what its user set has arrived:
                // fades are for what changes after that.
                Timer {
                    id: opening
                    interval: 0
                    running: true
                }
                text: modelData
                checkable: true
                checked: shown
                hoverEnabled: true
                Accessible.name: text + " tab"
                Accessible.role: Accessible.PageTab
                // The whole strip takes the press; the pill is only what is drawn.
                implicitHeight: 36
                implicitWidth: label.implicitWidth + 2 * Spacing.sm
                onClicked: {
                    strip.switched = true
                    strip.current = index
                    // A click toggles a checkable button and ends its binding.
                    checked = Qt.binding(() => shown)
                }

                background: Item {
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        height: 26
                        radius: 8
                        color: Colors.surfaceHover
                        opacity: tab.hovered && !tab.shown ? 0.6 : 0
                        Behavior on opacity {
                            // Not into what it opens as: nothing has moved then.
                            enabled: !opening.running
                            NumberAnimation {
                                duration: Motion.enterSmallFrequent
                                easing.type: Easing.BezierSpline
                                easing.bezierCurve: Motion.easeOut
                            }
                        }
                    }
                    // Apart from the hover fill, whose opacity would hide it
                    // on the current tab and on one not hovered.
                    Rectangle {
                        objectName: "focusRing"
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        height: 26
                        radius: 8
                        color: "transparent"
                        border.width: 2
                        border.color: Colors.focusRing
                        visible: tab.visualFocus
                    }
                }
                contentItem: Text {
                    id: label
                    text: tab.text
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    color: tab.shown || tab.hovered ? Colors.text : Colors.textMuted
                    font.pixelSize: Typography.bodySize
                    font.weight: tab.shown ? Typography.bodyStrongWeight : Typography.bodyWeight
                }
            }
        }
    }
}
