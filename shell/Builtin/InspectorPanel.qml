// The selected object's bindings as written. A value is set as a literal
// (`property_set`); "Bind" sets it as an expression (`binding_set`). The
// last row adds a property not yet bound.
//
// Below them, its states (`state_edit`): one is picked to edit the values
// it changes and the condition that turns it on, or to give it a
// breakpoint, a width bound of the window.
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

    // The state picked to edit; empty for none.
    property string picked: ""
    readonly property var current: {
        const all = workspace.states
        for (let i = 0; i < all.length; i++) {
            if (all[i].name === picked)
                return all[i]
        }
        return null
    }

    function editState(args) {
        refusal.text = ""
        args.doc = workspace.doc
        args.node = {node: workspace.selected}
        workspace.dispatch("state_edit", args)
    }

    // A change is QML as written after the `:`, so what a person types is
    // what the file says; an empty one takes the change out.
    function change(target, name, text) {
        const args = {action: "change", state: picked, target: {id: target}, name: name}
        if (text !== "")
            args.binding = text
        editState(args)
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
            text: "States"
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }

        Repeater {
            model: panel.workspace.states
            delegate: RowLayout {
                id: stateRow
                required property var modelData
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Button {
                    text: stateRow.modelData.name
                    checkable: true
                    checked: panel.picked === stateRow.modelData.name
                    Accessible.name: "State " + stateRow.modelData.name
                    onClicked: panel.picked = checked ? stateRow.modelData.name : ""
                }
                TextField {
                    text: stateRow.modelData.name
                    Accessible.name: "Name of " + stateRow.modelData.name
                    Layout.fillWidth: true
                    onAccepted: {
                        const was = stateRow.modelData.name
                        if (text === was || text === "")
                            return
                        if (panel.picked === was)
                            panel.picked = text
                        panel.editState({action: "rename", state: was, new_name: text})
                    }
                }
                Button {
                    text: "Remove"
                    Accessible.name: "Remove state " + stateRow.modelData.name
                    onClicked: {
                        if (panel.picked === stateRow.modelData.name)
                            panel.picked = ""
                        panel.editState({action: "remove", state: stateRow.modelData.name})
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Spacing.xxs

            TextField {
                id: newState
                placeholderText: "state"
                Accessible.name: "New state"
                Layout.fillWidth: true
                onAccepted: addState.clicked()
            }
            Button {
                id: addState
                text: "Add"
                Accessible.name: "Add state"
                enabled: newState.text !== ""
                onClicked: {
                    panel.editState({action: "add", state: newState.text})
                    newState.text = ""
                }
            }
        }

        ColumnLayout {
            visible: panel.current !== null
            Layout.fillWidth: true
            spacing: Spacing.xxs

            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Label {
                    text: "when"
                    color: Colors.text
                    Layout.preferredWidth: 96
                }
                TextField {
                    text: panel.current !== null && panel.current.when !== undefined ? panel.current.when : ""
                    placeholderText: "always off"
                    Accessible.name: "When"
                    Layout.fillWidth: true
                    onAccepted: {
                        const args = {action: "when", state: panel.picked}
                        if (text !== "")
                            args.when = text
                        panel.editState(args)
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Repeater {
                    model: ["phone", "tablet", "desktop"]
                    delegate: Button {
                        required property string modelData
                        text: modelData
                        Accessible.name: "Breakpoint " + modelData
                        onClicked: panel.editState({action: "when", state: panel.picked, breakpoint: modelData})
                    }
                }
            }

            Repeater {
                model: panel.current !== null ? panel.current.changes : []
                delegate: RowLayout {
                    id: changeRow
                    required property var modelData
                    readonly property string name: modelData.target + "." + modelData.name
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Label {
                        text: changeRow.name
                        color: Colors.text
                        Layout.preferredWidth: 96
                        elide: Text.ElideRight
                    }
                    TextField {
                        text: changeRow.modelData.value
                        Accessible.name: changeRow.name
                        Layout.fillWidth: true
                        onAccepted: panel.change(changeRow.modelData.target, changeRow.modelData.name, text)
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                TextField {
                    id: changeTarget
                    placeholderText: "id"
                    Accessible.name: "Change target"
                    Layout.preferredWidth: 64
                }
                TextField {
                    id: changeName
                    placeholderText: "property"
                    Accessible.name: "Change property"
                    Layout.preferredWidth: 64
                }
                TextField {
                    id: changeValue
                    placeholderText: "value"
                    Accessible.name: "Change value"
                    Layout.fillWidth: true
                    onAccepted: setChange.clicked()
                }
                Button {
                    id: setChange
                    text: "Set"
                    Accessible.name: "Set change"
                    enabled: changeTarget.text !== "" && changeName.text !== ""
                    onClicked: panel.change(changeTarget.text, changeName.text, changeValue.text)
                }
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
