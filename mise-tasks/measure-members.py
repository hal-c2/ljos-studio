# Asks qml6, running it as nobody's source, what each type in the table has:
# what a `for in` over an instance lists, and of its functions those a handler
# loads for. Written out is what the table does not say, and a line for every
# type measured, so that a type with no line is known not to be.
import os
import re
import subprocess
import sys

work, written, types_written, ids_written = sys.argv[1:]
rows = [line.rstrip("\n").split("\t") for line in open(f"{work}/table.tsv")]
env = dict(os.environ, QT_QPA_PLATFORM="offscreen", QT_FORCE_STDERR_LOGGING="1")
OPENS = 'var H="import QtQuick; import QtQuick.Layouts; import QtQuick.Controls; ";\n'
HEAD = (
    "import QtQuick\nimport QtQuick.Layouts\nimport QtQuick.Controls\n"
    "Item{\nComponent.onCompleted:{\n" + OPENS
)


def run(body):
    with open(f"{work}/m.qml", "w") as qml:
        qml.write(HEAD + body + "Qt.quit()}}\n")
    done = subprocess.run(
        ["qml6", f"{work}/m.qml"], capture_output=True, text=True, timeout=300, env=env
    )
    return done.stderr + done.stdout


listed = {}
body = ""
for _, name, _, _ in rows:
    body += (
        f'try{{var o=Qt.createQmlObject(H+"{name} {{}}",this);var a=[];'
        'for(var k in o){var f=false;try{f=typeof o[k]==="function"}catch(e){};'
        'a.push((f?"F:":"P:")+k)};'
        f'console.log("TYPE {name} "+a.join(","))}}catch(e){{}};\n'
    )
for match in re.finditer(r"TYPE (\S+) (\S*)", run(body)):
    listed[match[1]] = match[2].split(",")

missing = {name: set() for name in listed}
functions = []
for _, name, properties, signals in rows:
    if name not in listed:
        continue
    properties, signals = set(properties.split(",")), set(signals.split(","))
    for key in filter(None, listed[name]):
        member = key[2:]
        if key[0] == "P" and member not in properties:
            missing[name].add(member)
        elif key[0] == "F" and member not in signals:
            functions.append((name, member))

body = ""
for name, member in functions:
    handler = "on" + member[0].upper() + member[1:]
    body += (
        f'try{{Qt.createQmlObject(H+"{name} {{ {handler}: {{}} }}",this);'
        f'console.log("SIGNAL {name} {member}")}}catch(e){{}};\n'
    )
for name, member in re.findall(r"SIGNAL (\S+) (\S+)", run(body)):
    missing[name].add(member)

body = ""
for name in listed:
    body += (
        f'try{{Qt.createQmlObject(H+"{name} {{ nothereZz: 1 }}",this);'
        f'console.log("ANY {name} *")}}catch(e){{}};\n'
        f'try{{Qt.createQmlObject(H+"{name} {{ onNothereZz: {{}} }}",this);'
        f'console.log("ANY {name} on*")}}catch(e){{}};\n'
    )
for name, mark in re.findall(r"ANY (\S+) (\S+)", run(body)):
    missing[name].add(mark)

# Only `is not a type` says a module lacks a type: others are refused for how
# they are used, as what is only attached is.
modules = sorted({uri for uri, _, _, _ in rows})
body = ""
for _, name, _, _ in rows:
    for uri in modules:
        body += (
            f'try{{Qt.createQmlObject("import {uri}; {name} {{}}",this);'
            f'console.log("HAS {name} {uri}")}}catch(e){{'
            'if(e.qmlErrors[0].message.indexOf("is not a type")<0)'
            f'console.log("HAS {name} {uri}")}};\n'
        )
has = {name: set() for _, name, _, _ in rows}
for name, uri in re.findall(r"HAS (\S+) (\S+)", run(body)):
    has[name].add(uri)
with open(types_written, "w") as out:
    for name in sorted(has):
        out.write(f"{name} {','.join(sorted(has[name]))}\n")

# What might be refused as an id: the language's words and every global.
WORDS = """break case catch class const continue debugger default delete do else
enum export extends false finally for function if implements import in instanceof
interface let new null package pragma private protected public return static super
switch this throw true try typeof var void while with yield""".split()
globals_listed = re.search(
    r"GLOBALS (\S+)",
    run('console.log("GLOBALS "+Object.getOwnPropertyNames((function(){return this})()).join(","));\n'),
)[1].split(",")
words = sorted(
    {w for w in WORDS + globals_listed if re.fullmatch(r"[a-z_]\w*", w)}
)
body = ""
for word in words:
    body += (
        f'try{{Qt.createQmlObject(H+"Item {{ id: {word} }}",this)}}'
        f'catch(e){{console.log("NOT_ID {word}")}};\n'
    )
with open(ids_written, "w") as out:
    for word in sorted(set(re.findall(r"NOT_ID (\S+)", run(body)))):
        out.write(word + "\n")

with open(written, "w") as out:
    for name in sorted(missing):
        out.write(f"{name} {','.join(sorted(missing[name]))}\n")
