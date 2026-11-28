#!/usr/bin/python3
"""Every text the apps translate in place, tr("..."), is in the template
(.pot) of each program that compiles it, so that it cannot slip through
untranslated. The programs' own `--check` (run by the image build) then
makes sure every template text is translated in every language; it only
sees the lists of texts the programs keep (TEXTS), which this check
completes. A text is also defined only once in each template and
translation: gettext's own tools (msgfmt, translation editors) refuse a
file with a text twice. A text that two programs share reads the same in
both (the same button, the same name), French and Italian use the
typographic apostrophe (l’écran), and no space comes before "…". Run by
tests/lint.sh."""

import pathlib
import re
import sys

CRATE = pathlib.Path(__file__).resolve().parent.parent / "build_files/first_steps"
# Each program: its source root and its template
PROGRAMS = {
    "src/main.rs": "po/celenn-tour.pot",
    "src/tools.rs": "po/tools/celenn-tools.pot",
    "src/parental.rs": "po/parental/celenn-parental-controls.pot",
    "src/parental_daemon.rs": "po/parental/celenn-parental-controls.pot",
}
TR = re.compile(r'\btr\(\s*"((?:[^"\\]|\\.)*)"\s*\)', re.S)
# A module declaration, with the file named by #[path = "..."] when it has one
MOD = re.compile(
    r'^\s*(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;', re.M
)
MSGID = re.compile(r'^msgid "((?:[^"\\]|\\.)*)"\n((?:"(?:[^"\\]|\\.)*"\n)*)', re.M)
ENTRY = re.compile(
    r'^msgid "((?:[^"\\]|\\.)*)"\n((?:"(?:[^"\\]|\\.)*"\n)*)'
    r'msgstr "((?:[^"\\]|\\.)*)"\n((?:"(?:[^"\\]|\\.)*"\n)*)',
    re.M,
)
# The catalogs of one language, by program
CATALOGS = {"tour": "po/{}.po", "tools": "po/tools/{}.po", "parental": "po/parental/{}.po"}
# Shared texts that may read differently in one program, on purpose:
# (text, language): the program whose translation differs
SHARED_EXCEPTIONS = {
    # The tour's "Back" goes to the previous step of a wizard (Précédent);
    # the other apps' goes back to their main page (Retour)
    ("Back", "fr"): "tour",
    # The tour invites to try (imperative); the Tools menus name an action
    # (infinitive), like every other entry there
    ("Lock the screen", "it"): "tour",
    ("Take a screenshot", "it"): "tour",
}
STRAIGHT_APOSTROPHE = re.compile(r"(?<=[^\W\d_])'(?=[^\W_])")


def unescape(text):
    return re.sub(r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), text)


def rust_unescape(text):
    # A backslash at the end of a line joins it to the next, without the
    # next line's indentation
    text = re.sub(r"\\\n\s*", "", text)
    return unescape(text)


def msgids(path):
    ids = []
    for match in MSGID.finditer(path.read_text()):
        more = re.findall(r'"((?:[^"\\]|\\.)*)"', match.group(2))
        ids.append(unescape(match.group(1) + "".join(more)))
    return ids


def template(path):
    return set(msgids(path))


def translations(path):
    """{text: translation} of a .po file, its header left out"""
    found = {}
    for match in ENTRY.finditer(path.read_text()):
        text = unescape(match.group(1) + "".join(re.findall(r'"((?:[^"\\]|\\.)*)"', match.group(2))))
        if text:
            found[text] = unescape(
                match.group(3) + "".join(re.findall(r'"((?:[^"\\]|\\.)*)"', match.group(4)))
            )
    return found


errors = 0
for po in sorted(CRATE.glob("po/**/*.po*")):
    seen = set()
    for text in msgids(po):
        if text and text in seen:
            print(f"{po.relative_to(CRATE)}: defined twice: {text!r}")
            errors += 1
        seen.add(text)
languages = sorted(po.stem for po in CRATE.glob("po/*.po"))
for lang in languages:
    catalogs = {
        name: translations(CRATE / pattern.format(lang))
        for name, pattern in CATALOGS.items()
        if (CRATE / pattern.format(lang)).exists()
    }
    for name, found in catalogs.items():
        where = CATALOGS[name].format(lang)
        for text, translation in found.items():
            if lang in ("fr", "it") and STRAIGHT_APOSTROPHE.search(translation):
                print(f"{where}: straight apostrophe, use ’: {translation!r}")
                errors += 1
            if " …" in translation:
                print(f"{where}: space before …: {translation!r}")
                errors += 1
    shared = set()
    for found in catalogs.values():
        shared |= {text for text in found if sum(text in other for other in catalogs.values()) > 1}
    for text in sorted(shared):
        apart = SHARED_EXCEPTIONS.get((text, lang))
        versions = {
            name: found[text] for name, found in catalogs.items() if text in found and name != apart
        }
        if len(set(versions.values())) > 1:
            print(f"{lang}: {text!r} reads differently: {versions}")
            errors += 1
for root, pot in PROGRAMS.items():
    known = template(CRATE / pot)
    # The program's modules, and theirs (src/ is flat: a module's own
    # modules are files next to it, or the file its #[path] names)
    files = [CRATE / root]
    for file in files:
        for path, name in MOD.findall(file.read_text()):
            module = CRATE / "src" / (path or f"{name}.rs")
            if module.exists() and module not in files:
                files.append(module)
    for file in files:
        source = file.read_text()
        # Not the unit tests
        source = source.split("#[cfg(test)]")[0]
        for match in TR.finditer(source):
            text = rust_unescape(match.group(1))
            if text not in known:
                line = source.count("\n", 0, match.start()) + 1
                print(f"{file.relative_to(CRATE)}:{line}: not in {pot}: {text!r}")
                errors += 1
print(f"translations: {errors} problem(s) found")
sys.exit(1 if errors else 0)
