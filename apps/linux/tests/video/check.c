// Test hors ligne du décodeur (src/native/decoder.c) : flux HEVC générés par
// ffmpeg (run.sh), découpés par l'analyseur de FFmpeg et passés à l'interface
// de l'app. Vérifie le format rendu, la couleur au centre (images unies) et
// mesure le décodage avec la conversion 4:4:4 → BGRA. `hdr10` à la place de
// la couleur : un flux HEVC Main 10 en PQ BT.2020 (HDR10), dont les images
// doivent sortir en 10 bits (P010 ou yuv420p10le) avec la courbe, les
// primaires et les métadonnées (écran de matriçage de 1 000 cd/m², MaxCLL 1 000,
// MaxFALL 400).
//   check FICHIER HW [RRGGBB | hdr10]
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <libavcodec/avcodec.h>

typedef struct kn_frame {
    int width, height, format;
    uint8_t *data[3];
    int linesize[3];
    int64_t pts;
    int bt709, full_range;
    int pq, bt2020;
    float mastering_min, mastering_max, max_cll, max_fall;
} kn_frame;
void *kn_decoder_open(int codec, int hw, int threads, char *err, int errlen);
int kn_decoder_send(void *d, const uint8_t *data, int len, int64_t pts);
int kn_decoder_receive(void *d, kn_frame *out);
const char *kn_decoder_name(void *d);
void kn_decoder_close(void *d);
void kn_log_quiet(void);

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

static void centre(const kn_frame *f, int *r, int *g, int *b) {
    int x = f->width / 2, y = f->height / 2;
    if (f->format == 3) {
        const uint8_t *p = f->data[0] + (size_t)y * f->linesize[0] + x * 4;
        *b = p[0]; *g = p[1]; *r = p[2];
    } else {
        *r = *g = *b = -1;
    }
}

int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "check FICHIER HW [RRGGBB]\n"); return 2; }
    kn_log_quiet();
    FILE *fp = fopen(argv[1], "rb");
    if (!fp) { perror(argv[1]); return 2; }
    fseek(fp, 0, SEEK_END); long n = ftell(fp); fseek(fp, 0, SEEK_SET);
    uint8_t *buf = malloc(n + AV_INPUT_BUFFER_PADDING_SIZE);
    if (fread(buf, 1, n, fp) != (size_t)n) return 2;
    fclose(fp);
    char err[256] = {0};
    void *d = kn_decoder_open(1, atoi(argv[2]), 4, err, sizeof err);
    if (!d) { fprintf(stderr, "ouverture : %s\n", err); return 1; }
    const AVCodec *c = avcodec_find_decoder(AV_CODEC_ID_HEVC);
    AVCodecParserContext *ps = av_parser_init(AV_CODEC_ID_HEVC);
    AVCodecContext *pc = avcodec_alloc_context3(c);
    int frames = 0, fmt = -1, r = -1, g = -1, b = -1, w = 0, h = 0, pq = 0, bt2020 = 0;
    float mmax = 0, cll = 0, fall = 0;
    double busy = 0;
    uint8_t *p = buf; long left = n;
    while (left > 0 || 1) {
        uint8_t *out = NULL; int outlen = 0;
        int used = av_parser_parse2(ps, pc, &out, &outlen, p, (int)left, AV_NOPTS_VALUE, AV_NOPTS_VALUE, 0);
        p += used; left -= used;
        if (outlen > 0) {
            double t0 = now();
            if (kn_decoder_send(d, out, outlen, frames) < 0) { fprintf(stderr, "envoi refusé\n"); return 1; }
            kn_frame f;
            int got = kn_decoder_receive(d, &f);
            busy += now() - t0;
            if (got < 0) { fprintf(stderr, "décodage : %d\n", got); return 1; }
            if (got > 0) {
                frames++; fmt = f.format; w = f.width; h = f.height; centre(&f, &r, &g, &b);
                pq = f.pq; bt2020 = f.bt2020;
                if (f.mastering_max > 0) mmax = f.mastering_max;
                if (f.max_cll > 0) { cll = f.max_cll; fall = f.max_fall; }
            }
        }
        if (left <= 0 && outlen == 0) break;
    }
    printf("%s : %s, %d images %dx%d, format %d, %.2f ms par image", argv[1], kn_decoder_name(d), frames, w, h, fmt, frames ? busy * 1000 / frames : 0);
    int ok = frames > 0 && fmt > 0;
    if (argc > 3 && strcmp(argv[3], "hdr10") == 0) {
        printf(", PQ %d, BT.2020 %d, matriçage %.0f cd/m², MaxCLL %.0f, MaxFALL %.0f", pq, bt2020, mmax, cll, fall);
        if ((fmt != 4 && fmt != 5) || !pq || !bt2020 || mmax < 999 || mmax > 1001 || cll != 1000 || fall != 400) ok = 0;
    } else if (argc > 3) {
        unsigned rgb = strtoul(argv[3], NULL, 16);
        int er = (rgb >> 16) & 255, eg = (rgb >> 8) & 255, eb = rgb & 255;
        printf(", centre %02X%02X%02X (attendu %s)", r, g, b, argv[3]);
        if (abs(r - er) > 3 || abs(g - eg) > 3 || abs(b - eb) > 3) ok = 0;
    }
    printf(" → %s\n", ok ? "ok" : "ÉCHEC");
    kn_decoder_close(d);
    return ok ? 0 : 1;
}
