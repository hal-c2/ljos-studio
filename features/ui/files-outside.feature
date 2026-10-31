# Sources:
#   docs/DOGFOOD.md (D16: a file made outside Studio showed only after the next tool call)
#   features/project/changed-outside.feature (what a change outside does to the session)

@ui
Feature: The window hears of files other programs make
  Other programs write the project too: an editor, an agent's shell, `git
  checkout`. The operating system says when; nothing polls. So the documents
  panel follows the directory with nobody calling a tool or touching the
  window.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project

  Scenario: A file made outside shows up by itself
    When the file "Page.qml" is written outside Studio:
      """qml
      import QtQuick
      Item {}
      """
    Then the documents panel lists "Page.qml" within 5 seconds

  Scenario: A file in a new directory shows up too
    When the file "pages/deep/Page.qml" is written outside Studio:
      """qml
      import QtQuick
      Item {}
      """
    Then the documents panel lists "pages/deep/Page.qml" within 5 seconds

  Scenario: A file removed outside leaves by itself
    Given the file "Page.qml":
      """qml
      import QtQuick
      Item {}
      """
    And Studio's window shows the project
    When the file "Page.qml" is deleted
    Then the documents panel does not list "Page.qml" within 5 seconds
