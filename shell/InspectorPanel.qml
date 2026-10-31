// The selected object's bindings as written. A value is set as a literal
// (`property_set`); "Bind" sets it as an expression (`binding_set`). The
// last row adds a property not yet bound.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Inspector"

    // A literal as a script writes one: a number, a string in quotes,
    // true, false or null. Anything else is an expression, for "Bind".
    function literal(text) {
        try {
            const value = JSON.parse(text)
            if (value === null || ["boolean", "number", "string"].indexOf(typeof value) >= 0)
                return {ok: true, value: value}
        } catch (e) {
        }
        return {ok: false}
    }

    function set(name, text) {
        const parsed = literal(text)
        if (!parsed.ok) {
            refusal.text = text + " is not a literal: bind it as an expression"
            return
        }
        refusal.text = ""
        workspace.dispatch("property_set", {
            doc: workspace.doc,
            node: {node: workspace.selected},
            name: name,
            value: parsed.value
        })
    }

    function bind(name, text) {
        refusal.text = ""
        workspace.dispatch("binding_set", {
            doc: workspace.doc,
            node: {node: workspace.selected},
            name: name,
            value: text
        })
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Spacing.xs
        spacing: Spacing.xxs
        enabled: panel.workspace.selected !== ""

        Repeater {
            model: panel.workspace.inspector
            delegate: RowLayout {
                id: row
                required property var modelData
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Label {
                    text: row.modelData.name
                    color: Colors.text
                    Layout.preferredWidth: 96
                    elide: Text.ElideRight
                }
                TextField {
                    id: value
                    text: row.modelData.value
                    Accessible.name: row.modelData.name
                    Layout.fillWidth: true
                    onAccepted: panel.set(row.modelData.name, text)
                }
                Button {
                    text: "Bind"
                    Accessible.name: "Bind " + row.modelData.name
                    onClicked: panel.bind(row.modelData.name, value.text)
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Spacing.xxs

            TextField {
                id: newName
                placeholderText: "property"
                Accessible.name: "New property"
                Layout.preferredWidth: 96
            }
            TextField {
                id: newValue
                placeholderText: "value"
                Accessible.name: "New value"
                Layout.fillWidth: true
                onAccepted: add.clicked()
            }
            Button {
                id: add
                text: "Set"
                Accessible.name: "Set new property"
                enabled: newName.text !== ""
                onClicked: panel.set(newName.text, newValue.text)
            }
            Button {
                text: "Bind"
                Accessible.name: "Bind new property"
                enabled: newName.text !== ""
                onClicked: panel.bind(newName.text, newValue.text)
            }
        }

        Label {
            id: refusal
            objectName: "inspectorRefusal"
            color: Colors.text
            visible: text !== ""
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
