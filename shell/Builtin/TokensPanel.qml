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

    function unitOf(token) {
        return /^(Spacing|Radius)\.|Size$/.test(token) ? "px"
            : /^Motion\.(enter|exit|move|change)[A-Z]/.test(token) ? "ms" : ""
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
        Rectangle {
            id: hues
            readonly property real seed: {
                const row = panel.workspace.tokens.find(t => t.token === "Colors.seedHue")
                return row !== undefined ? Number(row.value) : NaN
            }
            Layout.fillWidth: true
            implicitHeight: 20
            radius: 6
            clip: true
            color: "transparent"
            Repeater {
                model: 24
                delegate: AbstractButton {
                    id: hueTile
                    required property int index
                    readonly property int hue: index * 15
                    x: index * width
                    width: hues.width / 24
                    height: 20
                    Accessible.role: Accessible.Button
                    Accessible.name: "Hue " + hue
                    onClicked: panel.set({kind: "number", token: "Colors.seedHue"}, "", String(hue))
                    background: Rectangle {
                        color: Qt.hsla(hueTile.hue / 360, 0.7, 0.55, 1)
                        // The hue the theme starts from, ringed.
                        Rectangle {
                            visible: Math.round(hues.seed / 15) % 24 === hueTile.index
                            anchors.fill: parent
                            anchors.margins: 2
                            radius: 3
                            color: "transparent"
                            border.width: 2
                            border.color: Colors.surfaceRaised
                        }
                    }
                }
            }
        }

        Field {
            id: filter
            leadIcon: "search"
            placeholderText: "Filter tokens"
            Accessible.name: "Filter tokens"
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

                // The role strong, its group quiet, and a chip where the
                // theme file sets it rather than the module.
                RowLayout {
                    spacing: Spacing.xxs
                    Layout.fillWidth: true
                    Layout.topMargin: Spacing.xs
                    Text {
                        readonly property int dot: row.modelData.token.indexOf(".")
                        text: row.modelData.token.slice(0, dot + 1)
                            + "<b>" + row.modelData.token.slice(dot + 1) + "</b>"
                        textFormat: Text.StyledText
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        Accessible.role: Accessible.StaticText
                        Accessible.name: row.modelData.token
                    }
                    Rectangle {
                        visible: row.modelData.set
                        implicitWidth: themed.implicitWidth + Spacing.xs
                        implicitHeight: 16
                        radius: 8
                        color: Colors.surfaceSelected
                        Text {
                            id: themed
                            anchors.centerIn: parent
                            text: "theme"
                            color: Colors.link
                            font.pixelSize: Typography.captionSize
                        }
                    }
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
                        radius: 2
                        color: Colors.link
                        Accessible.ignored: true
                    }
                    Field {
                        text: row.modelData.value
                        unit: panel.unitOf(row.modelData.token)
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
                                implicitWidth: 28
                                implicitHeight: 28
                                radius: 6
                                color: variantRow.shown
                                // A swatch the colour of the panel still shows.
                                border.width: 1
                                border.color: Colors.border
                                Accessible.role: Accessible.Graphic
                                Accessible.name: row.modelData.token + " " + variantRow.modelData + " swatch"
                            }
                            Field {
                                text: variantRow.shown
                                leadIcon: variantRow.modelData === "light" ? "sun" : "moon"
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

        // What the theme refused, under the list in a place kept for it:
        // the list starts where it always does, and the row the person just
        // typed in stays under the pointer. Three lines at most; the whole
        // is its name.
        Label {
            id: refusal
            objectName: "tokensRefusal"
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            lineHeight: Typography.captionLineHeight
            lineHeightMode: Text.FixedHeight
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            Layout.fillWidth: true
            Layout.preferredHeight: 3 * Typography.captionLineHeight
            verticalAlignment: Text.AlignTop
        }
    }
}
