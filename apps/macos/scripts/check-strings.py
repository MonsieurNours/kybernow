#!/usr/bin/env python3
"""Catalogues de chaînes des apps Mac, Apple TV, iPhone et iPad (docs/i18n.md).

Le français est la langue source (les clés sont le texte français) et
l'anglais la langue de développement (CFBundleDevelopmentRegion = en, repli
de toute autre langue). Xcode n'écrit fr.lproj/Localizable.strings que pour
les chaînes qui ont une valeur française explicite : sans elle, un Mac en
français afficherait l'anglais. Ce script vérifie chaque catalogue :

- valeur française présente (avec --fix : ajoutée, égale à la clé) ;
- traduction anglaise présente, mêmes paramètres (%@, %lld…) que la clé.

    python3 apps/macos/scripts/check-strings.py [--fix]

À lancer après avoir ajouté des textes (Xcode les ajoute aux catalogues à
la compilation).
"""
import json, os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
CATALOGS = [
    "macos/KyberNow/Localizable.xcstrings",
    "macos/KyberNow/InfoPlist.xcstrings",
    "macos/KyberKit/Localizable.xcstrings",
    "tvos/KyberNowTV/Localizable.xcstrings",
    "tvos/KyberNowTV/InfoPlist.xcstrings",
    "tvos/TopShelf/Localizable.xcstrings",
    "ios/KyberNowIOS/Localizable.xcstrings",
    "ios/KyberNowIOS/InfoPlist.xcstrings",
]
SPEC = re.compile(r"%(\d+\$)?[-+ #0]*\d*(?:\.\d+)?(?:lld|llu|ld|lu|d|u|i|@|lf|f|e|g|s|c)")


def specs(text):
    found = []
    for i, m in enumerate(SPEC.finditer(text.replace("%%", ""))):
        pos = int(m.group(1)[:-1]) if m.group(1) else i + 1
        found.append((pos, re.sub(r"^%(\d+\$)?", "%", m.group(0))))
    return sorted(found)


def unit(entry, lang):
    return entry.get("localizations", {}).get(lang, {}).get("stringUnit", {})


def main():
    fix = "--fix" in sys.argv
    problems = 0
    for rel in CATALOGS:
        path = os.path.join(ROOT, rel)
        data = json.load(open(path))
        changed = False
        for key, entry in data["strings"].items():
            if entry.get("shouldTranslate") is False or entry.get("extractionState") == "stale":
                continue
            plist = rel.endswith("InfoPlist.xcstrings")
            if unit(entry, "fr").get("state") != "translated":
                if fix and not plist:
                    entry.setdefault("localizations", {})["fr"] = {"stringUnit": {"state": "translated", "value": key}}
                    changed = True
                else:
                    print(f"{rel}: pas de valeur française : {key!r}")
                    problems += 1
            en = unit(entry, "en")
            if en.get("state") != "translated":
                print(f"{rel}: pas de traduction anglaise : {key!r}")
                problems += 1
            elif not plist and specs(key) != specs(en.get("value", "")):
                print(f"{rel}: paramètres différents : {key!r} → {en.get('value')!r}")
                problems += 1
        if changed:
            with open(path, "w") as out:
                out.write(json.dumps(data, ensure_ascii=False, indent=2, separators=(",", " : "), sort_keys=True) + "\n")
    print("catalogues complets" if problems == 0 else f"{problems} problème(s)")
    sys.exit(1 if problems else 0)


main()
