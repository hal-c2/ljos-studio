# Sources:
#   docs/DESIGN.md (S6, as built: a site for the web)
#   features/project/export-web.feature (what Studio hands over)
#   ljos-web README.md at bf49026 (`qmlc`, its Vite plugin, `mount`)
#
# Run by `mise run export:web:check` (`web/site.mjs`): Studio exports
# `web/site/project` for the web and builds the site itself (the tool
# `export` with `build`, through ljos-web's `qmlc` and Vite plugin), and
# headless Chromium loads it from a plain static server. It needs an
# ljos-web checkout, so neither ledger runs it.

@web @disk
Feature: A web export becomes a site any static server serves
  Studio hands ljos-web a directory and its manifest; ljos-web compiles
  the QML, bundles it with its runtime and writes static files. A browser
  then shows the project's entry as Studio's preview does, with the
  project's own modules and pictures and with nothing of Studio's.

  Background:
    Given the project "web/site/project", whose entry "Shell.qml" uses its own module "App.Media", "Ljos.Design" and a picture
    And it is exported with the tool "export", the target "web" and "build": true

  Scenario: The site shows the entry's text and picture
    When the site is opened from a static server, below a path of its own
    Then the page shows "Stopped"
    And the page draws the picture "images/logo.svg" 8 by 8 pixels
    And the text has the colour Ljos.Design gives text
    And the page logged no errors

  Scenario: The site is static files with addresses of its own
    When the site is opened from a static server, below a path of its own
    Then every request the page made was for a file of the site
    And the site has an "index.html"

  Scenario: The site holds nothing of Studio's
    Then no file of the site names "mocks/" or ".studio"
    And the page does not show "Playing", which only the mock module says

    And the site has no "images/unused.svg", which nothing names

  # The builder set the site before aside as `<out>.old`, and removed
  # whatever already had that name.
  Scenario: A build removes no directory beside the site
    Given the project had the file "build/web-site.old/keep.txt" before it was exported
    Then the project still has the file "build/web-site.old/keep.txt"

  # A window sizes the root it shows, so a breakpoint state turns on by the
  # window's width. A page is that window; the root kept its declared size, and
  # a phone saw the corner of a desktop page.
  Scenario: The entry's root takes the page's width, and a breakpoint turns on by it
    When the site is opened from a static server, below a path of its own
    Then the page shows "Wide"
    When the page is made 390 by 844 pixels
    Then the entry's root is 390 pixels wide and at least 844 tall
    And the page shows "Narrow"

  Scenario: The page has the project's title, description and icon
    When the site is opened from a static server, below a path of its own
    Then the page's title is "Media, from Studio"
    And the page's description is "A page Studio built"
    And the page's icon is "images/logo.svg", a file of the site

  Scenario: A link on the page is a link
    When the site is opened from a static server, below a path of its own
    Then the page has a link "Read the docs" to "https://example.com/docs"

  # The browser scrolls a document: its wheel, keys, scroll bar and "find"
  # are the reader's own, and a full-page picture is the whole page. The
  # root's Flickable scrolls in a window, where there is no document.
  Scenario: A page taller than the window scrolls as a document
    When the site is opened from a static server, below a path of its own
    Then the document is taller than the window
    And a full-page picture of it is taller than the window
    When the reader turns the mouse wheel down
    Then the window has scrolled down
    And the page shows "The end"
