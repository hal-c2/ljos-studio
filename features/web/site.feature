# Sources:
#   docs/DESIGN.md (S6, as built: a site for the web)
#   features/project/export-web.feature (what Studio hands over)
#   ljos-web README.md at bf49026 (`qmlc`, its Vite plugin, `mount`)
#
# Run by `mise run export:web:check` (`web/site.mjs`): Studio exports
# `web/site/project` for the web, `mise run export:web:build` builds the
# site with ljos-web's `qmlc` and Vite plugin, and headless Chromium loads
# it from a plain static server. It needs an ljos-web checkout, so neither
# ledger runs it. The project's `theme.json` is carried into the site but
# not applied: ljos-web reads no theme file yet (DESIGN.md, request 20).

@web @disk
Feature: A web export becomes a site any static server serves
  Studio hands ljos-web a directory and its manifest; ljos-web compiles
  the QML, bundles it with its runtime and writes static files. A browser
  then shows the project's entry as Studio's preview does, with the
  project's own modules and pictures and with nothing of Studio's.

  Background:
    Given the project "web/site/project", whose entry "Shell.qml" uses its own module "App.Media", "Ljos.Design" and a picture
    And it is exported with the tool "export" and the target "web"
    And the site is built from "build/web" with `mise run export:web:build`

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
