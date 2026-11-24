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

  Rule: What ljos-web has is exported, Ljós's own modules among them

    Scenario: A shell over Ljos.Blocks and a page over Ljos.Models go on the web
      Given the file "Shell.qml":
        """qml
        import QtQuick
        import Ljos.Blocks

        ShellLoader {}
        """
      And the file "Pages/Data.qml":
        """qml
        import QtQuick
        import Ljos.Models

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/manifest/imports" is ["Ljos.Blocks", "Ljos.Models", "QtQuick"]
      And the file "build/web/ljos-site.json" exists

  Rule: What cannot go on the web is refused with the reason

    Scenario: A Ljós module ljos-web has not got has no web face
      Given the file "Shell.qml":
        """qml
        import QtQuick
        import Ljos.Observer

        Item {}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "no_web_face"
      And the refusal's "/details/modules" is ["Ljos.Observer"]
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

    Scenario: A link to a module's directory is refused for the web, which would hold the module twice
      Given the file "App/Media/qmldir":
        """
        module App.Media
        Player 1.0 Player.qml
        """
      And the file "App/Media/Player.qml":
        """qml
        import QtQuick
        Item {}
        """
      And "media" links to "App/Media" in the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the agent is refused with "unsupported"
      And the refusal's "/details/kind" is "linked_module"
      And the refusal's "/details/module" is "App.Media"
      And the refusal's "/details/qmldir" is "App/Media/qmldir"
      And the refusal's "/details/through" is "media/qmldir"
      And the file "build/web/ljos-site.json" does not exist

    Scenario: The same link still exports natively, as a link
      Given the file "App/Media/qmldir":
        """
        module App.Media
        Player 1.0 Player.qml
        """
      And "media" links to "App/Media" in the project
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/links" is [{"path": "media", "to": "App/Media"}]

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
      And the refusal's message matches "no face on the web"

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

  # A public site is everything it holds: a file nothing names would be
  # served to whoever asks for it, and weighs on the upload. The code is
  # kept whole, since a page may load another by a name it makes.
  Rule: A site holds the project's code and only the files its code names

    Scenario: A file nothing names is left out of the site, saying why
      Given the file "images/unused.svg":
        """
        <svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>
        """
      And the file "notes.txt":
        """
        Ideas for the next page
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/manifest/left_out" has the entry:
        """json
        {"path": "images/unused.svg", "why": "nothing the site's code names"}
        """
      And the answer's "/manifest/left_out" has the entry:
        """json
        {"path": "notes.txt", "why": "nothing the site's code names"}
        """
      And the answer's "/manifest/files" has 5 entries
      And the file "build/web/images/logo.svg" exists
      And the file "build/web/images/unused.svg" does not exist

    Scenario: The pictures of a directory the code names by part of a path are in the site
      Given the file "icons/play.svg":
        """
        <svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>
        """
      And the file "icons/stop.svg":
        """
        <svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>
        """
      And the file "Shell.qml":
        """qml
        import QtQuick
        import App.Media

        Column {
            Player { id: player1 }
            Image { source: "icons/" + (player1.playing ? "stop" : "play") + ".svg" }
        }
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "dry_run": true}
        """
      Then the answer's "/manifest/files" has the entry:
        """json
        {"path": "icons/play.svg", "bytes": 64}
        """
      And the answer's "/manifest/files" has the entry:
        """json
        {"path": "icons/stop.svg", "bytes": 64}
        """
      And the answer's "/manifest/left_out" has the entry:
        """json
        {"path": "images/logo.svg", "why": "nothing the site's code names"}
        """

    Scenario: A native bundle still holds every file, since an app may read one by a path it makes
      Given the file "notes.txt":
        """text
        Ideas for the next page
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "dry_run": true}
        """
      Then the answer's "/manifest/files" has the entry:
        """json
        {"path": "notes.txt", "bytes": 24}
        """

    Scenario: The answer counts the manifest among the files it writes
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "dry_run": true}
        """
      Then the answer's "/files" is 5
      And the answer's "/writes" is 6

  Rule: A site has a title, a description and pictures of its own

    Scenario: The project's ljos-page.json gives the page its title, description and icon
      Given the file "ljos-page.json":
        """json
        {"title": "Media", "description": "Plays one source", "icon": "images/logo.svg"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/manifest/page" is {"title": "Media", "description": "Plays one source", "icon": "images/logo.svg"}
      And the answer's "/manifest/left_out" has the entry:
        """json
        {"path": "ljos-page.json", "why": "the page's title, description and pictures, which the manifest's page holds"}
        """
      And the file "build/web/ljos-page.json" does not exist

    Scenario: An export given the page writes it into the project's ljos-page.json
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "page": {"title": "Media", "image": "images/logo.svg"}}
        """
      Then the answer's "/manifest/page" is {"title": "Media", "image": "images/logo.svg"}
      And the file "ljos-page.json" is:
        """json
        {
          "title": "Media",
          "image": "images/logo.svg"
        }
        """

    Scenario: A dry run shows the page it was given and writes no ljos-page.json
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "dry_run": true, "page": {"title": "Media"}}
        """
      Then the answer's "/manifest/page/title" is "Media"
      And the file "ljos-page.json" does not exist

    # The page is the project's once the bundle that shows it is written: an
    # export that fails leaves the project's page as it was.
    Scenario: An export that fails leaves the project's ljos-page.json as it was
      Given the file "ljos-page.json":
        """json
        {"title": "Before"}
        """
      And the file "build/.web.partial":
        """
        not a directory
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "page": {"title": "After"}}
        """
      Then the agent is refused with "internal"
      And the file "ljos-page.json" is:
        """json
        {"title": "Before"}
        """

    # The project's own file of that name is config its code reads, not
    # the page: Studio's page has a name of Ljós's own.
    Scenario: A site.json of the project's own is a file of its site, not its page
      Given the file "site.json":
        """json
        {"feed": "https://example.org/feed.json"}
        """
      And the file "Shell.qml":
        """qml
        import QtQuick

        Item {
            property url settings: "site.json"
        }
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/written" is true
      And the answer's "/manifest/page" is absent
      And the file "build/web/site.json" is:
        """json
        {"feed": "https://example.org/feed.json"}
        """

    Scenario: The page is written beside a site.json of the project's own, which stays as it was
      Given the file "site.json":
        """json
        {"feed": "https://example.org/feed.json"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "page": {"title": "Media"}}
        """
      Then the answer's "/manifest/page" is {"title": "Media"}
      And the file "site.json" is:
        """json
        {"feed": "https://example.org/feed.json"}
        """
      And the file "ljos-page.json" is:
        """json
        {
          "title": "Media"
        }
        """

    Scenario: A picture of the page that is no file of the site is refused
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "page": {"icon": ".studio/review.json"}}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "page_file"
      And the refusal's "/details/path" is ".studio/review.json"
      And the file "ljos-page.json" does not exist
      And the file "build/web/ljos-site.json" does not exist

    # The page is saved only with the bundle: a refused export leaves the
    # page the next one reads as it was.
    Scenario: An export refused its place keeps the page it was given out of the project
      Given the file "ljos-page.json":
        """json
        {"title": "Before"}
        """
      And the file "build/web/notes.txt":
        """
        mine
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "page": {"title": "After"}}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"
      And the file "ljos-page.json" is:
        """json
        {"title": "Before"}
        """
      And the file "build/web/notes.txt" exists

  # Building needs ljos-web's compiler, runtime and bundler, which Studio
  # finds (DESIGN.md S6, as built: Studio builds the site); these say what
  # happens without them, which needs none of them to run.
  Rule: Studio builds the site when asked, and says what it lacks first

    Scenario: A build without ljos-web says what is missing and writes nothing
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "build": true, "ljos_web": "no-ljos-web"}
        """
      Then the agent is refused with "unavailable"
      And the refusal's "/details/kind" is "no_site_builder"
      And the refusal's "/details/missing/0" contains "no-ljos-web"
      And the file "build/web/ljos-site.json" does not exist

    Scenario: Checking a build gives a verdict and writes nothing
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "dry_run": true, "build": true, "ljos_web": "no-ljos-web"}
        """
      Then the answer's "/build/ready" is false
      And the answer's "/build/missing/0" contains "no-ljos-web"
      And the file "build/web/ljos-site.json" does not exist

    # A build runs the `qmlc` it is given and the code of the ljos-web it is
    # given, so neither is taken from outside the workspace: `run` is for
    # the project's own code and the build's own tools. LJOS_WEB and QMLC
    # are the machine's owner's, and are not held to it. A `..` after a link
    # goes up from where the link leads, as the system goes, not back to
    # where the link is.
    Scenario Outline: A toolchain from outside the workspace is refused before it is looked at
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      And "outside" links to a directory outside the project that holds the file "qmlc"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "build": true, <given>}
        """
      Then the agent is refused with "path_denied"
      And the refusal's "/details/kind" is "outside_workspace"
      And the refusal's "/details/argument" is "<argument>"
      And the file "build/web/ljos-site.json" does not exist

      Examples:
        | given                                  | argument |
        | "ljos_web": ".."                       | ljos_web |
        | "ljos_web": "/"                        | ljos_web |
        | "ljos_web": "outside"                  | ljos_web |
        | "ljos_web": "missing/../../ljos-web"   | ljos_web |
        | "ljos_web": "..", "dry_run": true      | ljos_web |
        | "qmlc": "outside/qmlc"                 | qmlc     |
        | "qmlc": "/bin/sh"                      | qmlc     |
        | "qmlc": "missing/../../qmlc"           | qmlc     |
        | "qmlc": "/bin/sh", "dry_run": true     | qmlc     |
        | "ljos_web": "outside/.."               | ljos_web |
        | "ljos_web": "outside/../ljos-web"      | ljos_web |
        | "qmlc": "outside/../qmlc"              | qmlc     |
        | "qmlc": "outside/../qmlc", "dry_run": true | qmlc |

    Scenario: Building a site runs a program, which needs the run capability
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "build": true}
        """
      Then the agent is refused with "capability_denied"
      And the file "build/web/ljos-site.json" does not exist

    Scenario Outline: What only a build uses is refused without one
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", <given>}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "needs_build"
      And the refusal's "/details/argument" is "<argument>"
      And the refusal's message matches "^`<argument>` needs `.build.: true`"
      And the file "build/web/ljos-site.json" does not exist

      Examples:
        | given                               | argument |
        | "open": true                        | open     |
        | "open": false                       | open     |
        | "open": true, "dry_run": true       | open     |
        | "ljos_web": "../ljos-web"           | ljos_web |
        | "qmlc": "qmlc"                      | qmlc     |
        | "page": {"title": "M"}, "open": true | open    |

    Scenario: Asking to open a native export names both arguments it needs
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "open": true}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "needs_build"
      And the refusal's message matches "^`open` needs `.build.: true` and `.target.: .web.`"
      And the file "build/native/ljos-bundle.json" does not exist

    Scenario: What a build is reading is not written over
      Given a site is being built from "build/web"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "page": {"title": "Second"}}
        """
      Then the agent is refused with "busy"
      And the refusal's "/details/kind" is "build_running"
      And the file "build/web/ljos-site.json" does not exist
      And the file "ljos-page.json" does not exist

    Scenario: A bundle is written again once its build has ended
      Given a site is being built from "build/web"
      When the build ends
      And the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      Then the answer's "/written" is true
      And the file "build/web/ljos-site.json" exists

    Scenario Outline: A bundle a build reads is the same bundle however it is spelled
      Given a site is being built from "build/web"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "<out>"}
        """
      Then the agent is refused with "<code>"
      And the file "build/web/ljos-site.json" does not exist

      Examples:
        | out             | code        |
        | build/./web     | busy        |
        | build//web/     | busy        |
        | build/web/.     | busy        |
        | build/web/inner | busy        |
        | build/WEB       | busy        |
        | ./build/web     | path_denied |
        | build/x/../web  | path_denied |

    Scenario: A link to a bundle a build reads is no way into it
      Given the file "build/web/keep.txt":
        """
        kept
        """
      And "build/alias" links to "build/web" in the project
      And a site is being built from "build/web"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "build/alias"}
        """
      Then the agent is refused with "path_denied"
      And the file "build/web/keep.txt" exists
      And the file "build/web/ljos-site.json" does not exist

    Scenario Outline: A build's own output is held before there is one
      Given a site is being built from "build/web"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "<target>", "out": "<out>"}
        """
      Then the agent is refused with "busy"
      And the refusal's "/details/kind" is "build_running"
      And the file "<out>" does not exist

      Examples:
        | target | out                  |
        | web    | build/web-site       |
        | web    | build/web-site/inner |
        | web    | build/WEB-site       |
        | native | build/web-site       |

    Scenario: A build's own output is held when it is an empty directory
      Given the empty directory "build/web-site"
      And a site is being built from "build/web"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "build/web-site"}
        """
      Then the agent is refused with "busy"
      And the refusal's "/details/kind" is "build_running"
      And the file "build/web-site/ljos-site.json" does not exist

    Scenario: A directory over a build's own output is held with it
      Given a site is being built from "build/group/web"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "build/group"}
        """
      Then the agent is refused with "busy"
      And the file "build/group/ljos-site.json" does not exist

    Scenario: A build's output is let go of with its build
      Given a site is being built from "build/web"
      When the build ends
      And the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "out": "build/web-site"}
        """
      Then the answer's "/written" is true
      And the file "build/web-site/ljos-site.json" exists

    # A build empties its work directory, and reads the copy of the bundle
    # it keeps there.
    Scenario Outline: Where builds work is no place for an export
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "<target>", "out": "<out>"<given>}
        """
      Then the agent is refused with "path_denied"
      And the refusal's "/details/kind" is "work_directory"
      And the file "<out>" does not exist

      Examples:
        | target | out                        | given            |
        | web    | build/.site-work           |                  |
        | web    | build/.site-work/site      |                  |
        | web    | build/.site-work/site      | , "dry_run": true |
        | web    | build/.SITE-WORK/site      |                  |
        | native | build/.site-work           |                  |
        | native | build/.site-work/deep/down |                  |
        | native | build/.Site-Work           | , "dry_run": true |

    Scenario Outline: Nothing is exported into a built site
      Given the file "build/web-site/.ljos-site-build":
        """
        {"files": {}}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "<target>", "out": "<out>"<given>}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "inside_site"
      And the file "<out>" does not exist

      Examples:
        | target | out                       | given            |
        | web    | build/web-site/inner      |                  |
        | web    | build/web-site/inner      | , "dry_run": true |
        | web    | build/web-site/inner/deep |                  |
        | native | build/web-site/inner      |                  |

    # A bundle's directory is replaced whole, and a site built below it
    # would go with it.
    Scenario Outline: A bundle with a built site below it is not replaced
      Given the file "build/group/ljos-site.json":
        """json
        {"format": "ljos-site"}
        """
      And the file "build/group/web-site/.ljos-site-build":
        """
        {"files": {}}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "<target>", "out": "build/group"<given>}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "holds_site"
      And the refusal's "/details/site" is "build/group/web-site"
      And the file "build/group/web-site/.ljos-site-build" exists

      Examples:
        | target | given             |
        | web    |                   |
        | web    | , "dry_run": true |
        | native |                   |

    # The runner's NODE is a stand-in: the toolchain is only looked at, and the
    # refusal comes before it runs.
    Scenario: A build waits for the bundle that lies where its site would go
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      And the file "standin/packages/runtime/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/ljos-web/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/solid-js/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/vite/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/@solidjs/web/package.json":
        """
        {}
        """
      And the file "standin/qmlc":
        """
        not run
        """
      And a site is being built from "build/web-site/inner"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "build": true, "ljos_web": "standin", "qmlc": "standin/qmlc"}
        """
      Then the agent is refused with "busy"
      And the refusal's "/details/kind" is "build_running"
      And the file "build/web/ljos-site.json" does not exist

    # The runner's NODE is a stand-in that does not start, so these builds
    # fail as one on any machine can after its bundle is written: the
    # refusal still says what the export wrote.
    Scenario: A build that fails says the bundle it wrote
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      And the file "standin/packages/runtime/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/ljos-web/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/solid-js/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/vite/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/@solidjs/web/package.json":
        """
        {}
        """
      And the file "standin/qmlc":
        """
        not run
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "build": true, "ljos_web": "standin", "qmlc": "standin/qmlc"}
        """
      Then the agent is refused with "not_actionable"
      And the refusal's "/details/kind" is "build_failed"
      And the refusal's "/details/export/written" is true
      And the refusal's "/details/export/replaces" is false
      And the refusal's "/details/export/out" is "build/web"
      And the file "build/web/ljos-site.json" exists

    Scenario: A build that fails says the bundle it wrote replaced the one before
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      And the file "standin/packages/runtime/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/ljos-web/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/solid-js/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/vite/package.json":
        """
        {}
        """
      And the file "standin/examples/web/node_modules/@solidjs/web/package.json":
        """
        {}
        """
      And the file "standin/qmlc":
        """
        not run
        """
      And the agent has called the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web"}
        """
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "target": "web", "build": true, "ljos_web": "standin", "qmlc": "standin/qmlc"}
        """
      Then the agent is refused with "not_actionable"
      And the refusal's "/details/kind" is "build_failed"
      And the refusal's "/details/export/written" is true
      And the refusal's "/details/export/replaces" is true
      And the refusal's "/details/export/out" is "build/web"
      And the file "build/web/ljos-site.json" exists

    Scenario: Only a web export is built
      Given the studio tools are connected with the capabilities "inspect, mutate, fs, run"
      When the agent calls the tool "export" with:
        """json
        {"entry": "Shell.qml", "build": true}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "build_target"
