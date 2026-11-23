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
  is written. What it says it writes is what it writes, manifest and all.
  Writing the bundle shows where it is and how to start it, and a refusal
  shows the tool's own message. A web site has a page of its own, and
  Studio builds it and opens it in the person's browser.

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
    Then the export panel says "^Would write 4 files to build/native: 3 of the project's \(\d+ bytes\) and ljos-bundle.json$"
    And the export panel says "^Manifest ljos-bundle.json$"
    And the export panel says "^File Main.qml \(\d+ bytes\)$"
    And the export panel says "^File images/logo.svg \(\d+ bytes\)$"
    And the export panel says "^Theme theme.json$"
    And the export panel says "^Left out mocks: mock modules stand in for blocks in previews only$"
    And the export panel says "^Ready to write$"
    And the file "build/native/ljos-bundle.json" does not exist

  Scenario: Ctrl+E opens the panel and Escape closes it
    When the person presses "Ctrl+E"
    Then the export panel says "^Would write 4 files"
    When the person presses "Escape"
    Then the export panel is closed

  Scenario: Writing the bundle writes the manifest and says how to start it
    Given the person has clicked the button "Export"
    When the person clicks the button "Write bundle"
    Then the file "build/native/ljos-bundle.json" exists
    And the file "build/native/Main.qml" exists
    And the export panel says "^Wrote 4 files to build/native: 3 of the project's \(\d+ bytes\) and ljos-bundle.json$"
    And the directory "build/native" holds 4 files
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
    And the export panel says "^Wrote 4 files to build/web: 3 of the project's \(\d+ bytes\) and ljos-site.json$"

  Scenario: Typing a directory that holds a bundle warns before anything is written
    Given the person has clicked the button "Export"
    And the person has set the output directory to "build/old" in the export panel
    And the person has clicked the button "Write bundle"
    When the person types the output directory "build/native" in the export panel
    Then the export panel says "^Would write 4 files to build/native"
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
    And the person has set the output directory to "build/my site" in the export panel
    When the person clicks the button "Write bundle"
    Then the export panel says "^Start it with: ljos run Main.qml --project \.  \(in '/.*/build/my site'\)$"

  Scenario: A list the answer's size cuts short says so
    Given 900 files the bundle holds
    And the person has clicked the button "Export"
    Then the export panel says "^Would write 90\d files"
    And the export panel says "^This list is cut short: the bundle holds all 90\d files, and its manifest lists them and what is left out$"

  Scenario: A short list says nothing about being cut
    When the person clicks the button "Export"
    Then the export panel does not say "cut short"

  # Nothing else says the list goes on: its last row, cut in half above
  # the buttons, reads as the panel's own edge.
  Scenario: A list longer than the panel shows its scroll bar
    Given 900 files the bundle holds
    When the person clicks the button "Export"
    Then the export panel's list shows a scroll bar

  Scenario: A list the panel holds whole shows none
    When the person clicks the button "Export"
    Then the export panel's list shows no scroll bar

  Rule: A site holds only what its code names, and the panel says so

    Scenario: A picture nothing names is listed as left out and not written
      Given the file "images/hero.svg":
        """
        <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>
        """
      And the person has clicked the button "Export"
      When the person chooses "Web site" in the export panel
      Then the export panel says "^Would write 4 files to build/web: 3 of the project's \(\d+ bytes\) and ljos-site.json$"
      And the export panel says "^Left out images/hero.svg: nothing the site's code names$"
      When the person clicks the button "Write bundle"
      Then the directory "build/web" holds 4 files
      And the file "build/web/images/hero.svg" does not exist

  Rule: A site has a page of its own, kept in the project

    Scenario: The page's title and description are written into site.json
      Given the person has clicked the button "Export"
      And the person has chosen "Web site" in the export panel
      And the person has set "Page title" to "Media" in the export panel
      And the person has set "Page description" to "Plays one source" in the export panel
      When the person clicks the button "Write bundle"
      Then the file "site.json" exists
      And the export panel says "^Page Media: Plays one source$"
      When the person clicks the button "Close"
      And the person clicks the button "Export"
      And the person chooses "Web site" in the export panel
      Then the export panel's "Page title" reads "^Media$"

    Scenario: A page picture that is no file of the site is refused with the tool's message
      Given the person has clicked the button "Export"
      And the person has chosen "Web site" in the export panel
      When the person sets "Page icon" to "nowhere.svg" in the export panel
      Then the export panel says "nowhere.svg"
      And the file "site.json" does not exist

    Scenario: The page has no fields for a native app
      When the person clicks the button "Export"
      Then the export panel has no "Page title"

  Rule: Studio builds the site and says first whether it can

    Scenario: Checking says what a build lacks, and Build and open waits for it
      Given the person has clicked the button "Export"
      And the person has chosen "Web site" in the export panel
      When the person sets "ljos-web folder" to "no-ljos-web" in the export panel
      Then the export panel says "^Ready to write; to build the site here Studio needs: .*no-ljos-web"
      And the export panel's button "Build and open" is disabled
      And the file "build/web/ljos-site.json" does not exist

    Scenario: Writing the bundle builds nothing, whichever ljos-web folder is typed
      Given the person has clicked the button "Export"
      And the person has chosen "Web site" in the export panel
      And the person has set "ljos-web folder" to "no-ljos-web" in the export panel
      When the person clicks the button "Write bundle"
      Then the file "build/web/ljos-site.json" exists
      And the export panel says "^Wrote 4 files to build/web"

    Scenario: A native app is not built
      When the person clicks the button "Export"
      Then the export panel has no "Build and open"
