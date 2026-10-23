#!/usr/bin/env python3
"""Fabrique le paquet .ipk de l'app webOS (le format d'ares-package : une
archive ar avec debian-binary, control.tar.gz et data.tar.gz), sans les
outils de LG.

    python3 apps/webos/scripts/ipk.py <dossier de l'app> <version> <sortie.ipk>

Le dossier de l'app contient appinfo.json, bin/, lib/ et les images ; il est
posé dans usr/palm/applications/<id>/, avec usr/palm/packages/<id>/packageinfo.json.
"""
import gzip
import io
import json
import pathlib
import sys
import tarfile
import time


def tar_gz(entries):
    """entries : liste de (chemin dans l'archive, octets ou None pour un dossier, mode)."""
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.GNU_FORMAT) as tar:
        now = int(time.time())
        for name, data, mode in entries:
            info = tarfile.TarInfo(name)
            info.mtime = now
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            if data is None:
                info.type = tarfile.DIRTYPE
                info.mode = 0o755
                tar.addfile(info)
            else:
                info.size = len(data)
                info.mode = mode
                tar.addfile(info, io.BytesIO(data))
    return gzip.compress(raw.getvalue(), mtime=0)


def ar(members):
    out = bytearray(b"!<arch>\n")
    for name, data in members:
        header = f"{name:<16}{int(time.time()):<12}{0:<6}{0:<6}{0o100644:<8o}{len(data):<10}`\n"
        out += header.encode()
        out += data
        if len(data) % 2:
            out += b"\n"
    return bytes(out)


def main():
    src, version, dest = pathlib.Path(sys.argv[1]), sys.argv[2], pathlib.Path(sys.argv[3])
    info = json.loads((src / "appinfo.json").read_text(encoding="utf-8"))
    app_id = info["id"]
    root = f"./usr/palm/applications/{app_id}"
    entries = []
    dirs = {"./usr", "./usr/palm", "./usr/palm/applications", root, "./usr/palm/packages", f"./usr/palm/packages/{app_id}"}
    files = []
    size = 0
    for p in sorted(src.rglob("*")):
        rel = p.relative_to(src).as_posix()
        if p.is_dir():
            dirs.add(f"{root}/{rel}")
            continue
        data = p.read_bytes()
        mode = 0o755 if rel.startswith("bin/") or ".so" in rel else 0o644
        files.append((f"{root}/{rel}", data, mode))
        size += len(data)
    package = {
        "app": app_id,
        "id": app_id,
        "loc_name": info.get("title", app_id),
        "package_format_version": 2,
        "vendor": info.get("vendor", ""),
        "version": version,
    }
    pkg = json.dumps(package, indent=4).encode()
    files.append((f"./usr/palm/packages/{app_id}/packageinfo.json", pkg, 0o644))
    for d in sorted(dirs, key=lambda d: d.count("/")):
        entries.append((d, None, 0o755))
    entries += files
    control = (
        f"Package: {app_id}\n"
        f"Version: {version}\n"
        "Section: misc\n"
        "Priority: optional\n"
        "Architecture: arm\n"
        f"Installed-Size: {size // 1024 + 1}\n"
        f"Maintainer: {info.get('vendor', 'N/A')} <contact@leslignesclaires.fr>\n"
        f"Description: {info.get('title', app_id)}\n"
        "webOS-Package-Format-Version: 2\n"
        "webOS-Packager-Version: 1.0.0\n"
    ).encode()
    ipk = ar([
        ("debian-binary", b"2.0\n"),
        ("control.tar.gz", tar_gz([(".", None, 0o755), ("./control", control, 0o644)])),
        ("data.tar.gz", tar_gz(entries)),
    ])
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(ipk)
    print(f"{dest} ({len(ipk) // 1024} Ko)")


if __name__ == "__main__":
    main()
