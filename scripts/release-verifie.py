#!/usr/bin/env python3
"""Vérifications des fichiers d'une version avant publication (scripts/release.sh).

    release-verifie.py fichiers DOSSIER VERSION NOM...   présence, taille, version lisible dedans
    release-verifie.py manifeste DOSSIER VERSION CANAL   SHA256SUMS et kybernow-release.json cohérents
    release-verifie.py pe OBJDUMP DOSSIER [AUTRE...]     dépendances DLL des .exe/.dll de DOSSIER :
                                                         DLL de Windows, ou livrées (DOSSIER, AUTRE…)
    release-verifie.py liste GROUPE...                   noms de scripts/release-fichiers.txt

Chaque problème est écrit sur une ligne « NON  … » ; le code de sortie est le
nombre de problèmes (0 : tout va bien). Rien n'est modifié.
"""
import hashlib
import io
import json
import os
import re
import subprocess
import sys
import tarfile
import zipfile

PROBLEMES = []


def non(msg):
    PROBLEMES.append(msg)
    print(f"  NON  {msg}")


def ok(msg):
    print(f"  ok   {msg}")


# Taille minimale de chaque fichier publié (un fichier vide ou tronqué ne
# passe pas) ; les autres : 1 Mo.
MIN = {
    "update.ps1": 2_000, "appcast.xml": 500, "SHA256SUMS": 200,
    "kybernow-release.json": 200, "kybernow-release.json.minisig": 100,
    "KyberNow-Link-OS.img.xz": 200_000_000, "KyberNow-Link-OS.img.xz.info": 50,
}


def ar_membres(data):
    """Membres d'une archive ar (.deb, .ipk)."""
    if not data.startswith(b"!<arch>\n"):
        raise ValueError("pas une archive ar")
    pos, out = 8, {}
    while pos + 60 <= len(data):
        head = data[pos:pos + 60]
        name = head[:16].decode().strip().rstrip("/")
        size = int(head[48:58].decode().strip())
        out[name] = data[pos + 60:pos + 60 + size]
        pos += 60 + size + (size & 1)
    return out


def tar_fichier(blob, nom):
    with tarfile.open(fileobj=io.BytesIO(blob)) as t:
        for m in t.getmembers():
            if m.name.lstrip("./") == nom:
                return t.extractfile(m).read()
    return None


def control_version(path):
    m = ar_membres(open(path, "rb").read())
    for k, v in m.items():
        if k.startswith("control.tar"):
            if k.endswith(".zst"):
                v = subprocess.run(["zstd", "-dc"], input=v, capture_output=True, check=True).stdout
            ctl = tar_fichier(v, "control")
            if ctl:
                for line in ctl.decode().splitlines():
                    if line.startswith("Version:"):
                        return line.split(":", 1)[1].strip()
    return None


def utf16_contient(path, version):
    data = open(path, "rb").read()
    return version.encode("utf-16-le") in data or version.encode() in data


def plist_version(data):
    m = re.search(rb"<key>CFBundleShortVersionString</key>\s*<string>([^<]+)</string>", data)
    return m.group(1).decode() if m else None


def zip_app_version(path):
    with zipfile.ZipFile(path) as z:
        for n in z.namelist():
            if re.fullmatch(r"[^/]+\.app/Contents/Info\.plist", n):
                return plist_version(z.read(n))
    return None


def version_dedans(nom, path, version):
    """La version lue dans le fichier, quand elle est lisible : (lue, attendue) ou None."""
    if nom in ("KyberNow-Link-macOS.zip", "KyberNow-macOS.zip", "KyberNow-Station-Mac-update.zip"):
        return zip_app_version(path)
    if nom == "kybernow-update.zip":
        with zipfile.ZipFile(path) as z:
            for n in z.namelist():
                if n.endswith("version.txt"):
                    return z.read(n).decode().strip()
        return None
    if nom.endswith(".deb") or nom.endswith(".ipk"):
        v = control_version(path)
        return v.split("-")[0] if v else None
    if nom.endswith(".exe"):
        return version if utf16_contient(path, version) else "(absente)"
    if nom == "KyberNow-Link-OS.img.xz.info":
        txt = open(path).read()
        manque = [k for k in ("download_size", "download_sha256", "extract_sha256", "extract_size") if f"{k}=" not in txt]
        return version if not manque else "(champs manquants : " + ", ".join(manque) + ")"
    if nom == "appcast.xml":
        return version if f'sparkle:shortVersionString>{version}<' in open(path).read() or f'sparkle:shortVersionString="{version}"' in open(path).read() else "(absente)"
    return None


def cmd_fichiers(dossier, version, noms):
    for nom in noms:
        p = os.path.join(dossier, nom)
        if not os.path.isfile(p):
            non(f"{nom} absent")
            continue
        taille = os.path.getsize(p)
        mini = MIN.get(nom, 1_000_000)
        if taille < mini:
            non(f"{nom} : {taille} octets (au moins {mini} attendus)")
            continue
        try:
            lue = version_dedans(nom, p, version)
        except Exception as e:  # archive illisible
            non(f"{nom} : illisible ({e})")
            continue
        if lue is None:
            ok(f"{nom} ({taille // 1024} Kio)")
        elif lue == version:
            ok(f"{nom} ({taille // 1024} Kio, version {version})")
        else:
            non(f"{nom} : version {lue}, {version} attendue")


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


LISTE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "release-fichiers.txt")


def publies(*groupes):
    """Noms de scripts/release-fichiers.txt des groupes donnés."""
    out = []
    with open(LISTE, encoding="utf-8") as f:
        for ligne in f:
            c = ligne.split()
            if len(c) >= 2 and not c[0].startswith("#") and c[1] in groupes:
                out.append(c[0])
    return out


def publies_signes():
    """Ce que SHA256SUMS et le manifeste doivent décrire quand c'est publié."""
    return publies("construit", "copie", "flux", "station-linux")


def cmd_manifeste(dossier, version, canal):
    sums = {}
    for line in open(os.path.join(dossier, "SHA256SUMS")):
        h, n = line.split(None, 1)
        sums[n.strip().lstrip("*")] = h
    hashes = {}
    for n, h in sums.items():
        p = os.path.join(dossier, n)
        if not os.path.isfile(p):
            non(f"SHA256SUMS cite {n}, absent")
            continue
        hashes[n] = sha256(p)
        if hashes[n] != h:
            non(f"SHA256SUMS : empreinte de {n} fausse")
    ok(f"SHA256SUMS : {len(sums)} fichiers")
    m = json.load(open(os.path.join(dossier, "kybernow-release.json")))
    if m.get("version") != version:
        non(f"manifeste : version {m.get('version')}, {version} attendue")
    if m.get("channel") != canal:
        non(f"manifeste : canal {m.get('channel')}, {canal} attendu")
    for n, e in m.get("files", {}).items():
        p = os.path.join(dossier, n)
        if not os.path.isfile(p):
            non(f"manifeste : {n} absent")
            continue
        if e.get("size") != os.path.getsize(p):
            non(f"manifeste : taille de {n} fausse")
        if e.get("sha256") != hashes.get(n) and e.get("sha256") != sha256(p):
            non(f"manifeste : empreinte de {n} fausse")
        if n not in sums:
            non(f"manifeste : {n} absent de SHA256SUMS")
    # Les fichiers publiés (scripts/release-fichiers.txt) présents dans le
    # dossier : tous dans SHA256SUMS et dans le manifeste (Admin et les apps ne
    # prennent que ce que le manifeste signé décrit).
    for n in publies_signes():
        if not os.path.isfile(os.path.join(dossier, n)):
            continue
        if n not in sums:
            non(f"SHA256SUMS : {n} manque")
        if n not in m.get("files", {}):
            non(f"manifeste : {n} manque")
    for n in sums:
        if n not in m.get("files", {}):
            non(f"manifeste : {n} manque (il est dans SHA256SUMS)")
    ok(f"manifeste : {len(m.get('files', {}))} fichiers, version {m.get('version')}, canal {m.get('channel')}")


# DLL de Windows (et de son runtime universel) qu'un exécutable peut demander
# sans les livrer.
SYSTEME = {
    "kernel32", "user32", "gdi32", "advapi32", "shell32", "ole32", "oleaut32", "ws2_32", "bcrypt",
    "ntdll", "userenv", "crypt32", "secur32", "shlwapi", "psapi", "comctl32", "comdlg32", "winmm",
    "version", "setupapi", "hid", "dwmapi", "uxtheme", "imm32", "dxgi", "d3d11", "d3d12", "d3dcompiler_47",
    "dxva2", "mf", "mfplat", "mfreadwrite", "mfuuid", "propsys", "iphlpapi", "dnsapi", "netapi32",
    "wtsapi32", "powrprof", "winhttp", "wininet", "msvcrt", "ucrtbase", "rpcrt4", "dbghelp", "avrt",
    "cfgmgr32", "newdev", "wintrust", "credui", "ncrypt", "mpr", "winspool.drv", "dwrite", "d2d1",
    "windowscodecs", "opengl32", "glu32", "vulkan-1", "xinput1_4", "xinput9_1_0", "mmdevapi", "ksuser",
    "wlanapi", "fwpuclnt", "userenv", "authz", "sspicli", "msimg32", "shcore", "api-ms-win-core-synch-l1-2-0",
    "combase", "wevtapi", "taskschd", "mscoree", "pdh", "wsock32", "normaliz", "dhcpcsvc", "oleacc",
    "urlmon", "msi", "winscard", "bcryptprimitives", "avicap32", "msacm32", "mfsensorgroup", "ksproxy.ax", "evr", "strmiids", "quartz", "d3d10", "d3d10_1", "dcomp", "ntoskrnl.exe", "kernelbase", "dinput8", "hidclass", "user_env", "wbemuuid", "mswsock", "bthprops.cpl", "cabinet", "wldap32", "d3d9", "dxcore",
}


# Exécutables de la surcouche qui ne doivent jamais ouvrir de fenêtre de terminal.
SANS_FENETRE = {"kybernow-micro.exe", "kybernow-camera.exe"}


def est_systeme(dll):
    d = dll.lower()
    if d.startswith("api-ms-win-") or d.startswith("ext-ms-"):
        return True
    return d.rsplit(".", 1)[0] in SYSTEME or d in SYSTEME


def cmd_pe(objdump, dossiers):
    livrees = set()
    for d in dossiers:
        for n in os.listdir(d):
            livrees.add(n.lower())
    vus = 0
    for n in sorted(os.listdir(dossiers[0])):
        if not n.lower().endswith((".exe", ".dll")):
            continue
        p = os.path.join(dossiers[0], n)
        out = subprocess.run([objdump, "-p", p], capture_output=True, text=True)
        if out.returncode != 0:
            non(f"{n} : illisible par {os.path.basename(objdump)}")
            continue
        vus += 1
        # Aides lancées dans la session de l'utilisateur pendant une connexion :
        # en sous-système console, Windows leur ouvre une fenêtre de terminal.
        if n.lower() in SANS_FENETRE:
            m = re.search(r"^Subsystem\s+([0-9a-fA-F]+)", out.stdout, re.M)
            if not m or int(m.group(1), 16) != 2:
                non(f"{n} : sous-système {m.group(1) if m else 'inconnu'}, attendu 2 (Windows, sans fenêtre de terminal)")
        for dll in re.findall(r"DLL Name:\s*(\S+)", out.stdout):
            if not est_systeme(dll) and dll.lower() not in livrees:
                non(f"{os.path.basename(dossiers[0])}/{n} dépend de {dll}, ni de Windows ni livrée")
    ok(f"{os.path.basename(os.path.normpath(dossiers[0]))} : dépendances de {vus} exécutables et DLL")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    c, a = sys.argv[1], sys.argv[2:]
    if c == "fichiers":
        cmd_fichiers(a[0], a[1], a[2:])
    elif c == "manifeste":
        cmd_manifeste(a[0], a[1], a[2])
    elif c == "pe":
        cmd_pe(a[0], a[1:])
    elif c == "liste":
        print("\n".join(publies(*a)))
    else:
        print(__doc__)
        return 2
    return min(len(PROBLEMES), 100)


if __name__ == "__main__":
    sys.exit(main())
