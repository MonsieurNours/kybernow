/*
 * kybernow-encoder-check : l'encodeur de l'ordinateur accepte-t-il ce
 * format à ce débit ? (docs/profil-auto.md, « L'encodeur de l'ordinateur »)
 *
 * KyberNow Admin le lance pour `GET /api/stream/encoder` (server/admin/src/
 * encoder.rs) : l'encodeur de FFmpeg réglé pour Kyber (`video.encoder`),
 * ouvert avec les options que Kyber lui donne (kyavservice,
 * avconv/video.rs : configure_nvenc, configure_amf, configure_qsv,
 * configure_vaapi ; VideoToolbox sans option), puis quelques images grises
 * encodées. Les images viennent de la mémoire (VA-API : envoyées sur la
 * carte), pas de la capture : la limite de débit d'un encodeur se joue à
 * son ouverture et à ses premières images, pas à la source.
 *
 *   kybernow-encoder-check --encoder nvenc --codec hevc --width 3840 \
 *       --height 2160 --fps 160 --mbps 593 --yuv444 0 --hdr 0
 *
 * Sortie : 0 accepté ; 1 refusé (raison de FFmpeg sur la sortie d'erreur) ;
 * 2 pas pu essayer (arguments, encodeur absent de ce FFmpeg, format
 * d'images qu'il ne prend pas, carte introuvable).
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/imgutils.h>
#include <libavutil/opt.h>
#include <libavutil/pixdesc.h>

#define FRAMES 8

static void say(const char *what, int err)
{
    char buf[AV_ERROR_MAX_STRING_SIZE] = {0};
    av_strerror(err, buf, sizeof buf);
    fprintf(stderr, "%s : %s\n", what, buf);
}

/* Le nom de l'encodeur de FFmpeg, comme encoder_names() de kyavservice. */
static const char *encoder_name(const char *encoder, const char *codec)
{
    static char name[64];
    const char *suffix = NULL;
    if (!strcmp(encoder, "nvenc")) suffix = "nvenc";
    else if (!strcmp(encoder, "amf")) suffix = "amf";
    else if (!strcmp(encoder, "qsv")) suffix = "qsv";
    else if (!strcmp(encoder, "vaapi")) suffix = "vaapi";
    else if (!strcmp(encoder, "videotoolbox")) suffix = "videotoolbox";
    else return NULL;
    snprintf(name, sizeof name, "%s_%s", codec, suffix);
    return name;
}

/* Les options de Kyber pour cet encodeur (configure_* de kyavservice). */
static void kyber_options(AVDictionary **o, const char *encoder, const char *codec, int yuv444, int hdr)
{
    int h264 = !strcmp(codec, "h264"), hevc = !strcmp(codec, "hevc");
    if (!strcmp(encoder, "nvenc")) {
        av_dict_set(o, "preset", "p2", 0);
        av_dict_set(o, "tune", "ull", 0);
        av_dict_set(o, "zerolatency", "1", 0);
        av_dict_set(o, "delay", "1", 0);
        av_dict_set(o, "bf", "0", 0);
        av_dict_set(o, "b_ref_mode", "0", 0);
        av_dict_set(o, "g", "600", 0);
        if (h264) av_dict_set(o, "profile", yuv444 ? "high444p" : "high", 0);
    } else if (!strcmp(encoder, "amf")) {
        av_dict_set(o, "bf", "0", 0);
        av_dict_set(o, "max_b_frames", "0", 0);
        av_dict_set(o, "async_depth", "1", 0);
        av_dict_set(o, "flags", "+low_delay", 0);
        av_dict_set(o, "usage", (h264 || hevc) ? "ultralowlatency" : "lowlatency", 0);
        if (h264) av_dict_set(o, "profile", "high", 0);
    } else if (!strcmp(encoder, "qsv")) {
        av_dict_set(o, "preset", "veryfast", 0);
        av_dict_set(o, "bf", "0", 0);
        av_dict_set(o, "g", "600", 0);
        av_dict_set(o, "async_depth", "1", 0);
        if (h264) av_dict_set(o, "profile", "high", 0);
        if (hevc && yuv444) av_dict_set(o, "profile", "rext", 0);
    } else if (!strcmp(encoder, "vaapi")) {
        av_dict_set(o, "bf", "0", 0);
        av_dict_set(o, "async_depth", "1", 0);
        if (h264) av_dict_set(o, "profile", "high", 0);
    }
    /* KyberNow (0022) : HEVC Main 10 en HDR. */
    if (hdr && hevc && strcmp(encoder, "videotoolbox") && strcmp(encoder, "vaapi"))
        av_dict_set(o, "profile", "main10", 0);
    /* KyberNow (0030) : contrôle du débit (kyavservice, rate_control.rs) :
     * mémoire d'une image (FFmpeg 0030-avcodec-kybernow-vbv ; option
     * ignorée par un FFmpeg sans ce correctif), débit constant, image clé
     * toutes les 600 images, AMF en faible retard. */
    av_dict_set(o, "kn_vbv_frames", "1", 0);
    if (!strcmp(encoder, "nvenc") || !strcmp(encoder, "amf"))
        av_dict_set(o, "rc", "cbr", 0);
    if (!strcmp(encoder, "amf") || !strcmp(encoder, "vaapi") || !strcmp(encoder, "videotoolbox"))
        av_dict_set(o, "g", "600", 0);
    if (!strcmp(encoder, "amf")) {
        av_dict_set(o, "enforce_hrd", "1", 0);
        av_dict_set(o, "filler_data", "0", 0);
        av_dict_set(o, "latency", (h264 || hevc) ? "1" : "lowest_latency", 0);
    }
}

/* Le format des images : NV12, 4:4:4 planaire, P010 en HDR. */
static enum AVPixelFormat wanted_format(int yuv444, int hdr)
{
    if (hdr) return AV_PIX_FMT_P010LE;
    return yuv444 ? AV_PIX_FMT_YUV444P : AV_PIX_FMT_NV12;
}

static int supports(const AVCodec *c, const AVCodecContext *ctx, enum AVPixelFormat f)
{
    const enum AVPixelFormat *fmts = NULL;
#if LIBAVCODEC_VERSION_INT >= AV_VERSION_INT(61, 13, 100)
    if (avcodec_get_supported_config(ctx, c, AV_CODEC_CONFIG_PIX_FORMAT, 0, (const void **)&fmts, NULL) < 0)
        return 0;
#else
    (void)ctx;
    fmts = c->pix_fmts;
#endif
    if (!fmts) return 1; /* rien d'annoncé : on essaie */
    for (; *fmts != AV_PIX_FMT_NONE; fmts++)
        if (*fmts == f) return 1;
    return 0;
}

/* VA-API : un appareil et un ensemble d'images sur la carte (comme Kyber,
 * qui encode des surfaces VA). */
static int vaapi_frames(AVCodecContext *ctx, AVBufferRef **device, enum AVPixelFormat sw)
{
    int err = av_hwdevice_ctx_create(device, AV_HWDEVICE_TYPE_VAAPI, getenv("KYBERNOW_VAAPI_DEVICE"), NULL, 0);
    if (err < 0) {
        say("appareil VA-API", err);
        return err;
    }
    AVBufferRef *frames = av_hwframe_ctx_alloc(*device);
    if (!frames) return AVERROR(ENOMEM);
    AVHWFramesContext *fc = (AVHWFramesContext *)frames->data;
    fc->format = AV_PIX_FMT_VAAPI;
    fc->sw_format = sw;
    fc->width = ctx->width;
    fc->height = ctx->height;
    fc->initial_pool_size = 4;
    if ((err = av_hwframe_ctx_init(frames)) < 0) {
        say("images VA-API", err);
        av_buffer_unref(&frames);
        return err;
    }
    ctx->hw_frames_ctx = frames;
    ctx->pix_fmt = AV_PIX_FMT_VAAPI;
    return 0;
}

static int usage(void)
{
    fprintf(stderr, "usage : kybernow-encoder-check --encoder E --codec hevc|h264|av1 --width W --height H --fps F --mbps M [--yuv444 0|1] [--hdr 0|1]\n");
    return 2;
}

int main(int argc, char **argv)
{
    const char *encoder = NULL, *codec = NULL;
    long width = 0, height = 0, fps = 0, mbps = 0, yuv444 = 0, hdr = 0;
    for (int i = 1; i + 1 < argc; i += 2) {
        const char *k = argv[i], *v = argv[i + 1];
        if (!strcmp(k, "--encoder")) encoder = v;
        else if (!strcmp(k, "--codec")) codec = v;
        else if (!strcmp(k, "--width")) width = strtol(v, NULL, 10);
        else if (!strcmp(k, "--height")) height = strtol(v, NULL, 10);
        else if (!strcmp(k, "--fps")) fps = strtol(v, NULL, 10);
        else if (!strcmp(k, "--mbps")) mbps = strtol(v, NULL, 10);
        else if (!strcmp(k, "--yuv444")) yuv444 = strtol(v, NULL, 10);
        else if (!strcmp(k, "--hdr")) hdr = strtol(v, NULL, 10);
        else return usage();
    }
    if (!encoder || !codec || width < 64 || height < 64 || width > 16384 || height > 16384 || fps < 1 || fps > 500 || mbps < 1 || mbps > 4000)
        return usage();
    if (strcmp(codec, "hevc") && strcmp(codec, "h264") && strcmp(codec, "av1"))
        return usage();
    if (!strcmp(encoder, "x264"))
        return 0; /* logiciel : pas de limite de débit à chercher */

    av_log_set_level(AV_LOG_ERROR);
    const char *name = encoder_name(encoder, codec);
    const AVCodec *c = name ? avcodec_find_encoder_by_name(name) : NULL;
    if (!c) {
        fprintf(stderr, "encodeur %s absent de ce FFmpeg\n", name ? name : encoder);
        return 2;
    }
    AVCodecContext *ctx = avcodec_alloc_context3(c);
    if (!ctx) return 2;
    ctx->width = (int)width;
    ctx->height = (int)height;
    ctx->time_base = (AVRational){1, (int)fps};
    ctx->framerate = (AVRational){(int)fps, 1};
    ctx->bit_rate = (int64_t)mbps * 1000000;
    ctx->max_b_frames = 0;
    if (hdr) {
        ctx->color_primaries = AVCOL_PRI_BT2020;
        ctx->color_trc = AVCOL_TRC_SMPTE2084;
        ctx->colorspace = AVCOL_SPC_BT2020_NCL;
    } else {
        ctx->color_primaries = AVCOL_PRI_BT709;
        ctx->color_trc = AVCOL_TRC_BT709;
        ctx->colorspace = AVCOL_SPC_BT709;
    }
    ctx->color_range = AVCOL_RANGE_MPEG;

    enum AVPixelFormat sw = wanted_format((int)yuv444, (int)hdr);
    AVBufferRef *device = NULL;
    int vaapi = !strcmp(encoder, "vaapi");
    if (vaapi) {
        if (vaapi_frames(ctx, &device, sw) < 0) return 2;
    } else {
        if (!supports(c, ctx, sw)) {
            fprintf(stderr, "%s ne prend pas les images %s\n", name, av_get_pix_fmt_name(sw));
            return 2;
        }
        ctx->pix_fmt = sw;
    }

    AVDictionary *opts = NULL;
    kyber_options(&opts, encoder, codec, (int)yuv444, (int)hdr);
    int err = avcodec_open2(ctx, c, &opts);
    av_dict_free(&opts);
    if (err < 0) {
        say("ouverture de l'encodeur", err);
        return 1;
    }

    AVFrame *frame = av_frame_alloc(), *hw = NULL;
    AVPacket *pkt = av_packet_alloc();
    frame->format = sw;
    frame->width = ctx->width;
    frame->height = ctx->height;
    if ((err = av_frame_get_buffer(frame, 0)) < 0) {
        say("images", err);
        return 2;
    }
    /* Gris moyen : luminance et chrominance à mi-chemin (8 ou 10 bits). */
    for (int p = 0; p < 4 && frame->data[p]; p++) {
        int rows = (p > 0 && !yuv444) ? ctx->height / 2 : ctx->height;
        if (hdr) {
            for (int y = 0; y < rows; y++) {
                uint16_t *row = (uint16_t *)(frame->data[p] + (size_t)y * frame->linesize[p]);
                for (int x = 0; x < frame->linesize[p] / 2; x++) row[x] = 512 << 6;
            }
        } else {
            memset(frame->data[p], 128, (size_t)frame->linesize[p] * rows);
        }
    }
    if (vaapi) {
        hw = av_frame_alloc();
        if ((err = av_hwframe_get_buffer(ctx->hw_frames_ctx, hw, 0)) < 0 || (err = av_hwframe_transfer_data(hw, frame, 0)) < 0) {
            say("envoi de l'image sur la carte", err);
            return 2;
        }
    }

    int packets = 0;
    for (int i = 0; i <= FRAMES; i++) {
        AVFrame *in = NULL;
        if (i < FRAMES) {
            in = vaapi ? hw : frame;
            in->pts = i;
        }
        if ((err = avcodec_send_frame(ctx, in)) < 0) {
            say("encodage", err);
            return 1;
        }
        while ((err = avcodec_receive_packet(ctx, pkt)) >= 0) {
            packets++;
            av_packet_unref(pkt);
        }
        if (err != AVERROR(EAGAIN) && err != AVERROR_EOF) {
            say("encodage", err);
            return 1;
        }
    }
    if (packets == 0) {
        fprintf(stderr, "encodage : aucune image sortie\n");
        return 1;
    }
    av_packet_free(&pkt);
    av_frame_free(&frame);
    av_frame_free(&hw);
    avcodec_free_context(&ctx);
    av_buffer_unref(&device);
    return 0;
}
