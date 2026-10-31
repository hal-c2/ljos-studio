# Sources:
#   docs/DESIGN.md (S6, as built: a site for the web; the contract)
#   docs/MCP.md (export, `target: "web"`, `ljos-site.json`)
#   ljos-web README.md at bf49026 (`qmlc`, its Vite plugin, `mount`)
#   crates/studio_mcp/tests/site/mod.rs (a site builder held to the contract)

@disk
Feature: A project is exported for ljos-web to build a site from
  A web export is the same walk of the project as a native one, written
  for a site builder instead of for `ljos run`: the project's QML, theme,
  assets and own modules, with a manifest (`ljos-site.json`) that says
  which document the site starts with, where each of the project's modules
  is, what it needs from ljos-web and which ljos-web it was checked
  against. Compiling and bundling are ljos-web's; choosing the files and
  refusing what cannot go on the web are Studio's.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      Player 1.0 Player.qml
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
          property bool playing: true
      }
      """
    And the file "Shell.qml":
      """qml
      import QtQuick
      import App.Media
      import Ljos.Design

      Column {
          Player { id: player1 }
          Text { color: Colors.text; text: player1.playing ? "Playing" : "Stopped" }
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

  Rule: A site bundle holds the app and says what ljos-web needs to build it

    Scenario: The site bundle has the QML, the theme, the assets and the project's modules
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/out" is "build/web"
      And the answer's "/manifest/format" is "ljos-site"
      And the answer's "/manifest/version" is 1
      And the answer's "/manifest/target" is "web"
      And the answer's "/manifest/entry" is "Shell.qml"
      And the answer's "/manifest/theme" is "theme.json"
      And the answer's "/manifest/modules" is [{"module": "App.Media", "qmldir": "App/Media/qmldir"}]
      And the answer's "/manifest/imports" is ["Ljos.Design", "QtQuick"]
      And the answer's "/manifest/run" is absent
      And the answer's "/manifest/files" is:
        """json
        [
          {"path": "App/Media/Player.qml", "bytes": 114},
          {"path": "App/Media/qmldir", "bytes": 95},
          {"path": "Shell.qml", "bytes": 209},
          {"path": "images/logo.svg", "bytes": 64},
          {"path": "theme.json", "bytes": 15}
        ]
        """
      And the answer's "/manifest/left_out" is:
        """json
        [
          {"path": ".studio", "why": "Studio's sidecar"},
          {"path": "mocks", "why": "mock modules stand in for blocks in previews only"}
        ]
        """
      And the file "build/web/ljos-site.json" exists
      And the file "build/web/ljos-bundle.json" does not exist
      And the file "build/web/mocks/App/Media/Player.qml" does not exist

    Scenario: The manifest names the ljos-web it was checked against
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/manifest/ljos_web" names the ljos-web commit Studio is pinned to

    Scenario: A site builder that holds to the contract builds it
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then a site builder held to the contract builds "build/web", starting at "Shell.qml"

    Scenario: A directory a link leads to is in the site as plain files under the link's name
      Given the file "zone/Badge.qml":
        """qml
        import QtQuick
        Text { text: "From zone" }
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
      When the agent calls the tool "export" with:
        """json
        {"entry": "Linked.qml", "target": "web"}
        """
      Then the answer's "/manifest/links" is absent
      And the answer's "/manifest/files/4/path" is "alias/Badge.qml"
      And the answer's "/manifest/files/7/path" is "zone/Badge.qml"
      And a site builder held to the contract builds "build/web", starting at "Linked.qml"
      And the file "build/web/alias/Badge.qml" exists

    Scenario: A dry run says what the site bundle would hold and writes nothing
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "dry_run": true}
        """
      Then the answer's "/manifest/format" is "ljos-site"
      And the answer's "/written" is false
      And the file "build/web/ljos-site.json" does not exist

    Scenario: An export for the web leaves the documents as they were
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the file "Shell.qml" is:
        """qml
        import QtQuick
        import App.Media
        import Ljos.Design

        Column {
            Player { id: player1 }
            Text { color: Colors.text; text: player1.playing ? "Playing" : "Stopped" }
            Image { source: "images/logo.svg" }
        }
        """
      When the agent calls the tool "project_open"
      Then the answer's "/documents" is ["App/Media/Player.qml", "Shell.qml", "mocks/App/Media/Player.qml"]
      When the agent calls the tool "history" with:
        """json
        {"action": "list"}
        """
      Then the answer's "/done" is []

    Scenario: A native bundle and a site bundle sit side by side
      Given the agent has called the tool "export" with:
        """json
        {"entry": "Shell.qml"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the file "build/native/ljos-bundle.json" exists
      And the file "build/web/ljos-site.json" exists

    Scenario: Exporting for the web again replaces the last site bundle
      Given the agent has called the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      And the agent has called the tool "property_set" with:
        """json
        {"doc": "Shell.qml", "node": {"id": "player1"}, "name": "source", "value": "a.mp4"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/written" is true
      And the file "build/web/Shell.qml" is:
        """qml
        import QtQuick
        import App.Media
        import Ljos.Design

        Column {
            Player { id: player1; source: "a.mp4" }
            Text { color: Colors.text; text: player1.playing ? "Playing" : "Stopped" }
            Image { source: "images/logo.svg" }
        }
        """

  Rule: What cannot go on the web is refused with the reason

    Scenario: A shell over Ljos.Blocks has no web face yet
      Given the file "Shell.qml":
        """qml
        import QtQuick
        import Ljos.Blocks

        ShellLoader {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "no_web_face"
      And the refusal's "/details/modules" is ["Ljos.Blocks"]
      And the refusal's "/details/docs" is ["Shell.qml"]
      And the file "build/web/ljos-site.json" does not exist

    Scenario: A module nothing provides is refused before ljos-web is asked
      Given the file "Pages/Cart.qml":
        """qml
        import QtQuick
        import App.Store

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "dry_run": true}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "no_web_face"
      And the refusal's "/details/modules" is ["App.Store"]
      And the refusal's "/details/docs" is ["Pages/Cart.qml"]

    Scenario: A Ljós module written in Rust has no web face
      Given the file "Pages/Data.qml":
        """qml
        import QtQuick
        import Ljos.Models

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "no_web_face"
      And the refusal's "/details/modules" is ["Ljos.Models"]

    Scenario: Links that would copy the same directories along too many ways are refused
      Given a diamond of directory links 24 levels deep in "levels"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then it answered within 5 seconds
      And the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "links_multiply"
      And the file "build/web/ljos-site.json" does not exist

    Scenario: Links that only lead to more links along too many ways are refused too
      Given a diamond of directory links 24 levels deep in "levels" holding no files
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then it answered within 2 seconds
      And the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "links_multiply"
      And the file "build/web/ljos-site.json" does not exist

    Scenario: The same project still exports natively
      Given the file "Pages/Data.qml":
        """qml
        import QtQuick
        import Ljos.Models

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/target" is "native"

    Scenario: Blocks written in Rust have no web face
      Given the file "native/ljostypes.json":
        """json
        {"modules": []}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "native_types"
      And the refusal's "/details/files" is ["native/ljostypes.json"]

    Scenario: A module that only a mock provides has nothing to put on the web
      Given the file "mocks/App/Store/qmldir":
        """
        module App.Store
        Cart 1.0 Cart.qml
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "not_actionable"
      And the refusal's "/details/kind" is "mocked_only"
      And the refusal's "/details/modules" is ["App.Store"]

    Scenario: A document that does not parse is not put on the web
      Given the file "Broken.qml":
        """qml
        Item {
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "syntax"
      And the refusal's "/details/doc" is "Broken.qml"

    Scenario: A site bundle is written below the project's build directory only
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "site"}
        """
      Then the agent is refused with "path_denied"

    Scenario: A directory that holds a built site is not overwritten
      Given the file "build/web-site/index.html":
        """
        <!doctype html>
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "build/web-site"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"
      And the file "build/web-site/index.html" is:
        """
        <!doctype html>
        """
