// Which of a sidebar's panels shows. The current tab is marked by a bar and
// by weight, so it does not rest on colour alone.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Rectangle {
    id: strip
    required property var tabs
    property int current: 0
    implicitHeight: 36
    color: Colors.surface

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Spacing.xxs
        anchors.rightMargin: Spacing.xxs
        spacing: 0

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
                Accessible.name: text + " tab"
                Accessible.role: Accessible.PageTab
                implicitHeight: 36
                implicitWidth: label.implicitWidth + 2 * Spacing.xs
                onClicked: strip.current = index

                Text {
                    id: label
                    anchors.centerIn: parent
                    text: tab.text
                    color: tab.shown || tab.hovered ? Colors.text : Colors.textMuted
                    font.pixelSize: Typography.captionSize
                    font.weight: tab.shown ? Typography.bodyStrongWeight : Typography.bodyWeight
                }
                Rectangle {
                    visible: tab.shown
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: label.implicitWidth
                    height: 2
                    color: Colors.accent
                }
            }
        }
        Item {
            Layout.fillWidth: true
        }
    }
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: Colors.surfacePressed
    }
}
