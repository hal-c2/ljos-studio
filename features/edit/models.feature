# Sources:
#   docs/MCP.md (node_inspect's model, document_new's data)
#   docs/DESIGN.md (S6: CMS-like content is models)
#   https://doc.qt.io/qt-6/qml-qtqml-models-listmodel.html
#   ljos-ui features/models/json-list-model.feature (JsonListModel is Ljós's own)
#   https://doc.qt.io/qt-6/qtquick-modelviewsdata-modelview.html (roles in
#   a delegate, `model.role`)

Feature: An agent binds a list to its data
  A list's data is a model: written in the document, or a document of its
  own the list makes (`model: Contacts {}`). A JSON file of the project
  stays JSON: `document_new` (`data`) makes a document that reads it, a
  `JsonListModel` whose `source` names it relative to that document
  (`Qt.resolvedUrl`, so it is that file's whoever makes the model), and
  whatever writes the file goes on owning it. Data a person means to edit as QML is copied in
  instead (`"model": "list"`), a `ListModel` with one `ListElement` for
  each of its records. The list is bound with `binding_set`. `node_inspect`
  lists the roles of the data an object sees: a model's own (a
  `JsonListModel`'s are the keys of its file's records, read when asked),
  a list's model's, and, inside a list's delegate, the list's, which a
  delegate's bindings read as `model.name`.

  Background:
    Given the file "Contacts.qml":
      """qml
      import QtQuick

      ListModel {
          ListElement {
              name: "Ada"
              email: "ada@example.com"
          }
          ListElement {
              name: "Grace"
              team: "Navy"
          }
      }
      """
    And the file "Main.qml":
      """qml
      import QtQuick

      Column {
          id: root

          Repeater {
              id: people
              Text {
                  id: row
              }
          }
      }
      """
    And the studio tools are connected

  Rule: Roles come from the data

    Scenario: A ListModel lists its roles and rows
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Contacts.qml", "node": {"path": "root"}}
        """
      Then the answer's "/model/roles" lists "name, email, team"
      And the answer's "/model/rows" is 2
      And the answer's "/model/doc" is "Contacts.qml"

    Scenario: A list bound to a model document lists its roles
      Given the agent has called the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}, "name": "model", "value": "Contacts {}"}
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}}
        """
      Then the answer's "/model/roles" lists "name, email, team"
      And the answer's "/model/doc" is "Contacts.qml"
      And the answer's "/model/view" is absent

    Scenario: A delegate sees its list's roles
      Given the agent has called the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}, "name": "model", "value": "Contacts {}"}
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "row"}}
        """
      Then the answer's "/model/roles" lists "name, email, team"
      And the agent keeps the answer's "/model/view" as "view"
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}}
        """
      Then the answer's "/node" is "$view"

    Scenario: A role is bound in the delegate
      Given the agent has called the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}, "name": "model", "value": "Contacts {}"}
        """
      When the agent calls the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "row"}, "name": "text", "value": "model.name"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Column {
            id: root

            Repeater {
                id: people
                model: Contacts {}
                Text {
                    id: row
                    text: model.name
                }
            }
        }
        """

    Scenario: A model written in the document by its id
      Given the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "ListModel { id: tags; ListElement { label: \"new\" } }", "id": false}
        """
      And the agent has called the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}, "name": "model", "value": "tags"}
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "row"}}
        """
      Then the answer's "/model/roles" lists "label"
      And the answer's "/model/rows" is 1
      And the answer's "/model/doc" is "Main.qml"

    Scenario: Only a list's delegate sees its roles, not its header
      Given the file "List.qml":
        """qml
        import QtQuick

        ListView {
            id: list
            model: Contacts {}
            header: Text {
                id: heading
            }
            delegate: Text {
                id: line
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "List.qml", "node": {"id": "heading"}}
        """
      Then the answer's "/model" is absent
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "List.qml", "node": {"id": "line"}}
        """
      Then the answer's "/model/roles" lists "name, email, team"

    Scenario: An object outside any list sees no data
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "root"}}
        """
      Then the answer's "/model" is absent

  Rule: A JSON file stays JSON, which a JsonListModel reads

    Scenario: A JSON file becomes a document that reads it
      Given the file "data/contacts.json":
        """json
        [{"name": "Ada", "address": {"city": "London"}}, {"name": "Grace", "team": "Navy"}]
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/contacts.json"}
        """
      Then the file "People.qml" is:
        """qml
        import Ljos.Models

        JsonListModel {
            source: Qt.resolvedUrl("data/contacts.json")
        }
        """

    Scenario: The file is named from the document that reads it
      Given the file "data/contacts.json":
        """json
        [{"name": "Ada"}]
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "App/Data/People.qml", "data": "data/contacts.json"}
        """
      Then the file "App/Data/People.qml" is:
        """qml
        import Ljos.Models

        JsonListModel {
            source: Qt.resolvedUrl("../../data/contacts.json")
        }
        """

    Scenario: A list bound to it lists the JSON's keys as roles
      Given the file "data/contacts.json":
        """json
        [{"name": "Ada", "address": {"city": "London"}}, {"name": "Grace", "team": "Navy"}]
        """
      And the agent has called the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/contacts.json"}
        """
      And the agent has called the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "people"}, "name": "model", "value": "People {}"}
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "row"}}
        """
      Then the answer's "/model/roles" lists "name, address, team"
      And the answer's "/model/rows" is 2
      And the answer's "/model/doc" is "People.qml"
      And the answer's "/model/source" is "data/contacts.json"

    Scenario: One written in the document lists its file's keys
      Given the file "data/tags.json":
        """json
        [{"label": "new"}, {"label": "old"}, {"label": "kept"}]
        """
      And the file "Tags.qml":
        """qml
        import QtQuick
        import Ljos.Models

        Column {
            JsonListModel {
                id: tags
                source: "data/tags.json"
            }
            Repeater {
                model: tags
                Text {
                    id: tag
                }
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Tags.qml", "node": {"id": "tag"}}
        """
      Then the answer's "/model/roles" lists "label"
      And the answer's "/model/rows" is 3
      And the answer's "/model/doc" is "Tags.qml"
      And the answer's "/model/source" is "data/tags.json"

    Scenario: A key the JSON gains is a role at once
      Given the file "data/contacts.json":
        """json
        [{"name": "Ada"}]
        """
      And the agent has called the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/contacts.json"}
        """
      When the file "data/contacts.json" is rewritten outside Studio:
        """json
        [{"name": "Ada", "email": "ada@example.com"}, {"name": "Grace"}]
        """
      And the agent calls the tool "node_inspect" with:
        """json
        {"doc": "People.qml", "node": {"path": "root"}}
        """
      Then the answer's "/model/roles" lists "name, email"
      And the answer's "/model/rows" is 2

    Scenario: Data that is not a list of records is refused
      Given the file "data/one.json":
        """json
        {"name": "Ada"}
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/one.json"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "not_a_list"
      And the file "People.qml" does not exist

  Rule: A relative source is read from the file Ljós reads it from

    A url written as a plain string on a document's root object is read
    from the file that uses the document, as Qt reads it; on any other
    object, and written as `Qt.resolvedUrl`, from its own file.

    Background:
      Given the file "data/contacts.json":
        """json
        [{"name": "Ada", "team": "Navy"}]
        """
      And the file "App/Data/data/contacts.json":
        """json
        [{"title": "Draft"}, {"title": "Sent"}]
        """
      And the file "App/Data/qmldir":
        """
        module App.Data
        People 1.0 People.qml
        """
      And the file "Board.qml":
        """qml
        import QtQuick
        import App.Data

        Column {
            Repeater {
                model: People {}
                Text {
                    id: person
                }
            }
        }
        """

    Scenario: A plain source on a model document's root is read from the file that uses it
      Given the file "App/Data/People.qml":
        """qml
        import Ljos.Models

        JsonListModel {
            source: "data/contacts.json"
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Board.qml", "node": {"id": "person"}}
        """
      Then the answer's "/model/roles" lists "name, team"
      And the answer's "/model/rows" is 1
      And the answer's "/model/doc" is "App/Data/People.qml"
      And the answer's "/model/source" is "data/contacts.json"

    Scenario: A plain source on a model document's root names no file of its own
      Given the file "App/Data/People.qml":
        """qml
        import Ljos.Models

        JsonListModel {
            source: "data/contacts.json"
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "App/Data/People.qml", "node": {"path": "root"}}
        """
      Then the answer's "/model" is absent

    Scenario: A resolved source is read from its own file whoever uses it
      Given the file "App/Data/People.qml":
        """qml
        import Ljos.Models

        JsonListModel {
            source: Qt.resolvedUrl("data/contacts.json")
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Board.qml", "node": {"id": "person"}}
        """
      Then the answer's "/model/roles" lists "title"
      And the answer's "/model/rows" is 2
      And the answer's "/model/source" is "App/Data/data/contacts.json"

  Rule: A JSON file copied into QML is a ListModel document


    Scenario: Each record is a ListElement
      Given the file "data/contacts.json":
        """json
        [{"name": "Ada", "age": 36, "admin": true}, {"name": "Grace \"Amazing\""}]
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/contacts.json", "model": "list"}
        """
      Then the file "People.qml" is:
        """qml
        import QtQuick

        ListModel {
            ListElement {
                name: "Ada"
                age: 36
                admin: true
            }
            ListElement {
                name: "Grace \"Amazing\""
            }
        }
        """

    Scenario: Data that is not a list of flat records is refused
      Given the file "data/nested.json":
        """json
        [{"name": "Ada", "address": {"city": "London"}}]
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/nested.json", "model": "list"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "not_flat"
      And the file "People.qml" does not exist

    Scenario: A key QML reads as the object's id is not a role
      Given the file "data/people.json":
        """json
        [{"id": "p1", "name": "Ada"}]
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/people.json", "model": "list"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "not_a_role"
      And the file "People.qml" does not exist

    Scenario: A record that has a key twice is refused
      Given the file "data/people.json":
        """json
        [{"name": "Ada", "name": "Grace"}]
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/people.json", "model": "list"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "repeated_role"
      And the file "People.qml" does not exist

    Scenario: Text and data together are refused
      Given the file "data/contacts.json":
        """json
        []
        """
      When the agent calls the tool "document_new" with:
        """json
        {"doc": "People.qml", "data": "data/contacts.json", "text": "Item {}"}
        """
      Then the agent is refused with "invalid_argument"
