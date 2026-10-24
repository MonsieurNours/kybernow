// Décodage de l'image par FFmpeg : un décodeur matériel quand la machine en a
// un pour le format (VA-API sur PC ; sur le Raspberry Pi, les décodeurs V4L2 de
// la puce : HEVC par rpivid, API « request », et H.264 par bcm2835-codec, M2M),
// sinon le processeur. Une interface C réduite, appelée depuis Rust
// (src/device/video.rs), pour ne pas dépendre de la disposition des
// structures de FFmpeg côté Rust.
//
// Les images VA-API et DRM (Pi) sont recopiées en NV12 dans la mémoire du
// processeur (av_hwframe_transfer_data ; sur le Pi, le format en colonnes
// « SAND » du décodeur y devient linéaire) puis montrées par SDL : une copie par
// image, négligeable à la taille du Steam Deck.
//
// 4:4:4 (en option, HEVC) : SDL n'a pas de texture YUV 4:4:4 ; les images sont
// converties ici en BGRA (virgule fixe, sur quelques fils), sur le fil du
// décodeur, et montrées telles quelles.
//
// KN_V4L2_REQUEST (build.rs, quand build.sh compile le FFmpeg de Raspberry Pi,
// --rpi) : le hwaccel hevc_v4l2request existe, avec un périphérique DRM.

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <pthread.h>

#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/pixdesc.h>
#include <libavutil/error.h>
#include <libavutil/log.h>

#include <va/va.h>
#include <va/va_drm.h>

#define KN_FMT_NONE 0
#define KN_FMT_NV12 1
#define KN_FMT_I420 2
#define KN_FMT_BGRA 3

// Décodeur matériel demandé : aucun, VA-API, V4L2 (Raspberry Pi).
#define KN_HW_NONE 0
#define KN_HW_VAAPI 1
#define KN_HW_V4L2 2

typedef struct kn_frame {
    int width, height, format;
    uint8_t *data[3];
    int linesize[3];
    int64_t pts;
    // 1 : BT.709, 0 : BT.601 ; 1 : plage complète.
    int bt709, full_range;
} kn_frame;

typedef struct kn_decoder {
    AVCodecContext *ctx;
    AVBufferRef *hw;
    AVPacket *pkt;
    AVFrame *frame, *sw, *out;
    int hw_active;
    // Le format d'image du décodeur matériel (VAAPI, DRM_PRIME), ou NONE.
    enum AVPixelFormat hw_fmt;
    char name[64];
    // Image 4:4:4 convertie en BGRA.
    uint8_t *rgb;
    size_t rgb_size;
} kn_decoder;

// ---- 4:4:4 → BGRA --------------------------------------------------------

typedef struct kn_conv {
    const uint8_t *y, *u, *v;
    int ys, us, vs;     // pas de ligne
    int step;           // pas d'un pixel (1 : plans séparés, 4 : VUYX)
    uint8_t *out;
    int out_stride, width, y0, y1;
    int cy, crv, cgu, cgv, cbu, yoff; // coefficients × 2^14
} kn_conv;

static inline uint8_t clamp8(int v) { return v < 0 ? 0 : v > 255 ? 255 : (uint8_t)v; }

static void *conv_rows(void *arg) {
    kn_conv *c = arg;
    const int step = c->step;
    for (int row = c->y0; row < c->y1; row++) {
        const uint8_t *py = c->y + (size_t)row * c->ys;
        const uint8_t *pu = c->u + (size_t)row * c->us;
        const uint8_t *pv = c->v + (size_t)row * c->vs;
        uint8_t *o = c->out + (size_t)row * c->out_stride;
        for (int x = 0; x < c->width; x++) {
            int yy = (py[x * step] - c->yoff) * c->cy + (1 << 13);
            int uu = pu[x * step] - 128, vv = pv[x * step] - 128;
            o[4 * x + 0] = clamp8((yy + c->cbu * uu) >> 14);
            o[4 * x + 1] = clamp8((yy - c->cgu * uu - c->cgv * vv) >> 14);
            o[4 * x + 2] = clamp8((yy + c->crv * vv) >> 14);
            o[4 * x + 3] = 255;
        }
    }
    return NULL;
}

// Convertit l'image 4:4:4 `f` dans d->rgb. 0 : fait, < 0 : format inconnu.
static int to_bgra(kn_decoder *d, const AVFrame *f, int bt709, int full) {
    kn_conv base = {0};
    switch (f->format) {
    case AV_PIX_FMT_YUV444P:
    case AV_PIX_FMT_YUVJ444P:
        base.y = f->data[0]; base.u = f->data[1]; base.v = f->data[2];
        base.ys = f->linesize[0]; base.us = f->linesize[1]; base.vs = f->linesize[2];
        base.step = 1;
        break;
    case AV_PIX_FMT_VUYX:
    case AV_PIX_FMT_VUYA:
        base.v = f->data[0]; base.u = f->data[0] + 1; base.y = f->data[0] + 2;
        base.ys = base.us = base.vs = f->linesize[0];
        base.step = 4;
        break;
    default:
        return -1;
    }
    size_t need = (size_t)f->width * f->height * 4;
    if (d->rgb_size < need) {
        free(d->rgb);
        d->rgb = malloc(need);
        d->rgb_size = d->rgb ? need : 0;
        if (!d->rgb) return -1;
    }
    // BT.709 ou BT.601, plage limitée ou complète.
    double kr = bt709 ? 0.2126 : 0.299, kb = bt709 ? 0.0722 : 0.114, kg = 1.0 - kr - kb;
    double ys = full ? 1.0 : 255.0 / 219.0, cs = full ? 1.0 : 255.0 / 224.0;
    double rv = 2.0 * (1.0 - kr) * cs, bu = 2.0 * (1.0 - kb) * cs;
    double gu = bu * kb / kg, gv = rv * kr / kg;
    base.cy = (int)(ys * 16384 + 0.5);
    base.crv = (int)(rv * 16384 + 0.5);
    base.cbu = (int)(bu * 16384 + 0.5);
    base.cgu = (int)(gu * 16384 + 0.5);
    base.cgv = (int)(gv * 16384 + 0.5);
    base.yoff = full ? 0 : 16;
    base.out = d->rgb;
    base.out_stride = f->width * 4;
    base.width = f->width;
    long ncpu = sysconf(_SC_NPROCESSORS_ONLN);
    int n = ncpu > 4 ? 4 : ncpu < 1 ? 1 : (int)ncpu;
    if ((size_t)f->width * f->height < 640 * 480) n = 1;
    kn_conv parts[4];
    pthread_t th[4];
    int started[4] = {0};
    for (int i = 0; i < n; i++) {
        parts[i] = base;
        parts[i].y0 = f->height * i / n;
        parts[i].y1 = f->height * (i + 1) / n;
        if (i > 0) started[i] = pthread_create(&th[i], NULL, conv_rows, &parts[i]) == 0;
        if (i > 0 && !started[i]) conv_rows(&parts[i]);
    }
    conv_rows(&parts[0]);
    for (int i = 1; i < n; i++) if (started[i]) pthread_join(th[i], NULL);
    return 0;
}

// Le format que FFmpeg propose : celui du décodeur matériel s'il est là,
// sinon le premier format logiciel.
static enum AVPixelFormat get_format(AVCodecContext *ctx, const enum AVPixelFormat *fmts) {
    kn_decoder *d = ctx->opaque;
    if (d->hw && d->hw_fmt != AV_PIX_FMT_NONE) {
        for (const enum AVPixelFormat *p = fmts; *p != AV_PIX_FMT_NONE; p++) {
            if (*p == d->hw_fmt) {
                d->hw_active = 1;
                return *p;
            }
        }
    }
    d->hw_active = 0;
    for (const enum AVPixelFormat *p = fmts; *p != AV_PIX_FMT_NONE; p++) {
        const AVPixFmtDescriptor *desc = av_pix_fmt_desc_get(*p);
        if (desc && !(desc->flags & AV_PIX_FMT_FLAG_HWACCEL)) return *p;
    }
    return fmts[0];
}

static void set_err(char *err, int len, const char *what, int code) {
    if (!err || len <= 0) return;
    char buf[AV_ERROR_MAX_STRING_SIZE] = {0};
    if (code) av_strerror(code, buf, sizeof buf);
    snprintf(err, len, "%s%s%s", what, code ? " : " : "", buf);
}

void kn_log_quiet(void) { av_log_set_level(AV_LOG_ERROR); }

// Le périphérique DRM du Pi pour le hwaccel V4L2 request : la première carte qui s'ouvre.
#ifdef KN_V4L2_REQUEST
static int open_drm_device(AVBufferRef **hw) {
    int r = AVERROR(ENOENT);
    for (int card = 0; card < 4; card++) {
        char path[32];
        snprintf(path, sizeof path, "/dev/dri/card%d", card);
        if (access(path, R_OK) != 0) continue;
        r = av_hwdevice_ctx_create(hw, AV_HWDEVICE_TYPE_DRM, path, NULL, 0);
        if (r >= 0) return 0;
    }
    return r;
}
#endif

// codec : 0 H.264, 1 HEVC, 2 AV1. hw : KN_HW_NONE, KN_HW_VAAPI ou KN_HW_V4L2.
kn_decoder *kn_decoder_open(int codec, int hw, int threads, char *err, int errlen) {
    enum AVCodecID id = codec == 1 ? AV_CODEC_ID_HEVC : codec == 2 ? AV_CODEC_ID_AV1 : AV_CODEC_ID_H264;
    const AVCodec *c = NULL;
    kn_decoder *d = calloc(1, sizeof *d);
    if (!d) return NULL;
    d->hw_fmt = AV_PIX_FMT_NONE;
    int m2m = 0;
    if (hw == KN_HW_V4L2) {
        if (codec == 0) {
            // H.264 par bcm2835-codec : un décodeur à part (M2M), images en mémoire.
            c = avcodec_find_decoder_by_name("h264_v4l2m2m");
            if (!c) { set_err(err, errlen, "décodeur H.264 V4L2 absent de FFmpeg", 0); free(d); return NULL; }
            m2m = 1;
        } else if (codec == 1) {
#ifdef KN_V4L2_REQUEST
            int r = open_drm_device(&d->hw);
            if (r < 0) { set_err(err, errlen, "périphérique DRM pour le décodeur HEVC du Pi", r); d->hw = NULL; free(d); return NULL; }
            d->hw_fmt = AV_PIX_FMT_DRM_PRIME;
#else
            set_err(err, errlen, "HEVC V4L2 request absent de cette compilation", 0);
            free(d);
            return NULL;
#endif
        } else {
            set_err(err, errlen, "AV1 : rien ne le décode sur le Pi", 0);
            free(d);
            return NULL;
        }
    } else if (hw == KN_HW_VAAPI) {
        int r = av_hwdevice_ctx_create(&d->hw, AV_HWDEVICE_TYPE_VAAPI, NULL, NULL, 0);
        if (r < 0) { d->hw = NULL; if (codec == 2) { set_err(err, errlen, "VA-API indisponible", r); free(d); return NULL; } }
        if (d->hw) d->hw_fmt = AV_PIX_FMT_VAAPI;
    }
    if (!c) c = avcodec_find_decoder(id);
    if (!c) { set_err(err, errlen, "décodeur absent de FFmpeg", 0); av_buffer_unref(&d->hw); free(d); return NULL; }
    if (codec == 2 && !d->hw) { set_err(err, errlen, "AV1 sans décodeur matériel", 0); free(d); return NULL; }
    d->ctx = avcodec_alloc_context3(c);
    d->ctx->opaque = d;
    d->ctx->get_format = get_format;
    d->ctx->flags |= AV_CODEC_FLAG_LOW_DELAY;
    d->ctx->flags2 |= AV_CODEC_FLAG2_FAST;
    if (d->hw) {
        d->ctx->hw_device_ctx = av_buffer_ref(d->hw);
        d->ctx->thread_count = 1;
    } else if (m2m) {
        d->ctx->thread_count = 1;
    } else {
        // Fils par tranches seulement : les fils par image ajoutent une image de retard chacun.
        d->ctx->thread_type = FF_THREAD_SLICE;
        d->ctx->thread_count = threads > 0 ? threads : 4;
    }
    int r = avcodec_open2(d->ctx, c, NULL);
    if (r < 0) {
        set_err(err, errlen, "ouverture du décodeur", r);
        avcodec_free_context(&d->ctx);
        av_buffer_unref(&d->hw);
        free(d);
        return NULL;
    }
    if (m2m) d->hw_active = 1;
    d->pkt = av_packet_alloc();
    d->frame = av_frame_alloc();
    d->sw = av_frame_alloc();
    const char *who = (hw == KN_HW_V4L2) ? "Pi" : d->hw ? "VA-API" : "CPU";
    snprintf(d->name, sizeof d->name, "%s %s", who, codec == 1 ? "HEVC" : codec == 2 ? "AV1" : "H.264");
    return d;
}

const char *kn_decoder_name(kn_decoder *d) {
    if (d->hw && !d->hw_active && d->ctx->pix_fmt != AV_PIX_FMT_NONE && d->ctx->pix_fmt != d->hw_fmt) {
        // Le décodeur matériel refusé par FFmpeg pour ce flux : le processeur décode.
        if (strncmp(d->name, "VA-API", 6) == 0) memmove(d->name, d->name + 3, strlen(d->name + 3) + 1);
        memcpy(d->name, "CPU", 3);
    }
    return d->name;
}

int kn_decoder_is_hw(kn_decoder *d) { return d->hw_active; }

int kn_decoder_send(kn_decoder *d, const uint8_t *data, int len, int64_t pts) {
    av_packet_unref(d->pkt);
    int r = av_new_packet(d->pkt, len);
    if (r < 0) return r;
    memcpy(d->pkt->data, data, len);
    d->pkt->pts = pts;
    r = avcodec_send_packet(d->ctx, d->pkt);
    if (r == AVERROR(EAGAIN)) return 0;
    return r;
}

// 1 : une image (la plus récente des images prêtes), 0 : rien, < 0 : erreur.
int kn_decoder_receive(kn_decoder *d, kn_frame *out) {
    int got = 0;
    for (;;) {
        int r = avcodec_receive_frame(d->ctx, d->frame);
        if (r == AVERROR(EAGAIN) || r == AVERROR_EOF) break;
        if (r < 0) return r;
        AVFrame *src = d->frame;
        if (d->frame->format == AV_PIX_FMT_VAAPI || d->frame->format == AV_PIX_FMT_DRM_PRIME) {
            av_frame_unref(d->sw);
            // NV12 en 4:2:0 (VA-API : le format des surfaces en 4:4:4, VUYX, 444P…).
            // Sur le Pi, les images SAND du décodeur deviennent du NV12 linéaire ici.
            enum AVPixelFormat swf = AV_PIX_FMT_NV12;
            if (d->frame->format == AV_PIX_FMT_VAAPI && d->frame->hw_frames_ctx) swf = ((AVHWFramesContext *)d->frame->hw_frames_ctx->data)->sw_format;
            d->sw->format = swf;
            r = av_hwframe_transfer_data(d->sw, d->frame, 0);
            if (r < 0 && d->frame->format == AV_PIX_FMT_DRM_PRIME && swf == AV_PIX_FMT_NV12) {
                // Le FFmpeg du Pi préfère parfois le 4:2:0 planaire.
                av_frame_unref(d->sw);
                d->sw->format = AV_PIX_FMT_YUV420P;
                r = av_hwframe_transfer_data(d->sw, d->frame, 0);
            }
            if (r < 0) { av_frame_unref(d->frame); return r; }
            d->sw->pts = d->frame->pts;
            d->sw->colorspace = d->frame->colorspace;
            d->sw->color_range = d->frame->color_range;
            av_frame_unref(d->frame);
            src = d->sw;
        }
        // Garder la dernière : celle qu'on montrera.
        if (!d->out) d->out = av_frame_alloc();
        av_frame_unref(d->out);
        av_frame_move_ref(d->out, src);
        got = 1;
    }
    if (!got) return 0;
    AVFrame *f = d->out;
    memset(out, 0, sizeof *out);
    out->width = f->width;
    out->height = f->height;
    switch (f->format) {
    case AV_PIX_FMT_NV12: out->format = KN_FMT_NV12; break;
    case AV_PIX_FMT_YUV420P:
    case AV_PIX_FMT_YUVJ420P: out->format = KN_FMT_I420; break;
    default: out->format = KN_FMT_NONE; break;
    }
    for (int i = 0; i < 3; i++) { out->data[i] = f->data[i]; out->linesize[i] = f->linesize[i]; }
    out->pts = f->pts;
    out->bt709 = f->colorspace != AVCOL_SPC_BT470BG && f->colorspace != AVCOL_SPC_SMPTE170M;
    out->full_range = f->color_range == AVCOL_RANGE_JPEG || f->format == AV_PIX_FMT_YUVJ420P || f->format == AV_PIX_FMT_YUVJ444P;
    if (out->format == KN_FMT_NONE && to_bgra(d, f, out->bt709, out->full_range) == 0) {
        out->format = KN_FMT_BGRA;
        memset(out->data, 0, sizeof out->data);
        memset(out->linesize, 0, sizeof out->linesize);
        out->data[0] = d->rgb;
        out->linesize[0] = f->width * 4;
    }
    if (out->format == KN_FMT_NONE) {
        static int warned = 0;
        if (!warned++) av_log(NULL, AV_LOG_ERROR, "kybernow : format d'image non montré : %s\n", av_get_pix_fmt_name(f->format));
    }
    return 1;
}

void kn_decoder_flush(kn_decoder *d) { avcodec_flush_buffers(d->ctx); }

void kn_decoder_close(kn_decoder *d) {
    if (!d) return;
    av_packet_free(&d->pkt);
    av_frame_free(&d->frame);
    av_frame_free(&d->sw);
    av_frame_free(&d->out);
    free(d->rgb);
    avcodec_free_context(&d->ctx);
    av_buffer_unref(&d->hw);
    free(d);
}

// Le FFmpeg de cette compilation sait-il le HEVC du Pi (hwaccel V4L2 request) ?
int kn_v4l2_request_built(void) {
#ifdef KN_V4L2_REQUEST
    return 1;
#else
    return 0;
#endif
}

// Le décodeur H.264 V4L2 (M2M) est-il dans ce FFmpeg ?
int kn_v4l2_m2m_built(void) { return avcodec_find_decoder_by_name("h264_v4l2m2m") != NULL; }

// Le profil a un point d'entrée de décodage (VLD).
static int has_vld(VADisplay dpy, VAProfile p) {
    int n = vaMaxNumEntrypoints(dpy);
    if (n <= 0) return 0;
    VAEntrypoint eps[n];
    int got = 0;
    if (vaQueryConfigEntrypoints(dpy, p, eps, &got) != VA_STATUS_SUCCESS) return 0;
    for (int i = 0; i < got; i++) if (eps[i] == VAEntrypointVLD) return 1;
    return 0;
}

// Ce que décode VA-API (premier nœud de rendu qui s'initialise) : bit 0 H.264,
// bit 1 HEVC, bit 2 AV1, bit 3 HEVC 4:4:4. `vendor` : le pilote (« Mesa Gallium
// driver … for AMD Custom GPU 0405 »). -1 : pas de VA-API (pas de nœud de
// rendu, pas de pilote).
int kn_vaapi_probe(char *vendor, int len) {
    for (int node = 128; node < 136; node++) {
        char path[32];
        snprintf(path, sizeof path, "/dev/dri/renderD%d", node);
        int fd = open(path, O_RDWR | O_CLOEXEC);
        if (fd < 0) continue;
        VADisplay dpy = vaGetDisplayDRM(fd);
        int major = 0, minor = 0;
        if (!dpy || vaInitialize(dpy, &major, &minor) != VA_STATUS_SUCCESS) { close(fd); continue; }
        const char *v = vaQueryVendorString(dpy);
        if (vendor && len > 0) snprintf(vendor, len, "%s", v ? v : "");
        int n = vaMaxNumProfiles(dpy);
        int mask = 0;
        if (n > 0) {
            VAProfile profiles[n];
            int got = 0;
            if (vaQueryConfigProfiles(dpy, profiles, &got) == VA_STATUS_SUCCESS) {
                for (int i = 0; i < got; i++) {
                    VAProfile p = profiles[i];
                    if (!has_vld(dpy, p)) continue;
                    if (p == VAProfileH264High || p == VAProfileH264Main || p == VAProfileH264ConstrainedBaseline) mask |= 1;
                    if (p == VAProfileHEVCMain) mask |= 2;
                    if (p == VAProfileAV1Profile0) mask |= 4;
                    if (p == VAProfileHEVCMain444) mask |= 8;
                }
            }
        }
        vaTerminate(dpy);
        close(fd);
        return mask;
    }
    return -1;
}
