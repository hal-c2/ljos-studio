# Sources:
#   docs/DESIGN.md (goal 1: the QML is the document; the sidecar is deletable; S6)
#   docs/MCP.md (export)
#   features/project/export-web.feature (the same export, for a site)
#   ljos-ui docs/ARCHITECTURE.md section 12.2 (production bundles) and
#   `ljos run` (a project directory and its entry document)

@disk
Feature: A project is exported as a bundle that runs without Studio
  An export writes what the app needs and nothing of Studio's: the
  project's QML, its theme, its assets and its own blocks, with a manifest
  that says what is in the bundle and how to start it. The mock modules
  that stand in for blocks in previews and Studio's sidecar never go in.
  What cannot be exported honestly is refused with the reason.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      """
    And the file "App/Media/Player.qml":
      """qml
      import QtQuick

      Item {
          property url source
          readonly property bool playing: false
          function play() {}
      }
      """
    And the file "mocks/App/Media/qmldir":
      """
      module App.Media
      Player 1.0 Player.qml
      """
    And the file "mocks/App/Media/Player.qml":
      """qml
      import QtQuick

      Item {
          property url source
          property bool playing: true
          function play() { playing = true }
      }
      """
    And the file "Shell.qml":
      """qml
      import QtQuick
      import App.Media

      Column {
          Player { id: player1 }
          Text { objectName: "status"; Accessible.role: Accessible.StaticText; text: player1.playing ? "Playing" : "Stopped" }
          Image { source: "images/logo.svg" }
      }
      """
    And the file "theme.json":
      """json
      {"colors": {}}
      """
    And the file "images/logo.svg":
      """
      <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>
      """
    And the file ".studio/review.json":
      """json
      {"version": 1, "comments": []}
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  Rule: A native bundle holds the app and says what it holds

    Scenario: The bundle has the QML, the theme, the assets and the live blocks
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/out" is "build/native"
      And the answer's "/manifest/target" is "native"
      And the answer's "/manifest/entry" is "Shell.qml"
      And the answer's "/manifest/run" is ["ljos", "run", "Shell.qml", "--project", "."]
      And the answer's "/manifest/theme" is "theme.json"
      And the answer's "/manifest/blocks" is [{"block": "App.Media.Player", "file": "App/Media/Player.qml"}]
      And the answer's "/manifest/files" is:
        """json
        [
          {"path": "App/Media/Player.qml", "bytes": 114},
          {"path": "App/Media/qmldir", "bytes": 73},
          {"path": "Shell.qml", "bytes": 232},
          {"path": "images/logo.svg", "bytes": 64},
          {"path": "theme.json", "bytes": 15}
        ]
        """
      And the answer's "/files" is 5
      And the answer's "/bytes" is 498
      And the file "build/native/Shell.qml" is:
        """qml
        import QtQuick
        import App.Media

        Column {
            Player { id: player1 }
            Text { objectName: "status"; Accessible.role: Accessible.StaticText; text: player1.playing ? "Playing" : "Stopped" }
            Image { source: "images/logo.svg" }
        }
        """
      And the file "build/native/ljos-bundle.json" exists
      And the file "build/native/mocks/App/Media/Player.qml" does not exist
      And the file "build/native/.studio/review.json" does not exist

    Scenario: An entry below the project's root names the theme Ljós reads for it
      Given the file "app/Main.qml":
        """qml
        import QtQuick

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "app/Main.qml"}
        """
      Then the answer's "/manifest/entry" is "app/Main.qml"
      And the answer's "/manifest/theme" is "theme.json"

    Scenario: The manifest says what was left out and why
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/manifest/left_out" is:
        """json
        [
          {"path": ".studio", "why": "Studio's sidecar"},
          {"path": "mocks", "why": "mock modules stand in for blocks in previews only"}
        ]
        """

    Scenario: A link out of the project is left out of the bundle
      Given "vendor" links to a directory outside the project that holds the file "Secret.qml"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/left_out" is:
        """json
        [
          {"path": ".studio", "why": "Studio's sidecar"},
          {"path": "mocks", "why": "mock modules stand in for blocks in previews only"},
          {"path": "vendor", "why": "a link out of the project"}
        ]
        """

    Scenario: A link to what the bundle leaves out is left out with it
      Given "shim" links to "mocks" in the project
      And "notes" links to ".studio" in the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/left_out" is:
        """json
        [
          {"path": ".studio", "why": "Studio's sidecar"},
          {"path": "mocks", "why": "mock modules stand in for blocks in previews only"},
          {"path": "notes", "why": "Studio's sidecar"},
          {"path": "shim", "why": "mock modules stand in for blocks in previews only"}
        ]
        """

    Scenario: Links that fan out and meet again are walked once, not once per way down
      Given a diamond of directory links 24 levels deep in "levels"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then it answered within 5 seconds
      And the answer's "/manifest/files" has 29 entries
      And the answer's "/manifest/links" has 46 entries
      And the answer's "/manifest/links/0" is {"path": "levels/0/left", "to": "levels/1"}
      And the answer's "/manifest/links/45" is {"path": "levels/9/right", "to": "levels/10"}
      And the answer's "/manifest/left_out" has 2 entries
      And the file "build/native/levels/0/left/level.txt" exists

    Scenario: A directory a link leads to is in the bundle under its own name, and the link goes in as a link
      Given the file "zone/Part.qml":
        """qml
        import QtQuick
        Item {}
        """
      And "alias" links to "zone" in the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/files" has 6 entries
      And the answer's "/manifest/files/5/path" is "zone/Part.qml"
      And the answer's "/manifest/left_out" is:
        """json
        [
          {"path": ".studio", "why": "Studio's sidecar"},
          {"path": "mocks", "why": "mock modules stand in for blocks in previews only"}
        ]
        """
      And the answer's "/manifest/links" is [{"path": "alias", "to": "zone"}]

    Scenario: A link to a directory it is in stays out of the bundle
      Given the file "zone/Part.qml":
        """qml
        import QtQuick
        Item {}
        """
      And "zone/up" links to "zone" in the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/links" is absent
      And the answer's "/manifest/left_out/2" is {"path": "zone/up", "why": "a link to `zone`, which it is in and the bundle holds already"}

    Scenario: A dry run says what the bundle would hold and writes nothing
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/entry" is "Shell.qml"
      And the answer's "/written" is false
      And the file "build/native/ljos-bundle.json" does not exist

    Scenario: An answer says whether the bundle it writes replaces one already there
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/replaces" is false
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/replaces" is false
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/replaces" is true

    Scenario: Exporting again replaces the last bundle
      Given the agent has called the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      And the agent has called the tool "property_set" with:
        """json
        {"doc": "Shell.qml", "node": {"id": "player1"}, "name": "source", "value": "a.mp4"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/written" is true
      And the file "build/native/Shell.qml" is:
        """qml
        import QtQuick
        import App.Media

        Column {
            Player { id: player1; source: "a.mp4" }
            Text { objectName: "status"; Accessible.role: Accessible.StaticText; text: player1.playing ? "Playing" : "Stopped" }
            Image { source: "images/logo.svg" }
        }
        """

    @preview
    Scenario: The bundle runs headless and shows what the live preview shows
      Given the studio tools are connected with the capabilities "inspect, mutate, run, fs"
      And the agent has called the tool "preview_open" with:
        """json
        {"doc": "Shell.qml", "state": "live"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the bundle "build/native", copied out of the project, runs headless and shows what the preview shows

    @preview
    Scenario: A bundle whose app imports a directory through a link runs
      Given the file "zone/Badge.qml":
        """qml
        import QtQuick
        Text { objectName: "badge"; Accessible.role: Accessible.StaticText; text: "From zone" }
        """
      And "alias" links to "zone" in the project
      And the file "Linked.qml":
        """qml
        import QtQuick
        import "alias"

        Column {
            Badge {}
        }
        """
      And the studio tools are connected with the capabilities "inspect, mutate, run, fs"
      And the agent has called the tool "preview_open" with:
        """json
        {"doc": "Linked.qml", "state": "live"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Linked.qml"}
        """
      Then the answer's "/manifest/links" is [{"path": "alias", "to": "zone"}]
      And the bundle "build/native", copied out of the project, runs headless and shows what the preview shows
      And the preview shows "From zone"

    Scenario: Deleting the sidecar changes nothing in the bundle
      Given the agent has called the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      And the agent keeps the answer's "/manifest/files" as "files"
      And the directory ".studio" is deleted
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/manifest/files" is "$files"

    Scenario: A named pipe is left out, not waited on
      Given the named pipe "images/feed"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/written" is true
      And the answer's "/manifest/left_out" is:
        """json
        [
          {"path": ".studio", "why": "Studio's sidecar"},
          {"path": "images/feed", "why": "not a regular file: a pipe, socket or device"},
          {"path": "mocks", "why": "mock modules stand in for blocks in previews only"}
        ]
        """
      And the file "build/native/images/logo.svg" exists

  Rule: What cannot be exported is refused with the reason

    Scenario: An export writes files and needs the capability to
      Given the studio tools are connected with the capabilities "inspect, mutate"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the agent is refused with "capability_denied"
      And the refusal's "/details/required" is "fs"

    Scenario: The entry must be a document of the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Missing.qml"}
        """
      Then the agent is refused with "not_found"

    Scenario: The entry must be in the bundle
      Given the file "mocks/Preview.qml":
        """qml
        import QtQuick

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "mocks/Preview.qml"}
        """
      Then the agent is refused with "not_actionable"
      And the refusal's "/details/kind" is "entry_left_out"
      And the refusal's "/details/why" is "mock modules stand in for blocks in previews only"
      And the file "build/native/ljos-bundle.json" does not exist

    Scenario: A document that does not parse is not shipped
      Given the file "Broken.qml":
        """qml
        Item {
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the agent is refused with "syntax"
      And the refusal's "/details/doc" is "Broken.qml"

    Scenario: Blocks written in Rust need the app's own program
      Given the file "native/ljostypes.json":
        """json
        {"modules": []}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "native_types"
      And the refusal's "/details/files" is ["native/ljostypes.json"]

    Scenario: A module that only a mock provides has nothing live to ship
      Given the file "mocks/App/Store/qmldir":
        """
        module App.Store
        Cart 1.0 Cart.qml
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the agent is refused with "not_actionable"
      And the refusal's "/details/kind" is "mocked_only"
      And the refusal's "/details/modules" is ["App.Store"]

    Scenario: A module only a mock provides is that still behind a link
      Given the file "mocks/App/Store/qmldir":
        """
        module App.Store
        Cart 1.0 Cart.qml
        """
      And "shim" links to "mocks" in the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the agent is refused with "not_actionable"
      And the refusal's "/details/kind" is "mocked_only"
      And the refusal's "/details/modules" is ["App.Store"]

    Scenario: A bundle is written below the project's build directory only
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "../elsewhere"}
        """
      Then the agent is refused with "path_denied"

    Scenario: A build directory that leads out of the project is refused
      Given "build" links to a directory outside the project that holds the file "ljos-bundle.json"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the agent is refused with "path_denied"
      And the directory outside the project holds only "ljos-bundle.json"

    Scenario: A place that is a link is refused however it is written
      Given "build/native" links to a directory outside the project that holds the file "ljos-bundle.json"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/native"}
        """
      Then the agent is refused with "path_denied"
      And the directory outside the project holds only "ljos-bundle.json"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/native/."}
        """
      Then the agent is refused with "path_denied"
      And the directory outside the project holds only "ljos-bundle.json"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/native/"}
        """
      Then the agent is refused with "path_denied"
      And the directory outside the project holds only "ljos-bundle.json"

    Scenario: A directory that holds something else is not overwritten
      Given the file "build/notes/todo.txt":
        """
        keep me
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/notes"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"
      And the file "build/notes/todo.txt" is:
        """
        keep me
        """

    Scenario: A dry run refuses what the write would, and writes nothing
      Given the file "build/notes/todo.txt":
        """
        keep me
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/notes", "dry_run": true}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"
      And the file "build/notes/ljos-bundle.json" does not exist

    Scenario: A file that is only named like a manifest does not make a bundle
      Given the file "build/notes/todo.txt":
        """
        keep me
        """
      And the file "build/notes/ljos-bundle.json":
        """
        my notes
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/notes"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"
      And the file "build/notes/todo.txt" is:
        """
        keep me
        """

    Scenario: A manifest that is a link does not make a bundle
      Given the agent has called the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      And the file "build/notes/todo.txt":
        """
        keep me
        """
      And the link "build/notes/ljos-bundle.json" leads to "build/native/ljos-bundle.json"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "out": "build/notes"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"
      And the file "build/notes/todo.txt" is:
        """
        keep me
        """

    Scenario: A half-written bundle's place that is a link is let go of, not emptied
      Given "build/.native.partial" links to a directory outside the project that holds the file "ljos-bundle.json"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      Then the answer's "/written" is true
      And the file "build/native/ljos-bundle.json" exists
      And the directory outside the project holds only "ljos-bundle.json"
