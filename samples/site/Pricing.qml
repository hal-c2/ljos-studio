import QtQuick
import Ljos.Design

Item {
    id: pricing
    property int columns
    property real gutter

    width: parent.width
    height: body.height + 2 * Spacing.xxl

    Column {
        id: body
        x: pricing.gutter
        y: Spacing.xxl
        width: parent.width - 2 * pricing.gutter
        spacing: Spacing.xl

        Text {
            text: "Pricing"
            color: Colors.text
            font.pixelSize: Typography.headlineSize
            font.weight: Typography.headlineWeight
        }

        Grid {
            id: plans
            width: parent.width
            columns: pricing.columns
            spacing: Spacing.lg
            readonly property real cell: (width - (columns - 1) * spacing) / columns

            Repeater {
                model: [
                    { name: "Day walker", price: "Free", text: "One region, routes and huts.", best: false },
                    { name: "Trekker", price: "€4 a month", text: "Every region, weather windows and route sharing.", best: true },
                    { name: "Club", price: "€30 a month", text: "Ten trekkers, shared routes and a hut calendar.", best: false }
                ]
                Rectangle {
                    required property var modelData
                    width: plans.cell
                    height: Spacing.xxxl * 2 + Spacing.xl
                    radius: Spacing.xs
                    color: modelData.best ? Colors.surfaceSelected : Colors.surfaceRaised
                    border.color: modelData.best ? Colors.accent : Colors.border
                    border.width: modelData.best ? 2 : 1

                    Column {
                        anchors { fill: parent; margins: Spacing.lg }
                        spacing: Spacing.xs
                        Text {
                            text: modelData.name
                            color: Colors.textMuted
                            font.pixelSize: Typography.bodyStrongSize
                            font.weight: Typography.bodyStrongWeight
                        }
                        Text {
                            text: modelData.price
                            color: Colors.text
                            font.pixelSize: Typography.headlineSize
                            font.weight: Typography.headlineWeight
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: modelData.text
                            color: Colors.textMuted
                            font.pixelSize: Typography.bodySize
                        }
                    }
                }
            }
        }

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "Does it work abroad? Yes: download any region in the world before you go. Can I cancel? Any time, and your downloaded maps stay."
            color: Colors.textMuted
            font.pixelSize: Typography.bodySize
        }
    }
}
