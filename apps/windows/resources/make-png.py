#!/usr/bin/env python3
"""kybernow.png (256 px) : icône des notifications de Windows, depuis la
géométrie du symbole (make-ico.py, état « now »).

Usage : python3 make-png.py  (Pillow requis)
"""

import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("ico", HERE / "make-ico.py")
ico = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ico)

if __name__ == "__main__":
    ico.tile(256).save(HERE / "kybernow.png")
    print("kybernow.png")
