"""Range les flux de samples.sh au format KNS1 du cœur (autotest::parse_sample).

KNS1, FourCC (u32), drapeaux (u8, 1 : 4:4:4), largeur et hauteur (u16), nombre
de paquets (u16), puis pour chaque paquet : drapeaux (u8, 1 : image clé),
taille (u32) et données. Petit-boutiste.

HEVC et H.264 : une unité d'accès (AUD en tête) par paquet, en Annex B, comme
la Station les envoie ; AV1 : une unité temporelle (OBU de délimitation en
tête) ; PyroWave : les images du fichier de l'outil `encode` (après son paquet
de configuration de 16 octets, `KNPW`, que l'app refait d'après l'en-tête :
taille u32 puis image).
"""
import struct
import sys
from pathlib import Path

H264, HEVC, AV1, PYROWAVE = 0x68323634, 0x68323635, 0x00617631, 0x70777631


def nal_units(data):
    """Positions de début (code de début compris) des NAL d'un flux Annex B."""
    starts = []
    i = 0
    while True:
        j = data.find(b"\x00\x00\x01", i)
        if j < 0:
            break
        s = j - 1 if j > 0 and data[j - 1] == 0 else j
        starts.append((s, j + 3))
        i = j + 3
    return starts


def access_units(data, hevc):
    aus = []
    starts = nal_units(data)
    for k, (s, h) in enumerate(starts):
        t = (data[h] >> 1) & 0x3F if hevc else data[h] & 0x1F
        aud = t == 35 if hevc else t == 9
        end = starts[k + 1][0] if k + 1 < len(starts) else len(data)
        if aud or not aus:
            aus.append(bytearray())
        aus[-1] += data[s:end]
    key = []
    for au in aus:
        types = [((au[h] >> 1) & 0x3F if hevc else au[h] & 0x1F) for _, h in nal_units(au)]
        key.append(any((16 <= t <= 21) if hevc else t == 5 for t in types))
    return list(zip(key, (bytes(a) for a in aus)))


def leb128(data, i):
    v, n = 0, 0
    while True:
        b = data[i + n]
        v |= (b & 0x7F) << (7 * n)
        n += 1
        if not b & 0x80:
            return v, n


def temporal_units(data):
    tus = []
    i = 0
    while i < len(data):
        header = data[i]
        obu_type = (header >> 3) & 0xF
        ext = (header >> 2) & 1
        size, n = leb128(data, i + 1 + ext)
        end = i + 1 + ext + n + size
        if obu_type == 2 or not tus:
            tus.append(bytearray())
        tus[-1] += data[i:end]
        i = end
    out = []
    for k, tu in enumerate(tus):
        out.append((k == 0, bytes(tu)))
    return out


def pyrowave_frames(data):
    # Le fichier commence par le paquet de configuration (16 octets, KNPW) :
    # le lire comme une taille rangeait un paquet illisible (0.11.2).
    assert data[:4] == b"KNPW", "fichier PyroWave sans paquet de configuration"
    out = []
    i = 16
    while i + 4 <= len(data):
        (n,) = struct.unpack_from("<I", data, i)
        out.append((True, data[i + 4:i + 4 + n]))
        i += 4 + n
    return out


def write(path, fourcc, yuv444, w, h, packets):
    with open(path, "wb") as f:
        f.write(b"KNS1" + struct.pack("<IBHHH", fourcc, 1 if yuv444 else 0, w, h, len(packets)))
        for key, data in packets:
            f.write(struct.pack("<BI", 1 if key else 0, len(data)))
            f.write(data)


def main():
    work, out, w, h = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
    for c in ("420", "444"):
        write(out / f"hevc-{c}.kns", HEVC, c == "444", w, h, access_units((work / f"hevc-{c}.es").read_bytes(), True))
        write(out / f"pyrowave-{c}.kns", PYROWAVE, c == "444", w, h, pyrowave_frames((work / f"pyrowave-{c}.pw").read_bytes()))
    write(out / "h264-420.kns", H264, False, w, h, access_units((work / "h264-420.es").read_bytes(), False))
    write(out / "av1-420.kns", AV1, False, w, h, temporal_units((work / "av1-420.obu").read_bytes()))


main()
