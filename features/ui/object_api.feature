# Sources:
#   studio7 dogfood backlog P7-4 (an object's own properties, signals and
#   functions are written only in the code view; a handler longer than a
#   line has nowhere to go but one line)
#   docs/MCP.md (`node_insert` of a member of `parent`, `binding_set` of a
#   handler)

@ui
Feature: An object's own API is written in the inspector
  What an object declares of its own, the properties it keeps, the signals
  it sends and the functions it offers, is listed in a section of its own,
  and one more is added there by its kind, name, type and first value. A
  handler of one of its signals is written over as many lines as it takes.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 200; height: 100
          property int count: 0
          signal done(int code)
          Rectangle { id: inner; property string label: "x"; width: 10; height: 10 }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: What an object declares is listed, and not what its children declare
    When the person selects "Item root" in the layers panel
    Then the inspector lists the members "count, done"

  Scenario: A property is added with its type and first value
    When the person selects "Item root" in the layers panel
    And the person sets "Member name" to "total" in the inspector
    And the person sets "Member type" to "real" in the inspector
    And the person sets "Member value" to "1.5" in the inspector
    Then the file "Main.qml" says "property real total: 1.5"
    And the inspector lists the members "count, done, total"
    And the inspector's "Member name" holds ""

  Scenario: A signal is added with its arguments
    When the person selects "Item root" in the layers panel
    And the person chooses "Signal" in the inspector's "Member kind"
    And the person sets "Member name" to "picked" in the inspector
    And the person sets "Member arguments" to "string name, int index" in the inspector
    Then the file "Main.qml" says "signal picked(string name, int index)"

  Scenario: A function is added with what it does
    When the person selects "Item root" in the layers panel
    And the person chooses "Function" in the inspector's "Member kind"
    And the person sets "Member name" to "reset" in the inspector
    And the person sets "Member body" to "count = 0" in the inspector
    Then the file "Main.qml" says "function reset()"
    And the file "Main.qml" says "count = 0"

  Scenario: A name the language cannot take is said under the row, and nothing is written
    When the person selects "Item root" in the layers panel
    And the person sets "Member name" to "2total" in the inspector
    And the person clicks the button "Add the member"
    Then the inspector's "API" section says something with "that member does not fit"
    And the file "Main.qml" does not say "2total"
    And the inspector's "Member name" holds "2total"

  # What is typed while the member before it is being added is a new one,
  # and the answer about the one before does not empty it.
  Scenario: A member typed while the one before it is added is kept
    When the person selects "Item root" in the layers panel
    And the person sets "Member name" to "first" in the inspector
    And the person clicks the button "Add the member" and types "second" in the inspector's "Member name" before its answer
    Then the file "Main.qml" says "property var first"
    And the inspector's "Member name" holds "second"

  Scenario: A handler is written over several lines
    When the person selects "Item root" in the layers panel
    And the person clicks the button "Edit the done handler"
    And the person types "console.log(code)\ncount = code" in the inspector's "Handler for done" and leaves it
    And the person clicks the button "Write the done handler"
    Then the file "Main.qml" says "onDone: {"
    And the file "Main.qml" says "console.log(code)"
    And the file "Main.qml" says "count = code"

  Scenario: A handler already written opens with its own lines
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    And the person has typed "console.log(code)\ncount = code" in the inspector's "Handler for done" and left it
    And the person has clicked the button "Write the done handler"
    When the person clicks the button "Edit the done handler"
    Then the inspector's "Handler for done" holds "console.log(code)\ncount = code"

  # A member on the line the object closes on is the object's too.
  Scenario: A member written just before the closing brace is listed
    When the person replaces "height: 10 }" with "height: 10; signal tapped }" in the code view
    And the person selects "Rectangle inner" in the layers panel
    Then the inspector lists the members "label, tapped"

  # A brace in a regular expression is the expression's, not the object's.
  Scenario: A brace in a function's regular expression ends nothing
    When the person replaces "signal done(int code)" with "function hasBrace(s) { return /[}]/.test(s) }\n    signal done(int code)" in the code view
    And the person selects "Item root" in the layers panel
    Then the inspector lists the members "count, hasBrace, done"

  # `of` is a keyword only in a `for (… of …)`: a name `of` is a value, and
  # a `/` after it divides.
  Scenario: A `/` after a name `of` divides, and one after a `for … of` starts a regular expression
    When the person replaces "signal done(int code)" with "function half(of) { return of / 2 }\n    function parts(s) { for (const p of /[}]/[Symbol.split](s)) count++ }\n    signal done(int code)" in the code view
    And the person selects "Item root" in the layers panel
    Then the inspector lists the members "count, half, parts, done"

  # A `)` that ends the head of an `if`, a `while`, a `for` or a `with` ends
  # no value: a `/` after it starts a regular expression. One that ends a
  # call ends a value, and a `/` after it divides.
  Scenario: A brace in a regular expression after an `if` ends nothing
    When the person replaces "signal done(int code)" with "function hasBrace(s) { if (s) /[}]/.test(s) }\n    function half(s) { return Number(s) / 2 + \"}\" }\n    signal done(int code)" in the code view
    And the person selects "Item root" in the layers panel
    Then the inspector lists the members "count, hasBrace, half, done"

  # Inside a string the lines are the string's: the inspector trims none of
  # them. (Setting the value indents each later line by the handler's own
  # indent, inside the string too: that is qml_edit's, and not asserted.)
  Scenario: A handler's multi-line string keeps the spaces typed at a line's end
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    When the person types "const s = `a\n  b  \nc`\nconsole.log(s)" in the inspector's "Handler for done" and leaves it
    And the person clicks the button "Write the done handler"
    Then the file "Main.qml" says "b  \n"
    And the file "Main.qml" says "console.log(s)"

  # A `/` where a value starts begins a regular expression, and a quote or a
  # backtick in one starts no string: the lines after it stay the handler's
  # own, indented with it.
  Scenario: A backtick in a handler's regular expression starts no string
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    And the person has typed "const t = String(code).replace(/`/g, \"'\")\nconsole.log(t)" in the inspector's "Handler for done" and left it
    And the person has clicked the button "Write the done handler"
    When the person clicks the button "Edit the done handler"
    Then the inspector's "Handler for done" holds "const t = String(code).replace(/`/g, \"'\")\nconsole.log(t)"

  Scenario: A quote in a handler's regular expression starts no string
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    When the person types "const s = /\"/.test(String(code)) ? `a\n  b  \nc` : \"\"\nconsole.log(s)" in the inspector's "Handler for done" and leaves it
    And the person clicks the button "Write the done handler"
    Then the file "Main.qml" says "b  \n"

  # After a value a `/` divides: the string after it is still read as one.
  Scenario: A division in a handler is no regular expression
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    When the person types "const k = String(code).length / 2 + \"/\" + `a\n  b  \nc`\nconsole.log(k)" in the inspector's "Handler for done" and leaves it
    And the person clicks the button "Write the done handler"
    Then the file "Main.qml" says "b  \n"

  # A name `of` is a value: the string after its division is still read as one.
  Scenario: A division after a name `of` in a handler is no regular expression
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    When the person types "const of = 4\nconst k = of / 2 + `a\n  b  \nc`\nconsole.log(k)" in the inspector's "Handler for done" and leaves it
    And the person clicks the button "Write the done handler"
    Then the file "Main.qml" says "b  \n"

  # A `++` or a `--` after a value ends it: a `/` after it divides.
  Scenario: A division after a postfix `++` in a handler is no regular expression
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    When the person types "let n = code\nconst k = n++ / 2 + `a\n  b  \nc`\nconsole.log(k)" in the inspector's "Handler for done" and leaves it
    And the person clicks the button "Write the done handler"
    Then the file "Main.qml" says "b  \n"

  # What is typed while the handler is written is the next draft: the
  # editor stays open with it.
  Scenario: A handler typed while the one before it is written stays open with it
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    And the person has typed "count = 1" in the inspector's "Handler for done" and left it
    When the person clicks the button "Write the done handler" and types "count = 2" in the inspector's "Handler for done" before its answer
    Then the file "Main.qml" says "onDone: count = 1"
    And the inspector's "Handler for done" holds "count = 2"

  # Opened again with the lines it had, a handler is as the person left it:
  # what was typed in another handler before is not what it holds.
  Scenario: A handler opened again unchanged after another is typed in closes when written
    Given the person has replaced "signal done(int code)" with "signal done(int code)\n    signal tapped()\n    onDone: count = 1" in the code view
    And the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    And the person has clicked the button "Leave the done handler as it was"
    And the person has clicked the button "Edit the tapped handler"
    And the person has typed "count = 2" in the inspector's "Handler for tapped" and left it
    And the person has clicked the button "Leave the tapped handler as it was"
    And the person has clicked the button "Edit the done handler"
    When the person clicks the button "Write the done handler"
    Then the inspector has no button "Write the done handler"
    And the file "Main.qml" says "onDone: count = 1"

  # A word ends at a space or a line's end: `else` and `return` after a word
  # on the line before are still words that a regular expression follows.
  Scenario: A regular expression after `else return` starts no string
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    And the person has typed "const t = String(code)\nif (t) count = 1\nelse return /`/.test(t)\nconsole.log(t)" in the inspector's "Handler for done" and left it
    And the person has clicked the button "Write the done handler"
    When the person clicks the button "Edit the done handler"
    Then the inspector's "Handler for done" holds "const t = String(code)\nif (t) count = 1\nelse return /`/.test(t)\nconsole.log(t)"

  # A field taller than the window is scrolled until its top shows, and is
  # typed in there. (Last: a scenario that never ends stops the run.)
  Scenario: A handler taller than the window is typed in again
    Given the person has selected "Item root" in the layers panel
    And the person has clicked the button "Edit the done handler"
    And the person has typed 80 lines of "count = code" in the inspector's "Handler for done" and left it
    And the inspector's "Handler for done" is taller than the window
    When the person types "count = 1" in the inspector's "Handler for done" and leaves it
    Then the inspector's "Handler for done" holds "count = 1"
