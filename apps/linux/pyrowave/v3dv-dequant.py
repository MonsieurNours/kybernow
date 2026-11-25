#!/usr/bin/env python3
# PyroWave sur Raspberry Pi 5 (Mesa v3dv), docs/pyrowave-client-pi.md :
# v3dv n'écrit jamais dans une image de stockage déclarée sans format
# (shaderStorageImageWriteWithoutFormat faux, et son compilateur exige le
# format). Le déquantificateur du décodeur (wavelet_dequant.comp) écrit dans
# une image R16_SFLOAT (précision 1, celle de la compilation par défaut)
# déclarée sans format. Ce script donne le format `r16f` à cette image :
#
#   - dans la source (wavelet_dequant.comp), pour qu'une régénération des
#     shaders par l'outil de l'amont (slangmosh.sh) donne la même chose ;
#   - dans le SPIR-V déjà compilé de shaders/slangmosh.hpp, en place et à
#     taille égale (la banque ne bouge pas) : le format de l'image
#     (OpTypeImage, Unknown → R16f) et la capacité StorageImageWriteWithoutFormat
#     remplacée par StorageImageExtendedFormats, que le format R16f exige (et
#     que glslang pose en compilant la source corrigée).
#
# Le format est celui de l'image sur toutes les cartes : rien ne change ailleurs.
# Le chemin par fragments du décodeur n'a pas d'autre écriture sans format
# (l'IDWT en calcul en a, il n'est pas pris sur v3dv).
#
# Usage : v3dv-dequant.py SOURCES_DE_PYROWAVE   (idempotent ; échoue si
# le SPIR-V n'a pas la forme attendue, au commit épinglé a32b05e)
import re, sys, pathlib

src = pathlib.Path(sys.argv[1])
comp = src / "shaders/wavelet_dequant.comp"
hpp = src / "shaders/slangmosh.hpp"

old_decl = "layout(set = 0, binding = 0) writeonly uniform image2DArray uDequantImg;"
new_decl = "layout(set = 0, binding = 0, r16f) writeonly uniform image2DArray uDequantImg;"
text = comp.read_text()
if new_decl not in text:
    if old_decl not in text:
        sys.exit("wavelet_dequant.comp : déclaration de uDequantImg introuvable")
    comp.write_text(text.replace(old_decl, new_decl))

h = hpp.read_text()
start = h.index("spirv_bank[] =")
start = h.index("{", start) + 1
end = h.index("};", start)
tokens = list(re.finditer(r"0x([0-9a-f]{8})u", h[start:end]))
words = [int(t.group(1), 16) for t in tokens]
blobs = [(int(o), int(n) // 4) for o, n in re.findall(r"this->wavelet_dequant\[\d\] = device\.request_program\(spirv_bank \+ (\d+), (\d+),", h)]
if len(blobs) != 3:
    sys.exit(f"slangmosh.hpp : {len(blobs)} variantes de wavelet_dequant au lieu de 3")

CAP, EXTENDED_FORMATS, WRITE_WITHOUT_FORMAT = 0x00020011, 49, 56
TYPE_IMAGE, R16F = 0x00090019, 9
changes = {}
for off, count in blobs:
    if words[off] != 0x07230203:
        sys.exit(f"slangmosh.hpp : pas de SPIR-V à {off}")
    i, caps, images = off + 5, 0, 0
    while i < off + count:
        op = words[i]
        n = op >> 16
        if n == 0:
            sys.exit("slangmosh.hpp : SPIR-V illisible")
        if op == CAP and words[i + 1] == WRITE_WITHOUT_FORMAT:
            changes[i + 1] = EXTENDED_FORMATS
            caps += 1
        # OpTypeImage %float 2D, arrayed, image de stockage (Sampled = 2), format inconnu.
        if op == TYPE_IMAGE and words[i + 3] == 1 and words[i + 5] == 1 and words[i + 7] == 2:
            if words[i + 8] not in (0, R16F):
                sys.exit("slangmosh.hpp : image du déquantificateur d'un autre format")
            if words[i + 8] == 0:
                changes[i + 8] = R16F
            images += 1
        i += n
    if images != 1 or caps > 1:
        sys.exit(f"slangmosh.hpp : variante à {off} inattendue ({images} images, {caps} capacités)")
# Remplacements à taille égale (même longueur de texte) : les positions restent justes.
out = list(h)
for idx, value in changes.items():
    t = tokens[idx]
    out[start + t.start():start + t.end()] = list(f"0x{value:08x}u")
hpp.write_text("".join(out))
print(f"PyroWave : déquantificateur en r16f ({len(changes)} mots changés)")
