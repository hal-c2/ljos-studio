// The selected object's bindings as written. A value is set as a literal
// (`property_set`); "Bind" sets it as an expression (`binding_set`). The
// last row adds a property not yet bound.
//
// Below them, its states (`state_edit`): one is picked to edit the values
// it changes and the condition that turns it on, or to give it a
// breakpoint, a width bound of the window. While one is picked, the frame
// shows it (`preview_state`) and what this panel sets goes into it, whichever
// object is selected, until the person is done.
//
// Then the signals it has, with their arguments, which the flow panel
// wires; and the roles of the data it sees, in a list's delegate: one is
// bound to a property as `model.<role>`, the form a delegate reads it in.
//
// Last, its review comments (`review`), and those of the document whose
// objects were taken out: a person adds one, resolves it or reopens it.
// They are kept in Studio's sidecar; the file does not change.
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
        if (picked !== "") {
            record(name, text)
            return
        }
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

    // The state picked to edit; empty for none. It belongs to the object
    // selected when it was picked, and stays picked as the selection moves.
    property string picked: ""
    property string pickedOn: ""
    readonly property var current: {
        const all = workspace.selected === pickedOn ? workspace.states : workspace.rootStates
        for (let i = 0; i < all.length; i++) {
            if (all[i].name === picked)
                return all[i]
        }
        return null
    }
    readonly property string selectedId: {
        const found = layer(workspace.selected)
        return found !== null ? found.id : ""
    }
    // Another document has its own states, and its own preview.
    readonly property string pickedDoc: workspace.doc
    onPickedDocChanged: {
        picked = ""
        pickedOn = ""
    }

    function layer(node) {
        const all = workspace.layers
        for (let i = 0; i < all.length; i++) {
            if (all[i].node === node)
                return all[i]
        }
        return null
    }

    // The frame shows what is being edited, by the object's id, which is how
    // a running preview is read. A state with a `when` is the app's to turn
    // on: forcing it would have a frame claim what its width does not do.
    function pick(name, on) {
        const was = pickedOn
        const had = picked !== ""
        picked = name
        pickedOn = on
        const conditioned = workspace.states.some(s => s.name === name && s.when !== undefined)
        if (name !== "" && !conditioned)
            show(on, name)
        else if (had)
            show(was, "")
    }
    function show(on, state) {
        const object = layer(on)
        if (workspace.previews && object !== null && object.id !== "")
            workspace.dispatch("preview_state", {node: {node: on}, show: state})
    }

    // What a state changes is written with the object's id: there is no other
    // way for a `PropertyChanges` to name it.
    function record(name, text) {
        if (selectedId === "") {
            refusal.text = "A state names the object it changes: give this one an id"
            return
        }
        change(selectedId, name, text)
    }

    // The value the picked state gives the selected object's property.
    function inState(name, base) {
        if (current === null)
            return base
        const changes = current.changes
        for (let i = 0; i < changes.length; i++) {
            if (changes[i].target === selectedId && changes[i].name === name)
                return changes[i].value
        }
        return base
    }

    function editState(args) {
        refusal.text = ""
        args.doc = workspace.doc
        args.node = {node: pickedOn !== "" && args.state === picked ? pickedOn : workspace.selected}
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

    // The selected object's comments, then those whose objects are gone.
    readonly property var comments: {
        const all = workspace.comments
        const out = []
        for (let i = 0; i < all.length; i++) {
            if (all[i].orphaned || all[i].node === workspace.selected)
                out.push(all[i])
        }
        return out
    }

    // The box keeps its text until the sidecar took the comment, so a
    // refused one can be sent again rather than typed again.
    property bool adding: false
    Connections {
        target: panel.workspace
        function onAnswered(tool, answer) {
            if (tool === "review" && panel.adding) {
                panel.adding = false
                newComment.text = ""
            }
        }
        function onRefused(tool, error) {
            if (tool === "review")
                panel.adding = false
        }
    }

    function review(args) {
        refusal.text = ""
        workspace.dispatch("review", args)
    }

    function bind(name, text) {
        if (picked !== "") {
            record(name, text)
            return
        }
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
                    text: panel.inState(row.modelData.name, row.modelData.value)
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
                    onClicked: panel.pick(checked ? stateRow.modelData.name : "", checked ? panel.workspace.selected : "")
                }
                TextField {
                    text: stateRow.modelData.name
                    Accessible.name: "Name of " + stateRow.modelData.name
                    Layout.fillWidth: true
                    onAccepted: {
                        const was = stateRow.modelData.name
                        if (text === was || text === "")
                            return
                        panel.editState({action: "rename", state: was, new_name: text})
                        if (panel.picked === was)
                            panel.pick(text, panel.pickedOn)
                    }
                }
                Button {
                    text: "Remove"
                    Accessible.name: "Remove state " + stateRow.modelData.name
                    onClicked: {
                        const name = stateRow.modelData.name
                        panel.editState({action: "remove", state: name})
                        if (panel.picked === name)
                            panel.pick("", "")
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

        RowLayout {
            visible: panel.picked !== ""
            Layout.fillWidth: true
            spacing: Spacing.xxs

            Label {
                text: "Editing state " + panel.picked
                color: Colors.text
                font.weight: Typography.bodyStrongWeight
                Accessible.role: Accessible.StaticText
                Accessible.name: text
                Layout.fillWidth: true
            }
            Button {
                text: "Done"
                Accessible.name: "Done editing state"
                onClicked: panel.pick("", "")
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
                        panel.show(panel.pickedOn, "")
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
                        onClicked: {
                            panel.editState({action: "when", state: panel.picked, breakpoint: modelData})
                            panel.show(panel.pickedOn, "")
                        }
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
            visible: signalList.count > 0
            text: "Signals"
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }

        // What a flow may handle on it, with the arguments a handler is
        // given; a property's change signal, which every property has, is
        // left out.
        Repeater {
            id: signalList
            model: panel.workspace.signals.filter(s => !s.change)
            delegate: Label {
                required property var modelData
                text: modelData.name + "(" + modelData.arguments
                    .map(a => a.type === "" ? a.name : a.name + ": " + a.type).join(", ") + ")"
                color: Colors.text
                elide: Text.ElideRight
                Layout.fillWidth: true
                Accessible.role: Accessible.StaticText
                Accessible.name: "Signal " + modelData.name
            }
        }

        Label {
            visible: panel.workspace.roles.length > 0
            text: "Roles"
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }

        Repeater {
            model: panel.workspace.roles
            delegate: RowLayout {
                id: roleRow
                required property string modelData
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Label {
                    text: roleRow.modelData
                    color: Colors.text
                    Layout.preferredWidth: 96
                    elide: Text.ElideRight
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Role " + roleRow.modelData
                }
                TextField {
                    placeholderText: "property"
                    Accessible.name: "Bind role " + roleRow.modelData + " to"
                    Layout.fillWidth: true
                    onAccepted: {
                        if (text === "")
                            return
                        panel.bind(text, "model." + roleRow.modelData)
                        text = ""
                    }
                }
            }
        }

        Label {
            text: "Comments"
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }

        Repeater {
            model: panel.comments
            delegate: RowLayout {
                id: commentRow
                required property var modelData
                readonly property string kind: modelData.orphaned ? "Orphaned"
                                               : modelData.resolved ? "Resolved" : "Open"
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Label {
                    text: commentRow.modelData.orphaned
                          ? commentRow.modelData.text + " (" + commentRow.modelData.label + ", removed)"
                          : commentRow.modelData.text
                    color: commentRow.modelData.resolved ? Colors.textMuted : Colors.text
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                    Accessible.role: Accessible.StaticText
                    Accessible.name: commentRow.kind + " comment: " + commentRow.modelData.text
                }
                Button {
                    text: commentRow.modelData.resolved ? "Reopen" : "Resolve"
                    Accessible.name: text + " " + commentRow.modelData.text
                    onClicked: panel.review({
                        action: commentRow.modelData.resolved ? "reopen" : "resolve",
                        comment: commentRow.modelData.id
                    })
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Spacing.xxs

            TextField {
                id: newComment
                placeholderText: "comment"
                Accessible.name: "New comment"
                Layout.fillWidth: true
                onAccepted: if (addComment.enabled) addComment.clicked()
            }
            Button {
                id: addComment
                text: "Comment"
                Accessible.name: "Add comment"
                enabled: newComment.text.trim() !== "" && panel.workspace.selected !== ""
                onClicked: {
                    panel.adding = true
                    panel.review({
                        action: "add",
                        doc: panel.workspace.doc,
                        node: {node: panel.workspace.selected},
                        text: newComment.text
                    })
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
