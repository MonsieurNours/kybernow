/*
 * kybernow-camera : la caméra de l'appareil vers la caméra virtuelle de la
 * Station (docs/webcam.md). Tronc commun (camera.c) et sorties par système
 * (out_windows.c, out_macos.c, out_linux.c).
 */
#ifndef KYBERNOW_CAMERA_H
#define KYBERNOW_CAMERA_H

#include <stdint.h>
#include <stddef.h>

/* Le nom que voient les logiciels, dans toutes les langues. */
#define KN_CAMERA_NAME "KyberNow Camera"
#define KN_CAMERA_FPS 30

/* Format d'image que veut la sortie. */
enum kn_format {
    KN_NV12,  /* Windows, Mac, PipeWire */
    KN_YUYV,  /* v4l2loopback : le format que lisent Zoom, Chrome, OBS */
};

/* Une sortie : la caméra virtuelle d'un système. */
struct kn_output {
    const char *name;
    enum kn_format format;
    /* 0 : prête, à la taille donnée (1280×720 ou 1920×1080). */
    int (*open)(struct kn_output *out, int width, int height);
    /* Une image de la taille ouverte, toutes les 1/30 s. */
    int (*write)(struct kn_output *out, const uint8_t *data, size_t size, int64_t pts_us);
    void (*close)(struct kn_output *out);
    void *priv;
};

/* La sortie de ce système (la première qui s'ouvre), ou NULL avec la raison
 * dans `reason` (« v4l2loopback absent et PipeWire injoignable »…). */
struct kn_output *kn_output_open(int width, int height, const char **reason);

/* `--probe` : 0 si une sortie s'ouvrirait ; écrit la raison sinon. */
int kn_output_probe(const char **reason);

/* `--install` / `--uninstall` (Windows : caméra de Windows 11 ou filtre de
 * Windows 10 ; ailleurs : rien). */
int kn_output_install(int install);

/* Une ligne JSON vers kycontroller (rendue à l'app en STATUS). */
void kn_status(const char *json);

/* Journal (sortie d'erreur, recopiée dans celui de kycontroller). */
void kn_log(const char *fmt, ...);

#endif
