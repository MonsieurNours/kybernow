// Encodeur d'essai (tests/pyrowave/run.sh) : l'encodeur Vulkan de l'amont, par
// son API C (libpyrowave-shared), comme la Station : une image = les paquets
// de pyrowave_encoder_packetize (frontière de 1152 octets) mis bout à bout.
// Entrée : images brutes (yuv420p, yuv444p, ou yuv420p10le) ; sortie : le
// paquet de configuration KNPW puis chaque image, précédés de leur taille (u32 LE).
//   encode IN.yuv OUT.pw LARGEUR HAUTEUR 420|444|420p10 [bits/pixel] [plage 0|1]
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vulkan/vulkan.h>
#include <pyrowave/pyrowave.h>

static void put(FILE *f, const void *data, uint32_t n) {
    uint8_t le[4] = {n, n >> 8, n >> 16, n >> 24};
    fwrite(le, 1, 4, f);
    fwrite(data, 1, n, f);
}

int main(int argc, char **argv) {
    if (argc < 6) {
        fprintf(stderr, "usage : encode IN OUT LARGEUR HAUTEUR 420|444|420p10 [bits/pixel] [plage]\n");
        return 2;
    }
    int w = atoi(argv[3]), h = atoi(argv[4]);
    int c444 = strcmp(argv[5], "444") == 0, p10 = strcmp(argv[5], "420p10") == 0;
    double bpp = argc > 6 ? atof(argv[6]) : 1.2;
    int narrow = argc > 7 ? atoi(argv[7]) : 1;
    FILE *in = fopen(argv[1], "rb"), *out = fopen(argv[2], "wb");
    if (!in || !out) { fprintf(stderr, "encode : fichiers illisibles\n"); return 1; }
    pyrowave_device dev;
    if (pyrowave_create_default_device(&dev) != PYROWAVE_SUCCESS) {
        fprintf(stderr, "encode : pas de device PyroWave\n");
        return 1;
    }
    pyrowave_encoder enc;
    pyrowave_encoder_create_info ei = {dev, w, h, c444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420};
    pyrowave_result er = pyrowave_encoder_create(&ei, &enc);
    if (er != PYROWAVE_SUCCESS) { fprintf(stderr, "encode : pas d'encodeur (%d)\n", (int)er); return 1; }
    int cw = c444 ? w : w / 2, ch = c444 ? h : h / 2, px = p10 ? 2 : 1;
    size_t ys = (size_t)w * h * px, cs = (size_t)cw * ch * px;
    uint8_t *frame = malloc(ys + 2 * cs);
    // KNPW : chroma, couleur (HDR10 si 10 bits), plage, largeur, hauteur.
    uint8_t cfg[16] = {'K', 'N', 'P', 'W', 1, (uint8_t)c444, (uint8_t)p10, (uint8_t)narrow};
    for (int i = 0; i < 4; i++) cfg[8 + i] = (uint8_t)(w >> (8 * i)), cfg[12 + i] = (uint8_t)(h >> (8 * i));
    put(out, cfg, 16);
    size_t max = (size_t)(bpp * w * h / 8.0);
    uint8_t *bits = malloc(max * 2 + 65536);
    pyrowave_packet *packets = malloc(sizeof(pyrowave_packet) * 65536);
    int n = 0;
    size_t total = 0;
    while (fread(frame, 1, ys + 2 * cs, in) == ys + 2 * cs) {
        pyrowave_cpu_buffer b = {0};
        b.width = w;
        b.height = h;
        b.format = c444 ? PYROWAVE_CPU_BUFFER_FORMAT_YUV444P : p10 ? PYROWAVE_CPU_BUFFER_FORMAT_YUV420P10 : PYROWAVE_CPU_BUFFER_FORMAT_YUV420P;
        b.data[0] = frame; b.data[1] = frame + ys; b.data[2] = frame + ys + cs;
        b.row_stride_in_bytes[0] = (size_t)w * px; b.row_stride_in_bytes[1] = b.row_stride_in_bytes[2] = (size_t)cw * px;
        b.plane_size_in_bytes[0] = ys; b.plane_size_in_bytes[1] = b.plane_size_in_bytes[2] = cs;
        pyrowave_rate_control rc = {max};
        if (pyrowave_encoder_encode_cpu(enc, &b, &rc) != PYROWAVE_SUCCESS) { fprintf(stderr, "encode : refusé\n"); return 1; }
        size_t np = 0, got = 0;
        pyrowave_encoder_compute_num_packets(enc, 1152, &np);
        if (np > 65536) { fprintf(stderr, "encode : trop de paquets\n"); return 1; }
        if (pyrowave_encoder_packetize(enc, packets, 1152, &got, bits, max * 2 + 65536) != PYROWAVE_SUCCESS) { fprintf(stderr, "encode : mise en paquets refusée\n"); return 1; }
        // Les paquets bout à bout : un paquet média de la Station.
        size_t len = 0;
        for (size_t i = 0; i < got; i++) {
            memmove(bits + len, bits + packets[i].offset, packets[i].size);
            len += packets[i].size;
        }
        put(out, bits, (uint32_t)len);
        total += len;
        n++;
    }
    fprintf(stderr, "encode : %d images %d × %d %s, %.0f octets par image (%.0f Mbit/s à 60 i/s)\n", n, w, h, argv[5], n ? (double)total / n : 0.0, n ? total * 8.0 * 60 / n / 1e6 : 0.0);
    pyrowave_encoder_destroy(enc);
    pyrowave_device_destroy(dev);
    fclose(out);
    return n ? 0 : 1;
}
