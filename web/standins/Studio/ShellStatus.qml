// What ljos-web's compiler knows of the native `Studio.ShellStatus`
// (`crates/studio_blocks/ljostypes.json`): only the native window, which
// watches a person's shell, uses it; a page's window does not.
import QtQml

QtObject {
    readonly property string reloadError
}
