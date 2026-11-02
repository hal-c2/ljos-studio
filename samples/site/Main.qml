import QtQuick
import Ljos.Design

Rectangle {
    id: root
    width: 1280
    height: 800
    color: Colors.canvas

    // The breakpoints: every section reads these two, so a state is the one
    // place that says what a phone, a tablet or a desktop gets.
    property int columns: 3
    property bool compact: false
    readonly property real gutter: compact ? Spacing.md : Spacing.xxl

    Flickable {
        id: page
        anchors.fill: parent
        contentHeight: sections.height
        clip: true

        Column {
            id: sections
            width: page.width

            NavBar { compact: root.compact; gutter: root.gutter }
            Hero { compact: root.compact; gutter: root.gutter }
            Features { columns: root.columns; gutter: root.gutter }
            Testimonial { compact: root.compact; gutter: root.gutter }
            Pricing { columns: root.columns; gutter: root.gutter }
            CallToAction { compact: root.compact; gutter: root.gutter }
            Footer { compact: root.compact; gutter: root.gutter }
        }
    }

    states: [
        State {
            name: "phone"
            when: root.width < 600
            PropertyChanges { target: root; columns: 1; compact: true }
        },
        State {
            name: "tablet"
            when: root.width >= 600 && root.width < 1024
            PropertyChanges { target: root; columns: 2; compact: false }
        },
        State {
            name: "desktop"
            when: root.width >= 1024
            PropertyChanges { target: root; columns: 3; compact: false }
        }
    ]
}
