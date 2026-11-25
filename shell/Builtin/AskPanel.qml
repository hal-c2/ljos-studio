// A question with two or three answers, over the window: what to do with
// edits not saved, whether to remove a file. The asker keeps what to do on
// each answer; the panel only says which was picked.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Popup {
    id: ask
    objectName: "askPanel"
    property string question: ""
    property string detail: ""
    // The answer that goes ahead, and the one beside it; empty for none.
    property string first: ""
    property string second: ""
    property bool firstDanger: false
    signal answered(int which)

    parent: Overlay.overlay
    x: Math.round((parent.width - width) / 2)
    y: Math.round(parent.height * 0.2)
    width: Math.min(440, parent.width - 2 * Spacing.lg)
    modal: true
    focus: true
    padding: Spacing.md
    closePolicy: Popup.CloseOnEscape

    function show(question, detail, first, second, firstDanger) {
        ask.question = question
        ask.detail = detail
        ask.first = first
        ask.second = second || ""
        ask.firstDanger = firstDanger === true
        open()
        primary.forceActiveFocus()
    }
    // Cancelling is the answer 0, Escape too.
    onClosed: if (!picked) answered(0)
    property bool picked: false
    onAboutToShow: picked = false
    function pick(which) {
        picked = true
        close()
        answered(which)
    }

    Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, Colors.dark ? 0.5 : 0.25)
    }
    background: Rectangle {
        radius: 12
        color: Colors.surfaceRaised
        border.width: 1
        border.color: Colors.surfacePressed
    }

    // The popup is not an item; what it shows is, so the question is said
    // there, as the dialog a screen reader announces.
    contentItem: ColumnLayout {
        spacing: Spacing.sm
        Accessible.role: Accessible.Dialog
        Accessible.name: ask.question
        Label {
            text: ask.question
            wrapMode: Text.Wrap
            color: Colors.text
            font.pixelSize: Typography.subtitleSize
            font.weight: Typography.subtitleWeight
            Layout.fillWidth: true
        }
        Label {
            objectName: "askDetail"
            visible: text !== ""
            text: ask.detail
            wrapMode: Text.Wrap
            color: Colors.textMuted
            font.pixelSize: Typography.bodySize
            Layout.fillWidth: true
        }
        RowLayout {
            spacing: Spacing.xs
            Layout.fillWidth: true
            Item {
                Layout.fillWidth: true
            }
            TextButton {
                text: "Cancel"
                onClicked: ask.pick(0)
            }
            TextButton {
                visible: ask.second !== ""
                text: ask.second
                onClicked: ask.pick(2)
            }
            TextButton {
                id: primary
                text: ask.first
                accent: true
                onClicked: ask.pick(1)
                Keys.onReturnPressed: ask.pick(1)
            }
        }
    }
}
