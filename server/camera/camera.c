/*
 * kybernow-camera : tronc commun (docs/webcam.md, « Station »).
 *
 *   kybernow-camera --uplink     lancé par kycontroller (correctif 0085) :
 *                                paquets de l'app sur l'entrée standard
 *   kybernow-camera --probe      0 si la caméra virtuelle s'ouvrirait
 *   kybernow-camera --install    Windows : inscrit la caméra (installateur)
 *   kybernow-camera --uninstall
 *   kybernow-camera --test       mire animée 20 s (essai sans app)
 *
 * Entrée : chaque paquet précédé de sa longueur (u32, gros-boutiste). START
 * (0x01 + JSON), MEDIA (0x02, drapeaux u8, pts µs u64, image Annex B),
 * PAUSE (0x04). L'entrée fermée : fin.
 *
 * Un fil lit et décode (libavcodec de Kyber : H.264 ou HEVC), met l'image à
 * la définition de l'ordinateur (720p ou 1080p, bordée si le rapport
 * diffère) et la garde ; le fil principal la donne à la sortie 30 fois par
 * seconde, qu'il en soit arrivé une nouvelle ou non : les logiciels qui
 * filment veulent un débit régulier. Avant la première image, et après la
 * fin : une image gris foncé. En pause : la dernière image.
 */
#include "camera.h"

#include <errno.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#else
#include <unistd.h>
#endif

#include <libavcodec/avcodec.h>
#include <libavutil/imgutils.h>
#include <libswscale/swscale.h>

#define START 0x01
#define MEDIA 0x02
#define PAUSE 0x04
#define FLAG_KEY 1
/* Bits 1 et 2 des drapeaux : quarts de tour (sens horaire) qui redressent
 * l'image (Android envoie l'image du capteur telle quelle). */
#define ROTATION_SHIFT 1
/* Une image encodée ne dépasse pas 8 Mo (1080p, image clé). */
#define MAX_PACKET (8u << 20)

void kn_log(const char *fmt, ...)
{
    va_list ap;
    va_start(ap, fmt);
    fputs("kybernow-camera: ", stderr);
    vfprintf(stderr, fmt, ap);
    fputc('\n', stderr);
    va_end(ap);
    fflush(stderr);
}

static pthread_mutex_t status_lock = PTHREAD_MUTEX_INITIALIZER;

void kn_status(const char *json)
{
    pthread_mutex_lock(&status_lock);
    fputs(json, stdout);
    fputc('\n', stdout);
    fflush(stdout);
    pthread_mutex_unlock(&status_lock);
}

static int64_t now_us(void)
{
#ifdef _WIN32
    static LARGE_INTEGER freq;
    LARGE_INTEGER t;
    if (!freq.QuadPart)
        QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t);
    return (int64_t)(t.QuadPart * 1000000 / freq.QuadPart);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;
#endif
}

static void sleep_us(int64_t us)
{
    if (us <= 0)
        return;
#ifdef _WIN32
    Sleep((DWORD)((us + 999) / 1000));
#else
    struct timespec ts = { us / 1000000, (us % 1000000) * 1000 };
    while (nanosleep(&ts, &ts) == -1 && errno == EINTR)
        ;
#endif
}

/* Une valeur texte d'un objet JSON plat (`"clé": "valeur"`), sans
 * échappements : ce que l'app envoie dans START. */
static int json_string(const char *json, const char *key, char *out, size_t size)
{
    char pattern[64];
    snprintf(pattern, sizeof pattern, "\"%s\"", key);
    const char *p = strstr(json, pattern);
    if (!p)
        return -1;
    p += strlen(pattern);
    while (*p == ' ' || *p == ':')
        p++;
    if (*p != '"')
        return -1;
    p++;
    size_t n = 0;
    while (*p && *p != '"' && n + 1 < size)
        out[n++] = *p++;
    out[n] = 0;
    return 0;
}

/* ---- L'image gardée ---- */

struct picture {
    pthread_mutex_t lock;
    int width, height;
    enum kn_format format;
    uint8_t *data;   /* l'image à donner */
    size_t size;
    int64_t pts;
    int fresh;       /* une image décodée est arrivée depuis START */
};

static size_t picture_size(enum kn_format format, int w, int h)
{
    return format == KN_YUYV ? (size_t)w * h * 2 : (size_t)w * h * 3 / 2;
}

/* Gris foncé uni (Y 40, chroma neutre). */
static void fill_grey(uint8_t *data, enum kn_format format, int w, int h)
{
    if (format == KN_YUYV) {
        for (size_t i = 0; i < (size_t)w * h * 2; i += 2) {
            data[i] = 40;
            data[i + 1] = 128;
        }
    } else {
        memset(data, 40, (size_t)w * h);
        memset(data + (size_t)w * h, 128, (size_t)w * h / 2);
    }
}

/* ---- Décodage ---- */

struct decoder {
    AVCodecContext *ctx;
    AVPacket *packet;
    AVFrame *frame;
    AVFrame *turned;          /* l'image redressée (YUV 4:2:0 planaire) */
    struct SwsContext *planar; /* vers YUV 4:2:0 planaire avant de tourner */
    AVFrame *planar_frame;
    struct SwsContext *sws;
    int sws_w, sws_h, sws_fmt;
    uint8_t *scaled;  /* l'image à la taille de la caméra, bordée */
};

static void decoder_close(struct decoder *d)
{
    if (d->sws)
        sws_freeContext(d->sws);
    if (d->planar)
        sws_freeContext(d->planar);
    avcodec_free_context(&d->ctx);
    av_packet_free(&d->packet);
    av_frame_free(&d->frame);
    av_frame_free(&d->turned);
    av_frame_free(&d->planar_frame);
    free(d->scaled);
    memset(d, 0, sizeof *d);
}

static int decoder_open(struct decoder *d, const char *codec)
{
    decoder_close(d);
    enum AVCodecID id = strcmp(codec, "hevc") == 0 ? AV_CODEC_ID_HEVC : AV_CODEC_ID_H264;
    const AVCodec *c = avcodec_find_decoder(id);
    if (!c) {
        kn_log("no %s decoder in this FFmpeg", codec);
        return -1;
    }
    d->ctx = avcodec_alloc_context3(c);
    d->ctx->flags |= AV_CODEC_FLAG_LOW_DELAY;
    d->ctx->thread_count = 2;
    d->ctx->thread_type = FF_THREAD_SLICE;
    if (avcodec_open2(d->ctx, c, NULL) < 0) {
        kn_log("%s decoder not opened", codec);
        avcodec_free_context(&d->ctx);
        return -1;
    }
    d->packet = av_packet_alloc();
    d->frame = av_frame_alloc();
    return 0;
}

/* Un plan de `w`×`h` octets tourné de `turns` quarts de tour (sens horaire). */
static void rotate_plane(const uint8_t *src, int src_stride, int w, int h, uint8_t *dst, int dst_stride, int turns)
{
    for (int y = 0; y < h; y++) {
        const uint8_t *row = src + (size_t)y * src_stride;
        for (int x = 0; x < w; x++) {
            int dx, dy;
            switch (turns) {
            case 1: dx = h - 1 - y; dy = x; break;
            case 2: dx = w - 1 - x; dy = h - 1 - y; break;
            default: dx = y; dy = w - 1 - x; break; /* 3 */
            }
            dst[(size_t)dy * dst_stride + dx] = row[x];
        }
    }
}

/* L'image décodée redressée (d->turned), ou NULL sans rotation. */
static AVFrame *decoder_turn(struct decoder *d, int turns)
{
    AVFrame *f = d->frame;
    if (!turns)
        return NULL;
    if (f->format != AV_PIX_FMT_YUV420P && f->format != AV_PIX_FMT_YUVJ420P) {
        if (!d->planar_frame || d->planar_frame->width != f->width || d->planar_frame->height != f->height) {
            av_frame_free(&d->planar_frame);
            d->planar_frame = av_frame_alloc();
            d->planar_frame->format = AV_PIX_FMT_YUV420P;
            d->planar_frame->width = f->width;
            d->planar_frame->height = f->height;
            if (av_frame_get_buffer(d->planar_frame, 32) < 0)
                return NULL;
        }
        d->planar = sws_getCachedContext(d->planar, f->width, f->height, f->format, f->width, f->height,
                                         AV_PIX_FMT_YUV420P, SWS_POINT, NULL, NULL, NULL);
        if (!d->planar)
            return NULL;
        sws_scale(d->planar, (const uint8_t *const *)f->data, f->linesize, 0, f->height, d->planar_frame->data,
                  d->planar_frame->linesize);
        f = d->planar_frame;
    }
    int w = turns == 2 ? f->width : f->height, h = turns == 2 ? f->height : f->width;
    if (!d->turned || d->turned->width != w || d->turned->height != h) {
        av_frame_free(&d->turned);
        d->turned = av_frame_alloc();
        d->turned->format = AV_PIX_FMT_YUV420P;
        d->turned->width = w;
        d->turned->height = h;
        if (av_frame_get_buffer(d->turned, 32) < 0)
            return NULL;
    }
    rotate_plane(f->data[0], f->linesize[0], f->width, f->height, d->turned->data[0], d->turned->linesize[0], turns);
    for (int p = 1; p < 3; p++)
        rotate_plane(f->data[p], f->linesize[p], (f->width + 1) / 2, (f->height + 1) / 2, d->turned->data[p],
                     d->turned->linesize[p], turns);
    return d->turned;
}

/* L'image décodée, à la taille de la caméra : agrandie ou réduite en gardant
 * son rapport, centrée sur du gris foncé. */
static int decoder_scale(struct decoder *d, struct picture *pic, int turns)
{
    AVFrame *f = decoder_turn(d, turns);
    if (!f)
        f = d->frame;
    int W = pic->width, H = pic->height;
    if (f->width <= 0 || f->height <= 0)
        return -1;
    /* La plus grande taille paire qui tient. */
    int w = W, h = (int)((int64_t)f->height * W / f->width);
    if (h > H) {
        h = H;
        w = (int)((int64_t)f->width * H / f->height);
    }
    w &= ~1;
    h &= ~1;
    if (w < 2 || h < 2)
        return -1;
    enum AVPixelFormat dst = pic->format == KN_YUYV ? AV_PIX_FMT_YUYV422 : AV_PIX_FMT_NV12;
    if (!d->sws || d->sws_w != f->width || d->sws_h != f->height || d->sws_fmt != f->format) {
        if (d->sws)
            sws_freeContext(d->sws);
        d->sws = sws_getContext(f->width, f->height, f->format, w, h, dst, SWS_BILINEAR, NULL, NULL, NULL);
        d->sws_w = f->width;
        d->sws_h = f->height;
        d->sws_fmt = f->format;
        if (!d->sws)
            return -1;
        if (!d->scaled)
            d->scaled = malloc(pic->size);
        fill_grey(d->scaled, pic->format, W, H);
    }
    int x = (W - w) / 2 & ~1, y = (H - h) / 2 & ~1;
    uint8_t *planes[2];
    int strides[2];
    if (pic->format == KN_YUYV) {
        planes[0] = d->scaled + (size_t)y * W * 2 + (size_t)x * 2;
        strides[0] = W * 2;
        planes[1] = NULL;
        strides[1] = 0;
    } else {
        planes[0] = d->scaled + (size_t)y * W + x;
        strides[0] = W;
        planes[1] = d->scaled + (size_t)W * H + (size_t)(y / 2) * W + x;
        strides[1] = W;
    }
    sws_scale(d->sws, (const uint8_t *const *)f->data, f->linesize, 0, f->height, planes, strides);
    pthread_mutex_lock(&pic->lock);
    memcpy(pic->data, d->scaled, pic->size);
    pic->fresh = 1;
    pthread_mutex_unlock(&pic->lock);
    return 0;
}

/* ---- Lecture de l'entrée ---- */

struct reader {
    struct picture *pic;
    struct decoder dec;
    volatile int done;
    volatile int paused;
    int keyframe_asked;
};

static int read_full(void *buf, size_t n)
{
    uint8_t *p = buf;
    while (n) {
        size_t r = fread(p, 1, n, stdin);
        if (r == 0)
            return -1;
        p += r;
        n -= r;
    }
    return 0;
}

static void *reader_main(void *arg)
{
    struct reader *rd = arg;
    uint8_t *buf = malloc(MAX_PACKET);
    for (;;) {
        uint8_t len_be[4];
        if (read_full(len_be, 4) < 0)
            break;
        uint32_t len = (uint32_t)len_be[0] << 24 | (uint32_t)len_be[1] << 16 | (uint32_t)len_be[2] << 8 | len_be[3];
        if (len == 0 || len > MAX_PACKET) {
            kn_log("packet of %u bytes: input lost", len);
            break;
        }
        if (read_full(buf, len) < 0)
            break;
        switch (buf[0]) {
        case START: {
            char json[1024], codec[16] = "h264";
            size_t n = len - 1 < sizeof json - 1 ? len - 1 : sizeof json - 1;
            memcpy(json, buf + 1, n);
            json[n] = 0;
            json_string(json, "codec", codec, sizeof codec);
            kn_log("start: %s", json);
            rd->paused = 0;
            if (decoder_open(&rd->dec, codec) < 0)
                kn_status("{\"state\":\"failed\",\"reason\":\"decoder\"}");
            break;
        }
        case MEDIA: {
            if (len < 10 || !rd->dec.ctx)
                break;
            rd->paused = 0;
            int key = buf[1] & FLAG_KEY;
            int turns = (buf[1] >> ROTATION_SHIFT) & 3;
            int64_t pts = 0;
            for (int i = 0; i < 8; i++)
                pts = pts << 8 | buf[2 + i];
            AVPacket *p = rd->dec.packet;
            if (av_new_packet(p, (int)(len - 10)) < 0)
                break;
            memcpy(p->data, buf + 10, len - 10);
            p->pts = pts;
            if (key)
                p->flags |= AV_PKT_FLAG_KEY;
            int err = avcodec_send_packet(rd->dec.ctx, p);
            av_packet_unref(p);
            if (err < 0 && err != AVERROR(EAGAIN)) {
                /* Référence perdue : une image clé, une fois par seconde au plus. */
                if (!rd->keyframe_asked) {
                    kn_status("{\"keyframe\":true}");
                    rd->keyframe_asked = 30;
                }
            } else if (rd->keyframe_asked) {
                rd->keyframe_asked--;
            }
            while (avcodec_receive_frame(rd->dec.ctx, rd->dec.frame) == 0) {
                decoder_scale(&rd->dec, rd->pic, turns);
                rd->pic->pts = rd->dec.frame->pts;
                av_frame_unref(rd->dec.frame);
            }
            break;
        }
        case PAUSE:
            rd->paused = 1;
            kn_log("paused: the last picture stays");
            break;
        default:
            break;
        }
    }
    free(buf);
    rd->done = 1;
    return NULL;
}

/* ---- Mire (--test) ---- */

static void test_pattern(struct picture *pic, int n)
{
    int W = pic->width, H = pic->height;
    pthread_mutex_lock(&pic->lock);
    fill_grey(pic->data, pic->format, W, H);
    /* Une bande claire qui traverse l'image en 3 s. */
    int x0 = (n * W / 90) % W, bw = W / 10;
    for (int y = H / 3; y < 2 * H / 3; y++)
        for (int x = x0; x < x0 + bw && x < W; x++) {
            if (pic->format == KN_YUYV)
                pic->data[(size_t)y * W * 2 + (size_t)x * 2] = 200;
            else
                pic->data[(size_t)y * W + x] = 200;
        }
    pic->fresh = 1;
    pthread_mutex_unlock(&pic->lock);
}

/* ---- Programme ---- */

static int resolution_of(const char *arg, int *w, int *h)
{
    if (arg && strcmp(arg, "1080p") == 0) {
        *w = 1920;
        *h = 1080;
    } else {
        *w = 1280;
        *h = 720;
    }
    return 0;
}

int main(int argc, char **argv)
{
    const char *mode = argc > 1 ? argv[1] : "";
#ifdef _WIN32
    _setmode(_fileno(stdin), _O_BINARY);
#endif
    if (strcmp(mode, "--probe") == 0) {
        const char *reason = NULL;
        int r = kn_output_probe(&reason);
        printf(r == 0 ? "{\"ready\":true}\n" : "{\"ready\":false,\"reason\":\"%s\"}\n", reason ? reason : "");
        return r == 0 ? 0 : 1;
    }
    if (strcmp(mode, "--install") == 0 || strcmp(mode, "--uninstall") == 0)
        return kn_output_install(strcmp(mode, "--install") == 0) == 0 ? 0 : 1;
    int test = strcmp(mode, "--test") == 0;
    if (!test && strcmp(mode, "--uplink") != 0) {
        fprintf(stderr, "usage: kybernow-camera --uplink | --probe | --install | --uninstall | --test [720p|1080p]\n");
        return 2;
    }

    /* La définition vient du premier START (lu ici, avant d'ouvrir la sortie). */
    int W, H;
    char first[1024] = "";
    uint32_t first_len = 0;
    uint8_t *first_packet = NULL;
    if (test) {
        resolution_of(argc > 2 ? argv[2] : NULL, &W, &H);
    } else {
        uint8_t len_be[4];
        if (read_full(len_be, 4) < 0)
            return 0;
        first_len = (uint32_t)len_be[0] << 24 | (uint32_t)len_be[1] << 16 | (uint32_t)len_be[2] << 8 | len_be[3];
        if (first_len == 0 || first_len > 4096)
            return 1;
        first_packet = malloc(first_len);
        if (read_full(first_packet, first_len) < 0 || first_packet[0] != START)
            return 1;
        size_t n = first_len - 1 < sizeof first - 1 ? first_len - 1 : sizeof first - 1;
        memcpy(first, first_packet + 1, n);
        first[n] = 0;
        char res[16] = "720p";
        json_string(first, "resolution", res, sizeof res);
        resolution_of(res, &W, &H);
    }

    const char *reason = NULL;
    struct kn_output *out = kn_output_open(W, H, &reason);
    if (!out) {
        char json[256];
        snprintf(json, sizeof json, "{\"state\":\"failed\",\"reason\":\"%s\"}", reason ? reason : "output");
        kn_status(json);
        kn_log("no virtual camera: %s", reason ? reason : "?");
        return 1;
    }
    kn_log("%s, %dx%d, %d fps", out->name, W, H, KN_CAMERA_FPS);

    struct picture pic = { .width = W, .height = H, .format = out->format };
    pthread_mutex_init(&pic.lock, NULL);
    pic.size = picture_size(out->format, W, H);
    pic.data = malloc(pic.size);
    fill_grey(pic.data, out->format, W, H);
    uint8_t *frame = malloc(pic.size);

    struct reader rd = { .pic = &pic };
    pthread_t thread;
    if (!test) {
        /* Le START déjà lu ouvre le décodeur. */
        char codec[16] = "h264";
        json_string(first, "codec", codec, sizeof codec);
        if (decoder_open(&rd.dec, codec) < 0) {
            kn_status("{\"state\":\"failed\",\"reason\":\"decoder\"}");
            out->close(out);
            return 1;
        }
        free(first_packet);
        pthread_create(&thread, NULL, reader_main, &rd);
    }

    const int64_t period = 1000000 / KN_CAMERA_FPS;
    int64_t next = now_us();
    for (int n = 0; !rd.done; n++) {
        if (test) {
            if (n >= 20 * KN_CAMERA_FPS)
                break;
            test_pattern(&pic, n);
        }
        pthread_mutex_lock(&pic.lock);
        memcpy(frame, pic.data, pic.size);
        pthread_mutex_unlock(&pic.lock);
        if (out->write(out, frame, pic.size, now_us()) < 0) {
            kn_status("{\"state\":\"failed\",\"reason\":\"output\"}");
            kn_log("virtual camera write failed");
            break;
        }
        next += period;
        int64_t wait = next - now_us();
        if (wait < -period) /* en retard d'une image : on repart d'ici */
            next = now_us();
        else
            sleep_us(wait);
    }

    /* Fin : une seconde de gris, que les logiciels ne restent pas sur la
     * dernière image de l'appareil. */
    fill_grey(frame, out->format, W, H);
    for (int i = 0; i < KN_CAMERA_FPS; i++) {
        out->write(out, frame, pic.size, now_us());
        sleep_us(period);
    }
    out->close(out);
    if (!test && !rd.done)
        pthread_cancel(thread);
    decoder_close(&rd.dec);
    free(frame);
    free(pic.data);
    return 0;
}
