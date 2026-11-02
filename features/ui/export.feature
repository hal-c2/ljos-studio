# Sources:
#   docs/DESIGN.md (S6: export bundles, a site for the web; the window's
#   export panel)
#   features/project/export.feature, features/project/export-web.feature
#   (the same `export` call over MCP; the files on disk are the oracle for
#   both)

@ui
Feature: A person exports a native app or a web site from Studio's window
  The export panel is the `export` call an agent makes. Opening it asks for
  a dry run, so the files the bundle would hold, what is left out and why,
  and whether it replaces a bundle already there are shown before anything
  is written. Writing the bundle shows where it is and how to start it,
  and a refusal shows the tool's own message.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Image {
          source: "images/logo.svg"
      }
      """
    And the file "images/logo.svg":
      """
      <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>
      """
    And the file "theme.json":
      """json
      {"Spacing": {"md": 20}}
      """
    And the file "mocks/Stub.qml":
      """qml
      import QtQuick

      Item {}
      """
    And Studio's window shows the project

  Scenario: The toolbar opens the panel, which lists what the bundle would hold
    When the person clicks the button "Export"
    Then the export panel says "^Would write 3 files, \d+ bytes, to build/native$"
    And the export panel says "^File Main.qml \(\d+ bytes\)$"
    And the export panel says "^File images/logo.svg \(\d+ bytes\)$"
    And the export panel says "^Theme theme.json$"
    And the export panel says "^Left out mocks: mock modules stand in for blocks in previews only$"
    And the file "build/native/ljos-bundle.json" does not exist

  Scenario: Ctrl+E opens the panel and Escape closes it
    When the person presses "Ctrl+E"
    Then the export panel says "^Would write 3 files"
    When the person presses "Escape"
    Then the export panel is closed

  Scenario: Writing the bundle writes the manifest and says how to start it
    Given the person has clicked the button "Export"
    When the person clicks the button "Write bundle"
    Then the file "build/native/ljos-bundle.json" exists
    And the file "build/native/Main.qml" exists
    And the export panel says "^Wrote 3 files, \d+ bytes, to build/native$"
    And the export panel says "^Start it with: ljos run Main.qml --project \.  \(in /.*/build/native\)$"
    And the export panel's button "Copy path" is enabled

  Scenario: The panel warns before it replaces a bundle that is already there
    Given the person has clicked the button "Export"
    And the person has clicked the button "Write bundle"
    When the person clicks the button "Close"
    And the person clicks the button "Export"
    Then the export panel says "^build/native already holds a bundle: exporting replaces it$"
    And the export panel's button "Copy path" is disabled

  Scenario: An output directory outside build is refused with the tool's message
    Given the person has clicked the button "Export"
    When the person sets the output directory to "elsewhere" in the export panel
    Then the export panel says "`elsewhere` is not below the project's `build/`, where bundles are written"
    And the file "elsewhere/ljos-bundle.json" does not exist

  Scenario: The web target writes the site manifest and names the next step
    Given the person has clicked the button "Export"
    And the person has chosen "Web site" in the export panel
    When the person clicks the button "Write bundle"
    Then the file "build/web/ljos-site.json" exists
    And the file "build/native/ljos-bundle.json" does not exist
    And the export panel says "^Wrote 3 files, \d+ bytes, to build/web$"
    And the export panel says "^Build the site with: mise run export:web:build /.*/build/web$"

  Scenario: Typing a directory that holds a bundle warns before anything is written
    Given the person has clicked the button "Export"
    And the person has set the output directory to "build/old" in the export panel
    And the person has clicked the button "Write bundle"
    When the person types the output directory "build/native" in the export panel
    Then the export panel says "^Would write 3 files, \d+ bytes, to build/native$"
    When the person types the output directory "build/old" in the export panel
    Then the export panel says "^build/old already holds a bundle: exporting replaces it$"

  Scenario: Copy path gives the bundle's absolute path
    Given the person has clicked the button "Export"
    And the person has clicked the button "Write bundle"
    When the person clicks the button "Copy path"
    And the person pastes into the output directory in the export panel
    Then the export panel's output directory reads "^/.*/build/native$"

  Scenario: A directory half typed is not refused until typing pauses
    Given the person has clicked the button "Export"
    When the person is typing the output directory "bu" in the export panel
    Then the export panel does not say "is not below the project's"
    And the export panel's button "Write bundle" is disabled
    When the person pauses typing in the export panel
    Then the export panel says "`bu` is not below the project's `build/`"
    And the export panel's button "Write bundle" is enabled
    And the window shows no refusal

  Scenario: The next step quotes a path with a space in it
    Given the person has clicked the button "Export"
    And the person has chosen "Web site" in the export panel
    And the person has set the output directory to "build/my site" in the export panel
    When the person clicks the button "Write bundle"
    Then the export panel says "^Build the site with: mise run export:web:build '/.*/build/my site'$"

  Scenario: A list the answer's size cuts short says so
    Given 900 files the bundle holds
    And the person has clicked the button "Export"
    Then the export panel says "^Would write 90\d files"
    And the export panel says "^This list is cut short: the bundle holds all 90\d files, and its manifest lists them and what is left out$"

  Scenario: A short list says nothing about being cut
    When the person clicks the button "Export"
    Then the export panel does not say "cut short"
