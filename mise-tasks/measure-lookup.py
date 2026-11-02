# Asks qml6, running it as nobody's source, where each row of the lookup table
# finds its name. Every file says it was the one made, so a load with no word
# from a file was a module's type.
import os
import shutil
import subprocess
import sys

work, table = sys.argv[1:]
env = dict(os.environ, QT_QPA_PLATFORM="offscreen", QT_FORCE_STDERR_LOGGING="1")
FILE = """import QtQuick
Item {{
    component Inner: Item {{ Component.onCompleted: console.log("LAND {folder}") }}
    Component.onCompleted: console.log("LAND {folder}")
}}
"""


def statement(spec):
    what, _, alias = spec.partition(" as ")
    what = f'"{what}"' if "/" in what or what == "." else what
    return f"import {what}" + (f" as {alias}" if alias else "")


def measure(document, imports, files, name):
    root = f"{work}/row"
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(os.path.dirname(f"{root}/{document}"), exist_ok=True)
    for file in files.split():
        folder, _, _ = file.rpartition("/")
        os.makedirs(f"{root}/{folder}", exist_ok=True)
        with open(f"{root}/{file}.qml", "w") as out:
            out.write(FILE.format(folder=folder or "."))
    lines = [statement(s.strip()) for s in imports.split(";") if s.strip()]
    # Under a name, so the root loads without the row's bare imports gaining one.
    if not any(line.split()[1:] == ["QtQuick"] for line in lines):
        lines.insert(0, "import QtQuick as Base_")
        base = "Base_."
    else:
        base = ""
    with open(f"{root}/{document}", "w") as out:
        out.write("\n".join(lines) + f"\n{base}Item {{\n    {name} {{}}\n")
        out.write(f"    {base}Component.onCompleted: Qt.quit()\n}}\n")
    done = subprocess.run(
        ["qml6", f"{root}/{document}"],
        capture_output=True, text=True, timeout=60, env=env,
    )
    said = done.stderr + done.stdout
    lands = [w.split("LAND ")[1].split()[0] for w in said.splitlines() if "LAND " in w]
    if lands:
        return lands[0]
    return "module" if done.returncode == 0 else "none"


rows = []
for line in open(table):
    if line.startswith("#") or not line.strip():
        rows.append(line)
        continue
    cells = [c.strip() for c in line.split("|")]
    case, document, imports, files, name, answer = cells
    if "App.Native" not in imports and "/opt/" not in imports and name != "Zzz":
        answer = measure(document, imports, files, name)
    rows.append(" | ".join([case, document, imports, files, name, answer]) + "\n")
with open(table, "w") as out:
    out.writelines(rows)
