import QtQuick
import App.Media
import Ljos.Design

Column {
    Player { id: player1 }
    Text { color: Colors.text; text: player1.playing ? "Playing" : "Stopped" }
    Image { source: "images/logo.svg" }
}
