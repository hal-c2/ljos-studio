# Sources:
#   docs/MCP.md (project_open's assets)
#   docs/DESIGN.md (every path a tool takes is confined)

Feature: A project's assets
  Pictures and fonts live in `assets/`. An agent or a person brings one in by
  its path or, holding no file on this machine, as base64. Anything in use
  stays: taking it out would leave a document pointing at nothing. Reading the
  list needs `inspect`; writing a file that is not a document needs `fs`.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  # A picture of 1 by 1 pixels.
  Scenario: A picture comes in as base64 and says what it is
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "dot.png", "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg=="}
      """
    Then the answer's "/asset/name" is "dot.png"
    And the answer's "/asset/url" is "assets/dot.png"
    And the answer's "/asset/kind" is "image"
    And the answer's "/asset/width" is 1
    And the answer's "/asset/height" is 1
    And the file "assets/dot.png" exists
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "list"}
      """
    Then the answer's "/assets" has 1 entry
    And the answer's "/assets/0/url" is "assets/dot.png"
    And the answer's "/assets/0/bytes" is 70

  Scenario: An SVG says its size from the file
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "logo.svg", "data": "PHN2ZyB2aWV3Qm94PSIwIDAgMjAgMTAiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIvPg=="}
      """
    Then the answer's "/asset/kind" is "image"
    And the answer's "/asset/width" is 20
    And the answer's "/asset/height" is 10

  Scenario: A name already taken gets a number, whatever its case
    Given the agent has called the tool "project_open" with:
      """json
      {"asset": "import", "name": "logo.svg", "data": "PHN2Zy8+"}
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "LOGO.svg", "data": "PHN2Zy8+"}
      """
    Then the answer's "/asset/name" is "LOGO-1.svg"

  Scenario Outline: A name cannot lead anywhere, and comes out plain
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "<given>", "data": "PHN2Zy8+"}
      """
    Then the answer's "/asset/url" is "<url>"

    Examples:
      | given             | url                 |
      | ../../evil.svg    | assets/evil.svg     |
      | .hidden.svg       | assets/hidden.svg   |
      | my logo (1).svg   | assets/my-logo-1.svg |

  Scenario Outline: Other kinds are refused, saying which are taken
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "<name>", "data": "PHN2Zy8+"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "asset_kind"
    And the refusal's message matches "png"
    And the file "assets/<name>" does not exist

    Examples:
      | name       |
      | run.sh     |
      | Page.qml   |
      | noextension |

  Scenario: A file that is not what its name says is refused
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "fake.png", "data": "PHN2Zy8+"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "asset_kind"
    And the file "assets/fake.png" does not exist

  Scenario: Base64 that is not base64 is refused
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "a.svg", "data": "not base64!"}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: A reader lists but does not bring in or take out
    Given the file "assets/a.svg":
      """
      <svg/>
      """
    And the studio tools are connected with the capabilities "inspect, mutate"
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "list"}
      """
    Then the answer's "/assets" has 1 entry
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "name": "b.svg", "data": "PHN2Zy8+"}
      """
    Then the agent is refused with "capability_denied"
    And the refusal's "/details/required" is "fs"
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "a.svg"}
      """
    Then the agent is refused with "capability_denied"
    And the file "assets/a.svg" exists

  Scenario: What no document uses can be taken out
    Given the file "assets/a.svg":
      """
      <svg/>
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "a.svg"}
      """
    Then the answer's "/removed" is "assets/a.svg"
    And the file "assets/a.svg" does not exist

  Scenario Outline: A file a script names is in use too
    Given the file "<script>":
      """
      var logo = "assets/logo.png"
      """
    And the file "assets/logo.png":
      """
      not looked at
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "logo.png"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "asset_in_use"

    Examples:
      | script    |
      | util.js   |
      | util.mjs  |

  Scenario: What a document uses stays, and the refusal says which
    Given the file "assets/a.svg":
      """
      <svg/>
      """
    And the file "parts/Logo.qml":
      """qml
      import QtQuick

      Image { source: "../assets/a.svg" }
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "a.svg"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "asset_in_use"
    And the refusal's "/details/documents/0" is "parts/Logo.qml"
    And the file "assets/a.svg" exists

  Scenario: A name that only ends the same is not a use
    Given the file "assets/a.svg":
      """
      <svg/>
      """
    And the file "Other.qml":
      """qml
      import QtQuick

      Image { source: "myassets/a.svg.png" }
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "a.svg"}
      """
    Then the answer's "/removed" is "assets/a.svg"

  Scenario: Taking out what is not there says so
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "gone.svg"}
      """
    Then the agent is refused with "not_found"

  Scenario: A placed picture is an Image with its relative url
    Given the file "assets/a.svg":
      """
      <svg/>
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Image { source: \"assets/a.svg\" }"}
      """
    Then the file "Main.qml" contains "source: \"assets/a.svg\""

  @disk
  Scenario: A file of the project comes in by its path, bytes as they are
    Given the file "incoming/photo.svg":
      """
      <svg width="4" height="3"/>
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "file": "incoming/photo.svg"}
      """
    Then the answer's "/asset/url" is "assets/photo.svg"
    And the answer's "/asset/width" is 4
    And the answer's "/asset/height" is 3
    And the file "incoming/photo.svg" exists

  @disk
  Scenario: A file outside the workspace is refused and comes as base64 instead
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "file": "/etc/hostname"}
      """
    Then the agent is refused with "path_denied"

  @disk
  Scenario: A link out of the workspace does not bring a file in
    Given the link "pic.svg" leads to the file "secret.svg" outside the project
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "file": "pic.svg"}
      """
    Then the agent is refused with "path_denied"
    And the file "assets/pic.svg" does not exist

  @disk
  Scenario: A pipe is refused and does not hold the call
    Given the named pipe "in.svg"
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "import", "file": "in.svg"}
      """
    Then the agent is refused with "not_actionable"
    And it answered within 5 seconds

  @disk
  Scenario: A picture in a file that is not text is listed with its size
    Given the agent has called the tool "project_open" with:
      """json
      {"asset": "import", "name": "dot.png", "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg=="}
      """
    And the studio tools are reconnected
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "list"}
      """
    Then the answer's "/assets/0/width" is 1
    And the answer's "/assets/0/bytes" is 70

  @disk
  Scenario Outline: A bundle carries the assets, bytes as they are
    Given the agent has called the tool "project_open" with:
      """json
      {"asset": "import", "name": "dot.png", "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg=="}
      """
    When the agent calls the tool "export" with:
      """json
      {"entry": "Main.qml", "target": "<target>"}
      """
    Then the answer's "/manifest/files" has the entry:
      """json
      {"path": "assets/dot.png", "bytes": 70}
      """
    And the file "build/<target>/assets/dot.png" exists

    Examples:
      | target |
      | native |
      | web    |

  Scenario: Only an asset is taken out of the folder
    Given the file "assets/Thing.qml":
      """qml
      import QtQuick

      Item {}
      """
    When the agent calls the tool "project_open" with:
      """json
      {"asset": "remove", "name": "Thing.qml"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "asset_kind"
    And the file "assets/Thing.qml" exists
