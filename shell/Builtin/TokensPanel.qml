// The roles of Ljos.Design: a colour with a swatch and a value for light
// and for dark, any other role with its value, the theme file's where it
// has one and the module's where it does not. Setting one is the
// `theme_set` call an agent makes, which writes the theme file beside the
// document shown; an empty value gives the role back to the module.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Tokens"

    // A role's value as `theme_set` takes it: a colour as written, a number
    // as a number, a curve as its JSON. Empty is null.
    function parsed(kind, text) {
        const trimmed = text.trim()
        if (trimmed === "")
            return {ok: true, value: null}
        if (kind === "color")
            return {ok: true, value: trimmed}
        try {
            return {ok: true, value: JSON.parse(trimmed)}
        } catch (e) {
            return {ok: false}
        }
    }

    function set(row, variant, text) {
        const value = parsed(row.kind, text)
        if (!value.ok) {
            refusal.text = text + " is not a value of " + row.token
            return
        }
        refusal.text = ""
        const args = {token: row.token, value: value.value}
        if (workspace.doc !== "")
            args.doc = workspace.doc
        if (variant !== "")
            args.variant = variant
        workspace.dispatch("theme_set", args)
    }

    Connections {
        target: panel.workspace
        function onAnswered(tool, answer) {
            if (tool === "theme_set")
                refusal.text = ""
        }
        function onRefused(tool, error) {
            if (tool === "theme_set")
                refusal.text = error.message
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Spacing.xs
        spacing: Spacing.xxs

        // The one colour a theme starts from, which the roles are worked out
        // of. A hue is a place on a wheel, so it is picked from one.
        Row {
            id: hues
            Layout.fillWidth: true
            Repeater {
                model: 24
                delegate: AbstractButton {
                    id: hueTile
                    required property int index
                    readonly property int hue: index * 15
                    width: hues.width / 24
                    height: 20
                    Accessible.role: Accessible.Button
                    Accessible.name: "Hue " + hue
                    onClicked: panel.set({kind: "number", token: "Colors.seedHue"}, "", String(hue))
                    background: Rectangle {
                        color: Qt.hsla(hueTile.hue / 360, 0.7, 0.55, 1)
                    }
                }
            }
        }

        Field {
            id: filter
            placeholderText: "Filter tokens"
            Accessible.name: "Filter tokens"
            Layout.fillWidth: true
        }

        Label {
            id: refusal
            objectName: "tokensRefusal"
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            color: Colors.text
            visible: text !== ""
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }

        ListView {
            objectName: "tokens"
            clip: true
            spacing: Spacing.xxs
            Layout.fillWidth: true
            Layout.fillHeight: true
            Accessible.role: Accessible.List
            Accessible.name: "Tokens"
            model: {
                const wanted = filter.text.trim().toLowerCase()
                return panel.workspace.tokens.filter(t => t.token.toLowerCase().indexOf(wanted) >= 0)
            }

            delegate: ColumnLayout {
                id: row
                required property var modelData
                readonly property bool colour: modelData.kind === "color"
                width: ListView.view.width
                spacing: 0

                Label {
                    text: row.modelData.set ? row.modelData.token + " (theme)" : row.modelData.token
                    color: Colors.text
                    font.pixelSize: Typography.captionSize
                    font.weight: Typography.bodyStrongWeight
                    topPadding: Spacing.xs
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                RowLayout {
                    visible: !row.colour
                    Layout.fillWidth: true
                    // What the value does, beside the number that does it.
                    Text {
                        visible: /size|weight/i.test(row.modelData.token)
                        text: "Aa"
                        color: Colors.text
                        font.pixelSize: /size/i.test(row.modelData.token)
                            ? Math.max(10, Number(row.modelData.value)) : Typography.bodySize
                        font.weight: /weight/i.test(row.modelData.token)
                            ? Number(row.modelData.value) : Typography.bodyWeight
                        Accessible.ignored: true
                    }
                    Rectangle {
                        visible: /^Spacing\./.test(row.modelData.token)
                        implicitWidth: Math.min(64, Number(row.modelData.value))
                        implicitHeight: 8
                        color: Colors.accent
                        Accessible.ignored: true
                    }
                    Field {
                        text: row.modelData.value
                        Accessible.name: row.modelData.token
                        Layout.fillWidth: true
                        onAccepted: {
                            const typed = text
                            // The field shows the role's value again,
                            // the new one once it is set, the old one
                            // when it is refused.
                            text = Qt.binding(() => row.modelData.value)
                            panel.set(row.modelData, "", typed)
                        }
                    }
                }

                // Light and dark side by side: a role is read as a pair, and
                // a row each pushed the list out of any panel's height.
                RowLayout {
                    visible: row.colour
                    Layout.fillWidth: true
                    spacing: Spacing.xs

                    Repeater {
                        model: row.colour ? ["light", "dark"] : []
                        delegate: RowLayout {
                            id: variantRow
                            required property string modelData
                            readonly property string shown: row.modelData[modelData]
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            spacing: Spacing.xxs

                            Rectangle {
                                implicitWidth: 24
                                implicitHeight: 24
                                radius: 4
                                color: variantRow.shown
                                // A swatch the colour of the panel still shows.
                                border.width: 1
                                border.color: Colors.border
                                Accessible.role: Accessible.Graphic
                                Accessible.name: row.modelData.token + " " + variantRow.modelData + " swatch"
                            }
                            Field {
                                text: variantRow.shown
                                Accessible.name: row.modelData.token + " " + variantRow.modelData
                                Layout.fillWidth: true
                                Layout.minimumWidth: 40
                                ToolTip.visible: hovered
                                ToolTip.text: variantRow.modelData
                                onAccepted: {
                                    const typed = text
                                    text = Qt.binding(() => variantRow.shown)
                                    panel.set(row.modelData, variantRow.modelData, typed)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
