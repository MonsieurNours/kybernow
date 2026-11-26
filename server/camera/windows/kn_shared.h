/*
 * Station Windows : l'image de la caméra entre kybernow-camera.exe (SYSTEM,
 * qui l'écrit) et ceux qui la lisent : la source de Windows 11
 * (KyberNowCameraSource.dll, dans le service Frame Server) et le filtre de
 * Windows 10 (KyberNowCameraFilter.dll, dans l'app qui filme). Voir
 * docs/webcam.md, « Station Windows ».
 *
 * Une mémoire partagée nommée, une seule image NV12 protégée par un compteur
 * (pair : stable ; impair : en cours d'écriture). Le lecteur copie l'image
 * et relit le compteur : s'il a bougé, il recommence. Sans écrivain (outil
 * arrêté, ou plus d'image depuis 2 s), il montre du gris foncé.
 */
#ifndef KN_SHARED_H
#define KN_SHARED_H

#include <stdint.h>

#define KN_SHARED_NAME L"Global\\KyberNowCameraFrame"
#define KN_CAMERA_NAME_W L"KyberNow Camera"
#define KN_SHARED_MAGIC 0x4D434E4Bu /* "KNCM" */
#define KN_SHARED_VERSION 1
#define KN_SHARED_MAX_W 1920
#define KN_SHARED_MAX_H 1080
#define KN_SHARED_FRAME_MAX ((size_t)KN_SHARED_MAX_W * KN_SHARED_MAX_H * 3 / 2)
/* Plus d'image depuis ce temps : gris. */
#define KN_SHARED_STALE_MS 2000

/* Lecture seule pour tous (applications à faible intégrité comprises),
 * écriture pour SYSTEM et les administrateurs. */
#define KN_SHARED_SDDL L"D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GR;;;WD)(A;;GR;;;AC)S:(ML;;NW;;;LW)"

/* {43F1F974-7150-488D-97B3-875357DB3949} : la source de Windows 11. */
#define KN_SOURCE_CLSID_STR L"{43F1F974-7150-488D-97B3-875357DB3949}"
#define KN_SOURCE_CLSID_INIT { 0x43f1f974, 0x7150, 0x488d, { 0x97, 0xb3, 0x87, 0x53, 0x57, 0xdb, 0x39, 0x49 } }
/* {F02E187A-10DB-4012-869D-0ED1997641DB} : le filtre de Windows 10. */
#define KN_FILTER_CLSID_STR L"{F02E187A-10DB-4012-869D-0ED1997641DB}"
#define KN_FILTER_CLSID_INIT { 0xf02e187a, 0x10db, 0x4012, { 0x86, 0x9d, 0x0e, 0xd1, 0x99, 0x76, 0x41, 0xdb } }

#pragma pack(push, 8)
struct kn_shared {
    uint32_t magic;
    uint32_t version;
    volatile int32_t seq;      /* pair : image stable */
    uint32_t width, height;    /* de l'image écrite (720p ou 1080p) */
    uint32_t writer_pid;
    uint64_t written_ms;       /* GetTickCount64 de la dernière image */
    int64_t pts_us;
    uint8_t reserved[24];
    uint8_t frame[KN_SHARED_FRAME_MAX]; /* NV12 */
};
#pragma pack(pop)

#ifdef __cplusplus
#include <windows.h>
#include <string.h>

/* Le lecteur commun : copie l'image NV12 à w×h (agrandie ou réduite au plus
 * proche si la taille écrite diffère), ou du gris. 1 si une image de
 * l'appareil a été copiée. */
static inline int kn_shared_read(HANDLE *mapping, struct kn_shared **view, uint8_t *dst, int w, int h)
{
    if (!*view) {
        if (!*mapping)
            *mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, KN_SHARED_NAME);
        if (*mapping)
            *view = (struct kn_shared *)MapViewOfFile(*mapping, FILE_MAP_READ, 0, 0, sizeof(struct kn_shared));
    }
    struct kn_shared *s = *view;
    if (s && s->magic == KN_SHARED_MAGIC && s->version == KN_SHARED_VERSION && s->width && s->height
        && s->width <= KN_SHARED_MAX_W && s->height <= KN_SHARED_MAX_H
        && GetTickCount64() - s->written_ms < KN_SHARED_STALE_MS) {
        for (int tries = 0; tries < 4; tries++) {
            int32_t seq = s->seq;
            if (seq & 1) {
                Sleep(1);
                continue;
            }
            MemoryBarrier();
            int sw = (int)s->width, sh = (int)s->height;
            if (sw == w && sh == h) {
                memcpy(dst, s->frame, (size_t)w * h * 3 / 2);
            } else {
                /* Au plus proche : luminance, puis chrominance par paires. */
                for (int y = 0; y < h; y++) {
                    const uint8_t *row = s->frame + (size_t)(y * sh / h) * sw;
                    uint8_t *out = dst + (size_t)y * w;
                    for (int x = 0; x < w; x++)
                        out[x] = row[x * sw / w];
                }
                const uint8_t *uv = s->frame + (size_t)sw * sh;
                uint8_t *ouv = dst + (size_t)w * h;
                for (int y = 0; y < h / 2; y++) {
                    const uint8_t *row = uv + (size_t)(y * sh / h) * sw;
                    uint8_t *out = ouv + (size_t)y * w;
                    for (int x = 0; x < w / 2; x++) {
                        int sx = (x * sw / w) * 2;
                        out[2 * x] = row[sx];
                        out[2 * x + 1] = row[sx + 1];
                    }
                }
            }
            MemoryBarrier();
            if (s->seq == seq)
                return 1;
        }
    }
    memset(dst, 40, (size_t)w * h);
    memset(dst + (size_t)w * h, 128, (size_t)w * h / 2);
    return 0;
}
#endif

#endif
