import QtQuick
import QtQuick.Controls
import Ljos.Design

Rectangle {
    id: screen
    property var model
    property var stack

    color: Colors.canvas

    Header {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        title: "Field notes"
        ToolButton {
            palette.buttonText: Colors.text
            text: "Settings"
            onClicked: screen.stack.push(Qt.resolvedUrl("SettingsScreen.qml"), { stack: screen.stack })
        }
    }

    ListView {
        id: list
        anchors { left: parent.left; right: parent.right; top: header.bottom; bottom: parent.bottom; margins: Spacing.md }
        spacing: Spacing.xs
        clip: true
        model: screen.model
        delegate: Rectangle {
            id: card
            required property int index
            required property string title
            required property string place
            required property string date
            required property string kind

            width: ListView.view.width
            height: Spacing.xxxl + Spacing.sm
            radius: Spacing.xs
            color: tap.pressed ? Colors.surfacePressed : Colors.surfaceRaised
            border.color: Colors.border

            Column {
                anchors { left: parent.left; right: chip.left; leftMargin: Spacing.md; rightMargin: Spacing.xs; verticalCenter: parent.verticalCenter }
                spacing: Spacing.xxs
                Text {
                    width: parent.width
                    text: card.title
                    elide: Text.ElideRight
                    color: Colors.text
                    font.pixelSize: Typography.bodyStrongSize
                    font.weight: Typography.bodyStrongWeight
                }
                Text {
                    text: card.place + " · " + card.date
                    color: Colors.textMuted
                    font.pixelSize: Typography.captionSize
                }
            }

            Rectangle {
                id: chip
                anchors { right: parent.right; rightMargin: Spacing.md; verticalCenter: parent.verticalCenter }
                width: kindLabel.implicitWidth + Spacing.md
                height: kindLabel.implicitHeight + Spacing.xxs
                radius: height / 2
                color: Colors.surfaceSelected
                Text {
                    id: kindLabel
                    anchors.centerIn: parent
                    text: card.kind
                    color: Colors.accent
                    font.pixelSize: Typography.captionSize
                }
            }

            TapHandler {
                id: tap
                onTapped: screen.stack.push(Qt.resolvedUrl("DetailScreen.qml"), { model: screen.model, index: card.index, stack: screen.stack })
            }
        }
    }

    Column {
        id: empty
        anchors.centerIn: list
        spacing: Spacing.xs
        opacity: 0
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "No notes yet"
            color: Colors.text
            font.pixelSize: Typography.subtitleSize
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "Tap + to write the first one."
            color: Colors.textMuted
            font.pixelSize: Typography.bodySize
        }
    }

    RoundButton {
        id: add
        anchors { right: parent.right; bottom: parent.bottom; margins: Spacing.lg }
        width: Spacing.xxxl
        height: Spacing.xxxl
        text: "+"
        palette.button: Colors.accent
        palette.buttonText: Colors.textOnAccent
        font.pixelSize: Typography.headlineSize
        onClicked: screen.stack.push(Qt.resolvedUrl("EditScreen.qml"), { model: screen.model, index: -1, stack: screen.stack })
    }

    states: [
        State {
            name: "empty"
            when: list.count === 0
            PropertyChanges { target: empty; opacity: 1 }
            PropertyChanges { target: list; opacity: 0 }
        }
    ]
    transitions: [
        Transition {
            NumberAnimation { property: "opacity"; duration: Motion.enterSmall }
        }
    ]
}
