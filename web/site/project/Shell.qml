import QtQuick
import App.Media
import Ljos.Design
import Site

// A landing page as Studio makes one: a Flickable that fills the window and
// holds a column as tall as what it shows, so it scrolls in a window and,
// as a site, as a document.
Item {
    id: root
    width: 800
    height: 600
    states: State {
        name: "narrow"
        when: root.width < 600
    }

    Flickable {
        id: page
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight

        Column {
            id: column
            width: page.width
            Player { id: player1 }
            Text { color: Colors.text; text: player1.playing ? "Playing" : "Stopped" }
            Image { source: "images/logo.svg" }
            Text { color: Colors.text; text: root.state === "narrow" ? "Narrow" : "Wide" }
            Link { text: "Read the docs"; href: "https://example.com/docs" }
            Rectangle { width: 8; height: 1600; color: Colors.surface }
            Text { color: Colors.text; text: "The end" }
        }
    }
}
