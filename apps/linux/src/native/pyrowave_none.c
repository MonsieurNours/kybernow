// PyroWave absent de cette compilation (Raspberry Pi, build.sh --rpi : pas de
// bibliothèque PyroWave dans le préfixe) : la sonde dit indisponible.
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

typedef struct kn_pw kn_pw;

int kn_pw_probe(char *gpu, int gpulen, char *err, int errlen) {
    if (gpu && gpulen > 0) gpu[0] = 0;
    if (err && errlen > 0) snprintf(err, (size_t)errlen, "PyroWave n'est pas dans cette compilation");
    return 0;
}
kn_pw *kn_pw_open(int w, int h, int c444, int hdr, char *err, int errlen) {
    (void)w; (void)h; (void)c444; (void)hdr;
    if (err && errlen > 0) snprintf(err, (size_t)errlen, "PyroWave n'est pas dans cette compilation");
    return NULL;
}
int kn_pw_decode(kn_pw *d, const uint8_t *data, size_t len, uint8_t *const out[3], const int stride[3], int bt709, int full) {
    (void)d; (void)data; (void)len; (void)out; (void)stride; (void)bt709; (void)full;
    return -2;
}
void kn_pw_close(kn_pw *d) { (void)d; }
int kn_pw_sequence_header(const uint8_t *data, size_t len, int *w, int *h, int *c444) {
    (void)data; (void)len; (void)w; (void)h; (void)c444;
    return 0;
}
