import QtQuick
import QtQuick.Controls
import Ljos.Design

Rectangle {
    id: screen
    property var model
    property int index: -1
    property var stack
    readonly property var entry: index >= 0 ? model.get(index) : ({ title: "", place: "", date: "", kind: "Bird", note: "" })
    readonly property bool valid: titleField.text.length > 0 && placeField.text.length > 0 && dateField.acceptableInput

    color: Colors.canvas

    function save() {
        const record = { title: titleField.text, place: placeField.text, date: dateField.text, kind: kindBox.currentText, note: noteArea.text }
        if (index >= 0)
            model.set(index, record)
        else
            model.insert(0, record)
        stack.pop()
    }

    Header {
        id: header
        anchors { left: parent.left; right: parent.right; top: parent.top }
        title: "New note"
        back: true
        onBackTapped: screen.stack.pop()
        ToolButton {
            id: saveButton
            text: "Save"
            enabled: screen.valid
            onClicked: screen.save()
        }
    }

    Column {
        anchors { left: parent.left; right: parent.right; top: header.bottom; margins: Spacing.lg }
        spacing: Spacing.xs

        Label { text: "Title"; color: Colors.textMuted }
        TextField {
            id: titleField
            width: parent.width
            text: screen.entry.title
            placeholderText: "What did you see?"
        }
        Label {
            id: titleError
            text: "A note needs a title."
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            visible: titleField.text.length === 0
        }

        Label { text: "Place"; color: Colors.textMuted }
        TextField {
            id: placeField
            width: parent.width
            text: screen.entry.place
            placeholderText: "Where?"
        }

        Label { text: "Date"; color: Colors.textMuted }
        TextField {
            id: dateField
            width: parent.width
            text: screen.entry.date
            placeholderText: "YYYY-MM-DD"
            validator: RegularExpressionValidator { regularExpression: /\d{4}-\d{2}-\d{2}/ }
        }
        Label {
            id: dateError
            text: "Write the date as YYYY-MM-DD."
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            visible: !dateField.acceptableInput
        }

        Label { text: "Kind"; color: Colors.textMuted }
        ComboBox {
            id: kindBox
            width: parent.width
            model: ["Bird", "Mammal", "Plant", "Fungus", "Weather"]
            currentIndex: Math.max(0, model.indexOf(screen.entry.kind))
        }

        Label { text: "Note"; color: Colors.textMuted }
        TextArea {
            id: noteArea
            width: parent.width
            height: Spacing.xxxl * 2
            text: screen.entry.note
            wrapMode: TextEdit.Wrap
            placeholderText: "What happened"
        }
    }

    states: [
        State {
            name: "editing"
            when: screen.index >= 0
            PropertyChanges { target: header; title: "Edit note" }
        }
    ]
}
