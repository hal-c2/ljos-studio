# Asks qml6, running it as nobody's source, what each type in the table has:
# what a `for in` over an instance lists, and of its functions those a handler
# loads for. Written out is what the table does not say, and a line for every
# type measured, so that a type with no line is known not to be.
import os
import re
import subprocess
import sys

work, written = sys.argv[1:]
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

with open(written, "w") as out:
    for name in sorted(missing):
        out.write(f"{name} {','.join(sorted(missing[name]))}\n")
