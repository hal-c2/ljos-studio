// The selected object's bindings as written, and the properties it declares
// without a value. A value is set as a literal (`binding_set`, as written); "Bind" sets
// it as an expression (`binding_set`). The last row adds a property not yet
// bound. A colour has a swatch, which opens a picker.
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

    // The new-property row's edit: answered, the row is emptied for the next.
    property bool rowEditing: false

    function fromRow(edit) {
        rowEditing = true
        edit()
        // Stopped before it was asked: there will be no answer.
        if (refusal.text !== "")
            rowEditing = false
    }

    signal reseed()
    Connections {
        target: panel.workspace
        function onSelectedChanged() {
            panel.reseed()
            // The selection is shared, and what is open was opened on another.
            picker.close()
            // An object removed under an open picker empties the selection,
            // and what the picker says of that is still to be read.
            if (panel.workspace.selected !== "")
                panel.forgetRefused()
            else
                panel.offered = null
        }
        function onRefused(tool, error) {
            if (["property_set", "binding_set", "state_edit"].includes(tool))
                panel.reseed()
            if (tool === "state_edit" && panel.renamedPicked !== "") {
                panel.pick(panel.renamedPicked, panel.pickedOn, true)
                panel.renamedPicked = ""
            }
        }
        function onAnswered(tool, answer) {
            if (tool === "state_edit")
                panel.renamedPicked = ""
        }
    }
    // The picked state's old name while its rename is unanswered: the pick
    // goes back to it if the rename is refused.
    property string renamedPicked: ""

    // What a row offers of colour: all of the swatch, the picker and its note
    // follow this, and features/ui/inspector.feature's table says what each
    // kind of row gets. The row's type or name says it is meant to be a colour,
    // which the runtime cannot: "red" is a fine colour and a fine label. So
    // text only earns a picker by itself when it is `#`, a written colour by
    // any reading.
    function colourRow(row, written) {
        const text = writtenText(written)
        const colour = text !== null && writesColour(text) ? text : ""
        return {
            picker: row.colour === true || /(^|\.)[a-zA-Z]*[cC]olor$/.test(row.name) || colour.charAt(0) === "#",
            paint: colour,
            note: written !== "" && !workspace.literal(written)
        }
    }
    // The text a value spells out, in any quotes, or null when it spells out
    // none. The parser says it is a literal; its first character says which.
    function writtenText(value) {
        return workspace.literal(value) && /^["'`]/.test(value) ? value.slice(1, -1) : null
    }
    // The runtime's own reading: what it does not parse would not paint.
    function writesColour(text) {
        try {
            Qt.color(text)
            return true
        } catch (e) {
            return false
        }
    }

    // A row that holds an expression stays one when it is edited: a vector
    // or an enumeration has no literal to fall back on. Only the row's own
    // kind, as the parser read it, decides, so a typo in a literal row is
    // still refused, not bound.
    function enter(row, text) {
        if (picked === "" && row.name !== "id" && row.expression === true && !workspace.literal(text.trim()))
            bind(row.name, text)
        else
            set(row.name, text)
    }

    // A refused value the refusal offers to bind, while that refusal stands.
    property var offered: null
    property string offeredSays: ""

    // On the selected object unless a picker says which it was opened on.
    function set(name, text, node) {
        if (picked !== "") {
            record(name, text, node)
            return
        }
        // The parser's word on what QML reads as a literal, and written as typed:
        // `0x20` stays `0x20`. An expression is for "Bind".
        const typed = text.trim()
        if (!workspace.literal(typed)) {
            // The field goes back to what the file says; what was typed is
            // kept for the button under the refusal, which is what it asks for.
            offered = {name: name, value: typed}
            refusal.text = typed + " is not a literal: bind it as an expression"
            offeredSays = refusal.text
            reseed()
            return
        }
        refusal.text = ""
        workspace.dispatch("binding_set", {
            doc: workspace.doc,
            node: {node: node || workspace.selected},
            name: name,
            value: typed === "null" ? null : typed
        })
    }

    // The state picked to edit; empty for none. It belongs to the object
    // selected when it was picked, and stays picked as the selection moves.
    property string picked: ""
    property string pickedOn: ""
    // For what else sets values: the canvas records into the same state.
    readonly property var recording: picked === "" ? null : {state: picked, on: {node: pickedOn}}
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
        forgetRefused()
    }
    // A refused value was typed on one object, in one document, into one state;
    // its offer would write it to whatever is there now.
    function forgetRefused() {
        refusal.text = ""
        offered = null
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
    // With `follows` this comes after an edit, whose refusal it leaves alone;
    // without it, it is the person's own gesture.
    function pick(name, on, follows) {
        if (!follows)
            forgetRefused()
        const was = pickedOn
        const had = picked !== ""
        picked = name
        pickedOn = on
        const conditioned = workspace.states.some(s => s.name === name && s.when !== undefined)
        if (name !== "" && !conditioned)
            show(on, name, follows)
        else if (had)
            show(was, "", follows)
    }
    function show(on, state, follows) {
        const object = layer(on)
        if (!workspace.previews || object === null || object.id === "")
            return
        if (follows)
            workspace.follow("preview_state", {node: {node: on}, show: state})
        else
            workspace.dispatch("preview_state", {node: {node: on}, show: state})
    }

    // What a state changes is written with the object's id: there is no other
    // way for a `PropertyChanges` to name it.
    function record(name, text, node) {
        const object = layer(node || workspace.selected)
        if (object === null || object.id === "") {
            refusal.text = "A state names the object it changes: give this one an id"
            return
        }
        change(object.id, name, text)
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
        function onAnswered(tool, answer, asked) {
            if (tool === "review" && panel.adding) {
                panel.adding = false
                newComment.text = ""
            }
            if (tool === "preview_state" && picker.asks(asked)) {
                const value = answer.state[picker.name]
                const read = value !== undefined ? String(value) : null
                // The running app has no value for it yet, so any is a start.
                if (value === null) {
                    picker.start("#000000")
                } else if (read !== null && panel.writesColour(read)) {
                    picker.start(read)
                } else {
                    picker.unread()
                    if (read !== null)
                        refusal.text = read + " is not a colour"
                }
            }
            if (panel.rowEditing && ["property_set", "binding_set", "state_edit"].includes(tool)) {
                panel.rowEditing = false
                newName.text = ""
                newValue.text = ""
            }
        }
        function onRefused(tool, error, asked) {
            if (tool === "review")
                panel.adding = false
            // The reason is on the window's error line; the sliders go on from
            // grey, since picking replaces what could not be read.
            if (tool === "preview_state" && picker.asks(asked))
                picker.unread()
            panel.rowEditing = false
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

    // Hue, saturation and value, or the hex a person has from elsewhere.
    // Set when the hand lets go or Enter is pressed: a drag is one undo step.
    // A colour an expression works out to is asked of the preview, so the
    // sliders start from what is on the canvas; when it cannot say, they go
    // on from a mid grey and an empty hex rather than stay off or start black.
    Popup {
        id: picker
        objectName: "colourPicker"
        property var row: ({})
        property string name: ""
        // The object it was opened on: the selection can move while it is open.
        property string node: ""
        // Read when asked: the object can be named again while this is open.
        readonly property string objectId: {
            const object = panel.layer(node)
            return object !== null ? object.id : ""
        }
        // Showing a state or clicking in play reads the same object: only this
        // picker's own read ends its wait.
        function asks(call) {
            return waiting && call.read !== undefined && call.read.includes(name)
                && call.node.node === node
        }
        property string written: ""
        property bool waiting: false
        property bool known: false
        // What the sliders have no say over stays as it was written.
        property real alpha: 1
        readonly property color chosen: Qt.hsva(hue.value, saturation.value, brightness.value, alpha)

        function start(colour) {
            const start = Qt.color(colour)
            hue.value = Math.max(0, start.hsvHue)
            saturation.value = start.hsvSaturation
            brightness.value = start.hsvValue
            alpha = start.a
            hex.text = colour
            waiting = false
            known = true
        }
        // Nothing says what colour it is now. A mid grey moves every way, and
        // the empty hex says it is no reading of the file.
        function unread() {
            start("#808080")
            hex.text = ""
        }
        function show(row, written, at) {
            refusal.text = ""
            picker.row = row
            picker.name = row.name
            picker.written = written
            picker.node = panel.workspace.selected
            // Beside the inspector, where it hides none of the rows.
            x = -implicitWidth - Spacing.xs
            y = at.mapToItem(panel, 0, 0).y
            const paint = panel.colourRow(row, written).paint
            if (paint !== "") {
                start(paint)
            } else {
                known = false
                hex.text = ""
                waiting = true
                panel.workspace.dispatch("preview_state", {
                    doc: panel.workspace.doc,
                    node: {node: picker.node},
                    read: [name]
                })
            }
            open()
        }
        function take(text) {
            written = JSON.stringify(text)
            if (panel.layer(node) === null) {
                refusal.text = "The object this picker was opened on has gone"
                return
            }
            panel.set(name, written, node)
        }

        onClosed: waiting = false
        padding: Spacing.xs
        ColumnLayout {
            spacing: Spacing.xxs
            Label {
                visible: panel.colourRow(picker.row, picker.written).note
                text: "Worked out by " + picker.written + ": picking replaces it"
                color: Colors.text
                font.pixelSize: Typography.captionSize
                Layout.maximumWidth: 200
                wrapMode: Text.Wrap
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 32
                color: picker.chosen
                border.color: Colors.border
            }
            Label {
                text: "Hue"
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Slider {
                id: hue
                enabled: picker.known
                Accessible.name: "Hue"
                onPressedChanged: if (!pressed) picker.take(picker.chosen.toString())
                onMoved: hex.text = picker.chosen.toString()
            }
            Label {
                text: "Saturation"
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Slider {
                id: saturation
                objectName: "pickerSaturation"
                enabled: picker.known
                Accessible.name: "Saturation"
                onPressedChanged: if (!pressed) picker.take(picker.chosen.toString())
                onMoved: hex.text = picker.chosen.toString()
            }
            Label {
                text: "Brightness"
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Slider {
                id: brightness
                objectName: "pickerBrightness"
                enabled: picker.known
                Accessible.name: "Brightness"
                onPressedChanged: if (!pressed) picker.take(picker.chosen.toString())
                onMoved: hex.text = picker.chosen.toString()
            }
            Field {
                id: hex
                Layout.fillWidth: true
                placeholderText: "#rrggbb"
                Accessible.name: "Colour"
                onAccepted: {
                    if (!panel.writesColour(text))
                        return
                    picker.take(text)
                    // The sliders go on from it, not from what it replaced.
                    picker.start(text)
                }
            }
        }
    }

    Flickable {
        id: scroll
        anchors.fill: parent
        clip: true
        contentHeight: rows.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {
            Accessible.name: "Scroll the inspector"
        }

    ColumnLayout {
        id: rows
        width: scroll.width
        spacing: 0

        Label {
            visible: panel.workspace.selected === ""
            text: "Select something to edit it"
            color: Colors.textMuted
            font.pixelSize: Typography.captionSize
            padding: Spacing.md
            Layout.alignment: Qt.AlignHCenter
        }

        Label {
            id: refusal
            objectName: "inspectorRefusal"
            color: Colors.danger
            visible: text !== ""
            wrapMode: Text.Wrap
            font.pixelSize: Typography.captionSize
            padding: Spacing.xs
            Layout.fillWidth: true
        }
        TextButton {
            visible: panel.offered !== null && refusal.text === panel.offeredSays
            Layout.leftMargin: Spacing.xs
            text: visible ? "Bind " + panel.offered.name + " to " + panel.offered.value : ""
            onClicked: panel.bind(panel.offered.name, panel.offered.value)
        }

        Section {
            title: "Properties"
            visible: panel.workspace.selected !== ""
            Layout.fillWidth: true

            Repeater {
                model: panel.workspace.inspector
                delegate: RowLayout {
                    id: row
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Label {
                        text: row.modelData.name
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        Layout.preferredWidth: 72
                        elide: Text.ElideRight
                    }
                    TextButton {
                        id: swatch
                        readonly property var kind: panel.colourRow(row.modelData, value.text)
                        visible: kind.picker
                        Layout.preferredWidth: 24
                        Layout.preferredHeight: 24
                        padding: 0
                        Accessible.name: "Pick " + row.modelData.name
                        Accessible.description: kind.paint
                        onClicked: picker.show(row.modelData, value.text, this)
                        contentItem: Rectangle {
                            color: swatch.kind.paint || "transparent"
                            border.color: Colors.border
                        }
                    }
                    Field {
                        id: value
                        text: panel.inState(row.modelData.name, row.modelData.value)
                        placeholderText: "no value"
                        Accessible.name: row.modelData.name
                        Layout.fillWidth: true
                        Layout.minimumWidth: 96
                        onAccepted: panel.enter(row.modelData, text)
                        // The rows are not made anew for a list that reads the
                        // same, and what is typed in one is not the next object's,
                        // nor stays when the file refused it.
                        Connections {
                            target: panel
                            function onReseed() {
                                value.text = Qt.binding(() => panel.inState(row.modelData.name, row.modelData.value))
                            }
                        }
                    }
                    IconButton {
                        icon_: "link"
                        text: "Bind " + row.modelData.name
                        onClicked: panel.bind(row.modelData.name, value.text)
                    }
                }
            }

            // Two lines: on one, the panel's width left the value a few pixels.
            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Field {
                    id: newName
                    placeholderText: "property"
                    Accessible.name: "New property"
                    Layout.preferredWidth: 96
                }
                Field {
                    id: newValue
                    placeholderText: "value"
                    Accessible.name: "New value"
                    Layout.fillWidth: true
                    onAccepted: add.clicked()
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Item {
                    Layout.fillWidth: true
                }
                TextButton {
                    id: add
                    text: "Set"
                    Accessible.name: "Set new property"
                    enabled: newName.text !== ""
                    onClicked: panel.fromRow(() => panel.set(newName.text, newValue.text))
                }
                TextButton {
                    text: "Bind"
                    Accessible.name: "Bind new property"
                    enabled: newName.text !== ""
                    onClicked: panel.fromRow(() => panel.bind(newName.text, newValue.text))
                }
            }
        }

        Section {
            title: "States"
            visible: panel.workspace.selected !== ""
            Layout.fillWidth: true

            Repeater {
                model: panel.workspace.states
                delegate: RowLayout {
                    id: stateRow
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    TextButton {
                        text: stateRow.modelData.name
                        checkable: true
                        checked: panel.picked === stateRow.modelData.name
                        Accessible.name: "State " + stateRow.modelData.name
                        onClicked: panel.pick(checked ? stateRow.modelData.name : "", checked ? panel.workspace.selected : "")
                    }
                    Field {
                        id: stateName
                        text: stateRow.modelData.name
                        Accessible.name: "Name of " + stateRow.modelData.name
                        Layout.fillWidth: true
                        onAccepted: {
                            const was = stateRow.modelData.name
                            if (text === was || text === "")
                                return
                            panel.editState({action: "rename", state: was, new_name: text})
                            if (panel.picked === was) {
                                panel.renamedPicked = was
                                panel.pick(text, panel.pickedOn, true)
                            }
                        }
                        Connections {
                            target: panel
                            function onReseed() {
                                stateName.text = Qt.binding(() => stateRow.modelData.name)
                            }
                        }
                    }
                    IconButton {
                        icon_: "trash"
                        text: "Remove state " + stateRow.modelData.name
                        onClicked: {
                            const name = stateRow.modelData.name
                            panel.editState({action: "remove", state: name})
                            if (panel.picked === name)
                                panel.pick("", "", true)
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Field {
                    id: newState
                    placeholderText: "state"
                    Accessible.name: "New state"
                    Layout.fillWidth: true
                    onAccepted: addState.clicked()
                }
                TextButton {
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
                    Field {
                        id: stateWhen
                        text: panel.current !== null && panel.current.when !== undefined ? panel.current.when : ""
                        placeholderText: "always off"
                        Accessible.name: "When"
                        Layout.fillWidth: true
                        onAccepted: {
                            const args = {action: "when", state: panel.picked}
                            if (text !== "")
                                args.when = text
                            panel.editState(args)
                            panel.show(panel.pickedOn, "", true)
                        }
                        Connections {
                            target: panel
                            function onReseed() {
                                stateWhen.text = Qt.binding(() => panel.current !== null && panel.current.when !== undefined ? panel.current.when : "")
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Repeater {
                        model: ["phone", "tablet", "desktop"]
                        delegate: TextButton {
                            required property string modelData
                            text: modelData
                            Accessible.name: "Breakpoint " + modelData
                            onClicked: {
                                panel.editState({action: "when", state: panel.picked, breakpoint: modelData})
                                panel.show(panel.pickedOn, "", true)
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
                        Field {
                            id: changeNow
                            text: changeRow.modelData.value
                            Accessible.name: changeRow.name
                            Layout.fillWidth: true
                            onAccepted: panel.change(changeRow.modelData.target, changeRow.modelData.name, text)
                            Connections {
                                target: panel
                                function onReseed() {
                                    changeNow.text = Qt.binding(() => changeRow.modelData.value)
                                }
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Field {
                        id: changeTarget
                        placeholderText: "id"
                        Accessible.name: "Change target"
                        Layout.preferredWidth: 64
                    }
                    Field {
                        id: changeName
                        placeholderText: "property"
                        Accessible.name: "Change property"
                        Layout.preferredWidth: 64
                    }
                    Field {
                        id: changeValue
                        placeholderText: "value"
                        Accessible.name: "Change value"
                        Layout.fillWidth: true
                        onAccepted: setChange.clicked()
                    }
                    TextButton {
                        id: setChange
                        text: "Set"
                        Accessible.name: "Set change"
                        enabled: changeTarget.text !== "" && changeName.text !== ""
                        onClicked: panel.change(changeTarget.text, changeName.text, changeValue.text)
                    }
                }
            }
        }

        Section {
            title: "Signals"
            Layout.fillWidth: true
            visible: panel.workspace.selected !== "" && signalList.count > 0

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
        }

        Section {
            title: "Roles"
            Layout.fillWidth: true
            visible: panel.workspace.roles.length > 0

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
                    Field {
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
        }

        Section {
            title: "Comments"
            Layout.fillWidth: true

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
                    TextButton {
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

                Field {
                    id: newComment
                    placeholderText: "comment"
                    Accessible.name: "New comment"
                    Layout.fillWidth: true
                    onAccepted: if (addComment.enabled) addComment.clicked()
                }
                TextButton {
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
        }
    }
    }
}
