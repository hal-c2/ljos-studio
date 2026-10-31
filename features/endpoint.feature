# Sources:
#   docs/DESIGN.md (Guarding the loopback endpoint)
#   docs/MCP.md (capabilities)

Feature: The running Studio's endpoint answers its own client only
  While the window is open Studio serves its tools on this machine, where
  they can rewrite a person's QML. So the endpoint listens on loopback, each
  start makes a token that only its owner can read, a web page that can reach
  localhost is turned away, and what a client may do was granted at launch.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {}
      """

  Rule: While Studio serves agents only its own client is answered

    Background:
      Given Studio serves agents with the capabilities "inspect"

    Scenario: The endpoint is on this machine and its token is its owner's
      Then the endpoint listens on loopback only
      And the token file can be read by its owner alone

    Scenario: A client with the token is answered
      When a client calls the tool "blocks_list" on the endpoint with the token
      Then the endpoint answers it

    Scenario: A client with no token is refused
      When a client calls the tool "blocks_list" on the endpoint
      Then the endpoint refuses it with 401 "unauthorized"

    Scenario: A client with another token is refused
      When a client calls the tool "blocks_list" on the endpoint with another token
      Then the endpoint refuses it with 401 "unauthorized"

    Scenario: A web page from another site is refused, token or not
      When a client calls the tool "blocks_list" on the endpoint with the token and the Origin "https://evil.example"
      Then the endpoint refuses it with 403 "forbidden_origin"

    Scenario: A request addressed to a name that only resolves here is refused
      When a client calls the tool "blocks_list" on the endpoint with the token and the Host "evil.example"
      Then the endpoint refuses it with 403 "forbidden_host"

    Scenario: A capability not granted at launch is refused
      When a client calls the tool "node_remove" on the endpoint with the token
      Then the endpoint's tool refuses it with "capability_denied"
      And the file "Main.qml" is:
        """qml
        import QtQuick

        Item {}
        """

  Rule: A Studio that cannot serve agents says why instead

    Scenario: The directory for the token is open to others
      Given the directory for the endpoint's token can be entered by others
      When Studio starts serving agents with the capabilities "inspect"
      Then Studio cannot serve agents, saying "others can get into it"
      And no token file is written
