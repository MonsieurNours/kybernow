// Vérification hors ligne du décodeur PyroWave de l'app (src/native/pyrowave.c),
// sur un flux d'encode.c : la sonde du lancement, puis chaque image comme un
// paquet média (I420, I010 en HDR10, BGRA en 4:4:4) : temps de décodage
// (relecture comprise) cadencé à 60 i/s et à la chaîne, PSNR de la première image
// contre la source (Y, et Cb/Cr en 4:2:0), une image tronquée sautée sans casser la suivante.
// Flux : celui d'encode.c (le paquet KNPW précédé de sa taille) ou celui de
// l'outil `encode` de l'app Mac (apps/macos/tests/pyrowave : KNPW seul en tête) ;
// `src8` : source 8 bits d'un flux étiqueté HDR (l'outil du Mac), comparée en 8 bits.
//   check FLUX SOURCE.yuv [src8]
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

typedef struct kn_pw kn_pw;
int kn_pw_probe(char *gpu, int gpulen, char *err, int errlen);
kn_pw *kn_pw_open(int w, int h, int c444, int hdr, char *err, int errlen);
int kn_pw_decode(kn_pw *d, const uint8_t *data, size_t len, uint8_t *const out[3], const int stride[3], int bt709, int full);
void kn_pw_close(kn_pw *d);

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1e3 + t.tv_nsec / 1e6;
}

static uint8_t *read_all(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    *n = (size_t)ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t *b = malloc(*n);
    if (fread(b, 1, *n, f) != *n) return NULL;
    fclose(f);
    return b;
}

// PSNR d'un plan (8 ou 16 bits, `max` : valeur la plus haute).
static double psnr(const uint8_t *a, const uint8_t *b, size_t count, int wide, double max) {
    double se = 0;
    for (size_t i = 0; i < count; i++) {
        double d = wide ? (double)((const uint16_t *)a)[i] - ((const uint16_t *)b)[i] : (double)a[i] - b[i];
        se += d * d;
    }
    return se == 0 ? 99.0 : 10.0 * log10(max * max / (se / count));
}

static int cmp(const void *a, const void *b) { double x = *(const double *)a, y = *(const double *)b; return x < y ? -1 : x > y; }

int main(int argc, char **argv) {
    if (argc < 3) return 2;
    char gpu[320] = {0}, err[256] = {0};
    double t0 = now_ms();
    int ok = kn_pw_probe(gpu, sizeof gpu, err, sizeof err);
    printf("sonde : %s (%s%s) en %.0f ms\n", ok ? "prêt" : "indisponible", gpu, ok ? "" : err, now_ms() - t0);
    if (!ok) return 1;
    size_t n, sn;
    uint8_t *s = read_all(argv[1], &n), *src = read_all(argv[2], &sn);
    if (!s || !src || n < 20) { fprintf(stderr, "flux illisible\n"); return 1; }
    // KNPW en tête (outil du Mac), ou précédé de sa taille (encode.c).
    size_t first = memcmp(s, "KNPW", 4) == 0 ? 16 : 20;
    const uint8_t *cfg = s + first - 16;
    if (memcmp(cfg, "KNPW", 4)) { fprintf(stderr, "flux illisible (pas de KNPW)\n"); return 1; }
    int src8 = argc > 3 && strcmp(argv[3], "src8") == 0;
    int c444 = cfg[5], hdr = cfg[6], full = cfg[7] == 0;
    int w = cfg[8] | cfg[9] << 8 | cfg[10] << 16, h = cfg[12] | cfg[13] << 8 | cfg[14] << 16;
    // Les images : (pointeur, taille).
    size_t cap = 4096, count = 0;
    const uint8_t **fr = malloc(cap * sizeof *fr);
    size_t *fl = malloc(cap * sizeof *fl);
    for (size_t o = first; o + 4 <= n && count < cap;) {
        size_t len = s[o] | s[o + 1] << 8 | s[o + 2] << 16 | (size_t)s[o + 3] << 24;
        fr[count] = s + o + 4;
        fl[count++] = len;
        o += 4 + len;
    }
    kn_pw *d = kn_pw_open(w, h, c444, hdr, err, sizeof err);
    if (!d) { fprintf(stderr, "décodeur : %s\n", err); return 1; }
    int px = hdr ? 2 : 1, cw = c444 ? w : w / 2, ch = c444 ? h : h / 2;
    uint8_t *planes[3];
    int stride[3];
    if (c444) {
        planes[0] = malloc((size_t)w * h * 4); planes[1] = planes[2] = NULL;
        stride[0] = w * 4; stride[1] = stride[2] = 0;
    } else {
        planes[0] = malloc((size_t)w * h * px); planes[1] = malloc((size_t)cw * ch * px); planes[2] = malloc((size_t)cw * ch * px);
        stride[0] = w * px; stride[1] = stride[2] = cw * px;
    }
    int fails = 0;
    // Première image : PSNR contre la source.
    if (kn_pw_decode(d, fr[0], fl[0], planes, stride, 1, full) != 1) { fprintf(stderr, "première image non décodée\n"); return 1; }
    size_t ys = (size_t)w * h * px, cs = (size_t)cw * ch * px;
    double avg_bytes = 0;
    for (size_t i = 0; i < count; i++) avg_bytes += fl[i];
    avg_bytes /= count;
    if (!c444 && hdr && src8) {
        // Source 8 bits étiquetée HDR : le code 10 bits ramené en 8 bits.
        for (size_t i = 0; i < (size_t)w * h; i++) ((uint16_t *)planes[0])[i] = (uint16_t)((((uint16_t *)planes[0])[i] * 255 + 511) / 1023);
        uint8_t *y8 = malloc((size_t)w * h);
        for (size_t i = 0; i < (size_t)w * h; i++) y8[i] = (uint8_t)((uint16_t *)planes[0])[i];
        double py = psnr(y8, src, (size_t)w * h, 0, 255);
        printf("%d × %d 4:2:0 HDR10 (I010, source 8 bits) : PSNR de la première image Y %.1f dB ; %.0f octets par image (%.0f Mbit/s à 60 i/s)\n", w, h, py, avg_bytes, avg_bytes * 8 * 60 / 1e6);
        if (py < 35) { fprintf(stderr, "PSNR trop bas\n"); fails++; }
        free(y8);
    } else if (!c444) {
        double py = psnr(planes[0], src, (size_t)w * h, hdr, hdr ? 1023 : 255);
        double pu = psnr(planes[1], src + ys, (size_t)cw * ch, hdr, hdr ? 1023 : 255);
        double pv = psnr(planes[2], src + ys + cs, (size_t)cw * ch, hdr, hdr ? 1023 : 255);
        printf("%d × %d %s %s : PSNR de la première image Y %.1f dB, Cb %.1f, Cr %.1f ; %.0f octets par image (%.0f Mbit/s à 60 i/s)\n", w, h, c444 ? "4:4:4" : "4:2:0", hdr ? "HDR10 (I010)" : "SDR (I420)", py, pu, pv, avg_bytes, avg_bytes * 8 * 60 / 1e6);
        if (py < 35) { fprintf(stderr, "PSNR trop bas\n"); fails++; }
    } else {
        // BGRA : le gris de la source (Y au milieu de l'image) retrouvé en vert, à 6 près.
        size_t mid = (size_t)(h / 2) * w + w / 2;
        int y = src[mid], u = src[ys + mid], v = src[ys + cs + mid];
        int g = planes[0][mid * 4 + 1];
        printf("%d × %d 4:4:4 SDR (BGRA) : pixel du milieu Y %d Cb %d Cr %d → B %d G %d R %d ; %.0f octets par image (%.0f Mbit/s à 60 i/s)\n", w, h, y, u, v, planes[0][mid * 4], g, planes[0][mid * 4 + 2], avg_bytes, avg_bytes * 8 * 60 / 1e6);
    }
    // Cadencé à 60 i/s, puis à la chaîne.
    double *times = malloc(count * sizeof *times);
    for (size_t i = 0; i < count; i++) {
        double a = now_ms();
        if (kn_pw_decode(d, fr[i], fl[i], planes, stride, 1, full) != 1) fails++;
        times[i] = now_ms() - a;
        double rest = 1000.0 / 60 - times[i];
        if (rest > 0) usleep((useconds_t)(rest * 1000));
    }
    double sum = 0, mx = 0;
    for (size_t i = 0; i < count; i++) { sum += times[i]; mx = times[i] > mx ? times[i] : mx; }
    qsort(times, count, sizeof *times, cmp);
    double a = now_ms();
    for (size_t i = 0; i < count; i++) kn_pw_decode(d, fr[i], fl[i], planes, stride, 1, full);
    double chain = now_ms() - a;
    printf("décodage, relecture comprise, cadencé à 60 i/s : moyenne %.2f ms, médiane %.2f, maximum %.2f ; à la chaîne : %.0f i/s\n", sum / count, times[count / 2], mx, count * 1000.0 / chain);
    // Image tronquée (60 %) : sautée (0 ou < 0), la suivante se décode.
    int r = kn_pw_decode(d, fr[1 % count], fl[1 % count] * 6 / 10, planes, stride, 1, full);
    int next = kn_pw_decode(d, fr[2 % count], fl[2 % count], planes, stride, 1, full);
    printf("image tronquée : %s ; la suivante : %s\n", r == 1 ? "décodée (partielle)" : "sautée", next == 1 ? "décodée" : "ÉCHEC");
    if (next != 1) fails++;
    kn_pw_close(d);
    printf("%s\n", fails ? "ÉCHEC" : "OK");
    return fails != 0;
}
