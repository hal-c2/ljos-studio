import QtQuick
import Ljos.Design

Item {
    id: features
    property int columns
    property real gutter

    width: parent.width
    height: body.height + 2 * Spacing.xxl

    Column {
        id: body
        x: features.gutter
        y: Spacing.xxl
        width: parent.width - 2 * features.gutter
        spacing: Spacing.xl

        Text {
            text: "Made for the long way round"
            color: Colors.text
            font.pixelSize: Typography.headlineSize
            font.weight: Typography.headlineWeight
        }

        Grid {
            id: grid
            width: parent.width
            columns: features.columns
            spacing: Spacing.lg
            readonly property real cell: (width - (columns - 1) * spacing) / columns

            Repeater {
                model: [
                    { title: "Offline first", text: "Download a region once; routes, contours and huts work with no signal." },
                    { title: "Cairn to cairn", text: "Old waymarks are on the map, with photos from walkers who passed them." },
                    { title: "Weather windows", text: "See when the pass is clear before you set out, and get told if it turns." },
                    { title: "Share your route", text: "Leave your plan with someone at home; they see where you meant to be." },
                    { title: "Hut bookings", text: "Book mountain huts along the way and keep the receipt offline." },
                    { title: "Battery kind", text: "A dark map and no polling: a day on the trail takes a tenth of a charge." }
                ]
                Rectangle {
                    required property var modelData
                    width: grid.cell
                    height: Spacing.xxxl * 2 + Spacing.lg
                    radius: Spacing.xs
                    color: Colors.surfaceRaised
                    border.color: Colors.border

                    Column {
                        anchors { fill: parent; margins: Spacing.lg }
                        spacing: Spacing.xs
                        Text {
                            text: modelData.title
                            color: Colors.text
                            font.pixelSize: Typography.subtitleSize
                            font.weight: Typography.titleWeight
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
    }
}
