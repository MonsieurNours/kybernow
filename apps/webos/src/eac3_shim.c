/* Encodeur E-AC-3 (Dolby Digital Plus) pour le son 5.1 vers l'ampli :
 * libavcodec (FFmpeg réduit aux encodeurs ac3 et eac3), derrière une petite
 * API C sans structure de FFmpeg côté Rust. Entrée : images entrelacées en
 * flottant, voies dans l'ordre de FFmpeg (G, D, C, LFE, arrière G, arrière D),
 * 1536 images par trame (32 ms à 48 kHz). */
#include <libavcodec/avcodec.h>
#include <libavutil/channel_layout.h>
#include <libavutil/opt.h>
#include <stdlib.h>
#include <string.h>

struct kn_eac3 {
    AVCodecContext *ctx;
    AVFrame *frame;
    AVPacket *pkt;
    int channels;
    int64_t pts;
};

int kn_eac3_frame_size(void) {
    return 1536;
}

/* channels : 2 ou 6 ; bitrate en bit/s. NULL si l'encodeur manque. */
struct kn_eac3 *kn_eac3_open(int channels, int bitrate, int plus) {
    const AVCodec *codec = avcodec_find_encoder_by_name(plus ? "eac3" : "ac3");
    if (!codec) {
        return NULL;
    }
    struct kn_eac3 *e = calloc(1, sizeof *e);
    if (!e) {
        return NULL;
    }
    e->channels = channels;
    e->ctx = avcodec_alloc_context3(codec);
    e->frame = av_frame_alloc();
    e->pkt = av_packet_alloc();
    if (!e->ctx || !e->frame || !e->pkt) {
        goto fail;
    }
    e->ctx->sample_rate = 48000;
    e->ctx->sample_fmt = AV_SAMPLE_FMT_FLTP;
    e->ctx->bit_rate = bitrate;
    /* 5.1 « arrière » (G, D, C, LFE, arrière G, arrière D), la disposition
     * des encodeurs AC-3 ; stéréo sinon. */
    av_channel_layout_from_mask(&e->ctx->ch_layout, channels == 6 ? AV_CH_LAYOUT_5POINT1_BACK : AV_CH_LAYOUT_STEREO);
    if (avcodec_open2(e->ctx, codec, NULL) < 0) {
        goto fail;
    }
    e->frame->nb_samples = e->ctx->frame_size;
    e->frame->format = AV_SAMPLE_FMT_FLTP;
    e->frame->sample_rate = 48000;
    if (av_channel_layout_copy(&e->frame->ch_layout, &e->ctx->ch_layout) < 0 || av_frame_get_buffer(e->frame, 0) < 0) {
        goto fail;
    }
    return e;
fail:
    if (e->ctx) {
        avcodec_free_context(&e->ctx);
    }
    av_frame_free(&e->frame);
    av_packet_free(&e->pkt);
    free(e);
    return NULL;
}

/* Encode une trame de kn_eac3_frame_size() images entrelacées ; écrit au
 * plus `cap` octets dans `out`. Rend la taille, 0 si rien n'est sorti, < 0
 * en cas d'erreur. */
int kn_eac3_encode(struct kn_eac3 *e, const float *interleaved, unsigned char *out, int cap) {
    if (av_frame_make_writable(e->frame) < 0) {
        return -1;
    }
    int n = e->frame->nb_samples;
    for (int c = 0; c < e->channels; c++) {
        float *dst = (float *)e->frame->data[c];
        for (int i = 0; i < n; i++) {
            dst[i] = interleaved[i * e->channels + c];
        }
    }
    e->frame->pts = e->pts;
    e->pts += n;
    if (avcodec_send_frame(e->ctx, e->frame) < 0) {
        return -2;
    }
    int ret = avcodec_receive_packet(e->ctx, e->pkt);
    if (ret < 0) {
        return 0;
    }
    int size = e->pkt->size;
    if (size > cap) {
        size = 0;
    } else {
        memcpy(out, e->pkt->data, size);
    }
    av_packet_unref(e->pkt);
    return size;
}

void kn_eac3_close(struct kn_eac3 *e) {
    if (!e) {
        return;
    }
    avcodec_free_context(&e->ctx);
    av_frame_free(&e->frame);
    av_packet_free(&e->pkt);
    free(e);
}
