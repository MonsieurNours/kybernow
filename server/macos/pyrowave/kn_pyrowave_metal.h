/*
 * KyberNow Station pour Mac : encodeur PyroWave sur la puce Apple
 * (docs/pyrowave.md, section « La Station Mac »).
 *
 * Images de la capture (CVPixelBuffer de ScreenCaptureKit, IOSurface) →
 * textures Metal → conversion RGB → YCbCr sur le GPU (BT.709 pleine plage en
 * SDR ; en HDR10, le P010 de la capture tel quel, BT.2020 PQ plage réduite) et
 * réduction éventuelle (image d'un invité) → encodeur Metal de PyroWave
 * (apps/macos/PyroWave, commit a32b05e) → paquets selon le contrat (une image
 * = la suite des paquets de pyrowave_encoder_packetize).
 *
 * Interface en C pur : l'encodeur FFmpeg `libpyrowave_metal` (correctif Mac
 * 0016) et les outils (kybernow-pyrowave-check, essai de capture) n'ont pas
 * besoin d'Objective-C.
 */
#ifndef KN_PYROWAVE_METAL_H
#define KN_PYROWAVE_METAL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct kn_pw kn_pw;

typedef struct kn_pw_config {
    /* Définition encodée ; l'image d'entrée est réduite (ou agrandie) à
     * cette taille sur le GPU si elle diffère. 4:2:0 : paire. */
    int width, height;
    /* 4:4:4 (SDR seulement : une entrée P010 est en 4:2:0). */
    int chroma444;
    /* Entrée P010 (HDR10 de la capture) plutôt que BGRA. */
    int hdr;
} kn_pw_config;

/* La puce sait-elle faire tourner PyroWave (famille Apple7 ou plus, SIMD de
 * 32) ? Écrit le nom de la puce dans `name` s'il n'est pas NULL. */
int kn_pw_supported(char *name, size_t name_size);

/* 0 : prêt ; < 0 : erreur, expliquée dans `err`. */
int kn_pw_create(const kn_pw_config *config, kn_pw **out, char *err, size_t err_size);
void kn_pw_destroy(kn_pw *pw);

/* Encode une image : `pixbuf` est un CVPixelBufferRef (BGRA, ou P010 'x420'
 * / 'xf20' en HDR) adossé à une IOSurface. `budget` : octets au plus pour
 * l'image (débit / 8 / fps). Rend dans `capacity` la place qu'il faut pour
 * kn_pw_packetize avec ce `packet_size`. 0 : encodée ; < 0 : erreur. */
int kn_pw_encode(kn_pw *pw, void *pixbuf, size_t budget, size_t packet_size, size_t *capacity);

/* Écrit l'image encodée dans `dst` : les paquets de PyroWave bout à bout
 * (frontière `packet_size`). Rend la taille dans `size`. */
int kn_pw_packetize(kn_pw *pw, size_t packet_size, uint8_t *dst, size_t capacity, size_t *size);

/* Dernière erreur (jamais NULL). */
const char *kn_pw_error(const kn_pw *pw);

/* Temps de la dernière image, en ms : conversion (GPU, attendue), encodage
 * (de la soumission à la fin du GPU, avec la mise en paquets). */
void kn_pw_last_timing(const kn_pw *pw, double *convert_ms, double *encode_ms);

/* Paquet de configuration KyberNow (16 octets) : "KNPW" | 1 | chroma |
 * couleur | plage | largeur u32 LE | hauteur u32 LE. */
void kn_pw_config_packet(const kn_pw *pw, uint8_t out[16]);

/* Vérification d'Admin (kybernow-pyrowave-check) : puce, encodeur démarré,
 * aller-retour d'une mire encodée puis décodée. 0 : bon ; 2 : puce qui ne
 * convient pas ; 4 : encodeur qui ne démarre pas ; 5 : aller-retour faux. */
int kn_pw_selftest(char *msg, size_t msg_size);

#ifdef __cplusplus
}
#endif

#endif
