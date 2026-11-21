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
    implicitHeight: 40

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
            enabled: !Motion.reduced
            NumberAnimation { duration: Motion.moveSmallFrequent; easing.type: Easing.BezierSpline; easing.bezierCurve: Motion.easeInOut }
        }
        Behavior on width {
            enabled: !Motion.reduced
            NumberAnimation { duration: Motion.moveSmallFrequent; easing.type: Easing.BezierSpline; easing.bezierCurve: Motion.easeInOut }
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
                text: modelData
                checkable: true
                checked: shown
                hoverEnabled: true
                Accessible.name: text + " tab"
                Accessible.role: Accessible.PageTab
                implicitHeight: 26
                implicitWidth: label.implicitWidth + 2 * Spacing.sm
                onClicked: {
                    strip.current = index
                    // A click toggles a checkable button and ends its binding.
                    checked = Qt.binding(() => shown)
                }

                background: Rectangle {
                    radius: 8
                    color: Colors.surfaceHover
                    opacity: tab.hovered && !tab.shown ? 0.6 : 0
                    border.width: tab.visualFocus ? 2 : 0
                    border.color: Colors.focusRing
                    Behavior on opacity {
                        NumberAnimation { duration: Motion.enterSmallFrequent; easing.type: Easing.BezierSpline; easing.bezierCurve: Motion.easeOut }
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
