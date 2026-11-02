// The project's documents; choosing one shows it everywhere. A new one is made
// by `document_new`, and an empty directory opens with a first one: a window
// with nothing to edit has nothing to offer.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Documents"

    // The document asked for and not yet answered.
    property string making: ""
    property string refusal: ""
    // Only the window's first look: an agent that empties or switches a
    // project later has its own plans for the directory.
    property bool looked: false

    function show(doc) {
        workspace.doc = doc
        // The canvas shows what an agent's preview would.
        if (workspace.previews)
            workspace.dispatch("preview_open", {doc: doc})
    }

    function make(name) {
        const doc = name.endsWith(".qml") ? name : name + ".qml"
        refusal = ""
        making = doc
        workspace.dispatch("document_new", {doc: doc})
    }

    function firstLook() {
        // Before the first publish the list is empty and `blank` is false:
        // that is no look yet.
        if (looked || (!workspace.blank && workspace.documents.length === 0))
            return
        looked = true
        if (workspace.blank)
            make("Main.qml")
    }

    Component.onCompleted: firstLook()

    Connections {
        target: panel.workspace
        function onDocumentsChanged() { panel.firstLook() }
        function onBlankChanged() { panel.firstLook() }
        function onAnswered(tool, answer) {
            if (tool === "document_new" && panel.making !== "") {
                const doc = panel.making
                panel.making = ""
                nameField.text = ""
                panel.show(doc)
            }
        }
        function onRefused(tool, error) {
            if (tool === "document_new" && panel.making !== "") {
                panel.making = ""
                panel.refusal = error.message
            }
        }
    }

    Column {
        id: maker
        width: parent.width
        spacing: Spacing.xxs

        Row {
            spacing: Spacing.xxs
            Field {
                id: nameField
                width: maker.width - newButton.width - Spacing.xxs
                placeholderText: "Name"
                Accessible.name: "New document's name"
                onAccepted: newButton.clicked()
            }
            TextButton {
                id: newButton
                text: "New"
                Accessible.name: "New document"
                enabled: nameField.text !== ""
                onClicked: panel.make(nameField.text)
            }
        }
        Label {
            objectName: "newDocumentRefusal"
            width: maker.width
            visible: text !== ""
            text: panel.refusal
            wrapMode: Text.Wrap
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
    }

    ListView {
        id: documentList
        objectName: "documents"
        anchors.fill: parent
        anchors.topMargin: maker.height + Spacing.xxs
        clip: true
        model: panel.workspace.documents
        // The list is short on purpose: the open document stays in it.
        currentIndex: panel.workspace.documents.indexOf(panel.workspace.doc)
        Accessible.role: Accessible.List
        Accessible.name: "Documents"
        ScrollBar.vertical: ScrollBar {
            Accessible.name: "Scroll the documents"
            // Always there when there is more: nothing else says so. Hidden,
            // not only off, when there is not: an unused bar still fades.
            policy: ScrollBar.AlwaysOn
            visible: documentList.contentHeight > documentList.height
        }

        delegate: ItemDelegate {
            id: entry
            required property string modelData
            width: ListView.view.width
            implicitHeight: 28
            padding: 0
            leftPadding: Spacing.xs
            text: modelData
            Accessible.name: modelData
            highlighted: modelData === panel.workspace.doc
            onClicked: panel.show(modelData)
            contentItem: RowLayout {
                spacing: Spacing.xxs
                Icon {
                    name: "document"
                    color: entry.highlighted ? Colors.accent : Colors.textMuted
                }
                Text {
                    text: entry.text
                    color: Colors.text
                    font.pixelSize: Typography.captionSize
                    font.weight: entry.highlighted ? Typography.bodyStrongWeight : Typography.bodyWeight
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
            background: Rectangle {
                radius: 6
                color: entry.highlighted ? Colors.surfaceSelected
                    : entry.hovered ? Colors.surfaceHover : "transparent"
            }
        }
    }
}
