/*
 * Banc d'essai de la taille des images (correctifs 0030, docs/debit-automatique.md,
 * « Côté Station : chaque image tient dans sa part ») : encode 600 images 1080p60
 * d'un bureau synthétique (fenêtres, texte, un changement de scène par seconde, un
 * défilement), avec le débit changé en cours (10, puis 4, puis 20 Mbit/s), et dit
 * combien d'images clés sortent, la taille des plus grosses et le retard moyen pour
 * envoyer chaque image sur un lien au débit visé (file comprise).
 *
 *   encoder-frame-bench <encodeur> <avant|apres>
 *   encodeurs : h264_nvenc hevc_nvenc av1_nvenc h264_amf hevc_amf av1_amf
 *               h264_qsv hevc_qsv h264_videotoolbox hevc_videotoolbox libx264
 *
 * « avant » : les options de Kyber d'avant 0030 ; « apres » : celles de
 * kyavservice/src/rate_control.rs. Contre le FFmpeg de kyavserver (l'option
 * kn_vbv_frames vient de son correctif 0030-avcodec-kybernow-vbv). Pas livré.
 * Compilé par build.sh (Windows, à côté de l'essai PyroWave) ; sur le Mac :
 *   cc -O2 -o encoder-frame-bench encoder-frame-bench.c -I<rootfs>/include \
 *      $(PKG_CONFIG_PATH=<rootfs>/lib/pkgconfig pkg-config --static --libs libavcodec libavutil) -lc++
 */
#include <libavcodec/avcodec.h>
#include <libavutil/opt.h>
#include <libavutil/time.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define W 1920
#define H 1080
#define FPS 60
#define FRAMES 600

static unsigned rnd = 1;
static unsigned next_rnd(void) { rnd = rnd * 1103515245u + 12345u; return rnd >> 8; }

static int has(const char *s, const char *part) { return strstr(s, part) != NULL; }

/* Options de Kyber pour cet encodeur (kyavservice, txproto/video.rs). */
static void options(AVDictionary **o, const char *enc, int after)
{
    int h264 = has(enc, "h264"), av1 = has(enc, "av1");
    av_dict_set(o, "b", "10000000", 0);
    av_dict_set(o, "bf", "0", 0);
    if (has(enc, "nvenc")) {
        av_dict_set(o, "preset", "p2", 0);
        av_dict_set(o, "tune", "ull", 0);
        av_dict_set(o, "zerolatency", "1", 0);
        av_dict_set(o, "delay", "1", 0);
        av_dict_set(o, "b_ref_mode", "0", 0);
        av_dict_set(o, "g", "600", 0);
        if (h264) av_dict_set(o, "profile", "high", 0);
        if (after) av_dict_set(o, "rc", "cbr", 0);
    } else if (has(enc, "amf")) {
        av_dict_set(o, "flags", "+low_delay", 0);
        av_dict_set(o, "async_depth", "1", 0);
        av_dict_set(o, "usage", av1 ? "lowlatency" : "ultralowlatency", 0);
        if (h264) av_dict_set(o, "profile", "high", 0);
        if (after) {
            av_dict_set(o, "rc", "cbr", 0);
            av_dict_set(o, "g", "600", 0);
            av_dict_set(o, "enforce_hrd", "1", 0);
            av_dict_set(o, "filler_data", "0", 0);
            av_dict_set(o, "latency", av1 ? "lowest_latency" : "1", 0);
        }
    } else if (has(enc, "qsv")) {
        av_dict_set(o, "preset", "veryfast", 0);
        av_dict_set(o, "g", "600", 0);
        av_dict_set(o, "async_depth", "1", 0);
        if (h264) av_dict_set(o, "profile", "high", 0);
    } else if (has(enc, "libx264")) {
        av_dict_set(o, "preset", "ultrafast", 0);
        av_dict_set(o, "tune", "zerolatency", 0);
        if (after) av_dict_set(o, "g", "600", 0);
    } else if (has(enc, "videotoolbox")) {
        av_dict_set(o, "realtime", "1", 0);
        av_dict_set(o, "prio_speed", "1", 0);
        av_dict_set(o, "power_efficient", "0", 0);
        if (after) av_dict_set(o, "g", "600", 0);
    }
    if (after) av_dict_set(o, "kn_vbv_frames", "1", 0);
}

/* Bureau : aplats et lignes de texte ; nouvelle scène toutes les secondes, défilement
 * d'une bande entre les deux. */
static void draw(AVFrame *f, int i)
{
    if (i % FPS == FPS / 2 || i == 0) {
        rnd = 1000 + i;
        for (int y = 0; y < H; y++) memset(f->data[0] + y * f->linesize[0], 40 + (next_rnd() & 7), W);
        for (int w = 0; w < 12; w++) {
            int x0 = next_rnd() % (W - 400), y0 = next_rnd() % (H - 300);
            int ww = 200 + next_rnd() % 800, hh = 150 + next_rnd() % 500;
            uint8_t bg = 60 + next_rnd() % 190;
            if (x0 + ww > W) ww = W - x0;
            if (y0 + hh > H) hh = H - y0;
            for (int y = y0; y < y0 + hh; y++) {
                uint8_t *row = f->data[0] + y * f->linesize[0];
                int text = ((y - y0) % 18) < 11 && (y - y0) > 30 && ((y - y0) % 18) > 2;
                unsigned seed = (unsigned)((y - y0) / 18) * 7919u + (unsigned)w * 104729u;
                for (int x = x0; x < x0 + ww; x++) {
                    unsigned g = ((unsigned)(x - x0) / 4) * 2654435761u ^ seed;
                    int ink = text && ((g >> 13) % 5) < 2 && (((x - x0) / 40) % 7) != 6;
                    row[x] = ink ? bg - 70 : bg;
                }
            }
        }
        for (int y = 0; y < H / 2; y++)
            for (int x = 0; x < W; x++)
                f->data[1][y * f->linesize[1] + x] = 128 + ((x / 97 + y / 61 + i) & 7) * 4;
    } else {
        for (int y = 600; y < 799; y++)
            memmove(f->data[0] + y * f->linesize[0], f->data[0] + (y + 1) * f->linesize[0], W);
    }
}

int main(int argc, char **argv)
{
    if (argc < 3) {
        fprintf(stderr, "usage : %s <encodeur> <avant|apres>\n", argv[0]);
        return 2;
    }
    const char *enc = argv[1];
    int after = !strcmp(argv[2], "apres");
    av_log_set_level(getenv("KN_VERBOSE") ? AV_LOG_VERBOSE : AV_LOG_WARNING);

    const AVCodec *c = avcodec_find_encoder_by_name(enc);
    if (!c) { fprintf(stderr, "%s : encodeur absent de ce FFmpeg\n", enc); return 1; }
    AVCodecContext *ctx = avcodec_alloc_context3(c);
    ctx->width = W; ctx->height = H; ctx->pix_fmt = AV_PIX_FMT_NV12;
    ctx->time_base = (AVRational){1, 1000000};
    ctx->framerate = (AVRational){FPS, 1};
    ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    AVDictionary *o = NULL;
    options(&o, enc, after);
    int err = avcodec_open2(ctx, c, &o);
    if (err < 0) { fprintf(stderr, "%s : ouverture refusée (%d)\n", enc, err); return 1; }

    AVFrame *f = av_frame_alloc();
    f->width = W; f->height = H; f->format = AV_PIX_FMT_NV12;
    av_frame_get_buffer(f, 0);
    AVPacket *p = av_packet_alloc();
    int64_t bitrate = 10000000, sent_at[FRAMES];
    int keys = 0, largest = 0, packets = 0, late = 0;
    double bytes = 0, encode_ms = 0, link_free = 0, delay_sum = 0, delay_max = 0;

    for (int i = 0; i <= FRAMES; i++) {
        if (i < FRAMES) {
            if (i == 200) ctx->bit_rate = bitrate = 4000000;   /* baisse en direct */
            if (i == 400) ctx->bit_rate = bitrate = 20000000;  /* hausse en direct */
            draw(f, i);
            f->pts = (int64_t)i * 1000000 / FPS;
            sent_at[i] = av_gettime_relative();
        }
        if (avcodec_send_frame(ctx, i < FRAMES ? f : NULL) < 0) { fprintf(stderr, "envoi refusé\n"); return 1; }
        while (avcodec_receive_packet(ctx, p) == 0) {
            int idx = (int)((p->pts * FPS + 500000) / 1000000);
            if (idx < 0 || idx >= FRAMES) idx = FRAMES - 1;
            double now = (double)idx / FPS, delay;
            keys += !!(p->flags & AV_PKT_FLAG_KEY);
            if (p->size > largest) largest = p->size;
            bytes += p->size;
            encode_ms += (av_gettime_relative() - sent_at[idx]) / 1000.0;
            /* Lien au débit visé : l'image part quand le lien est libre. */
            if (link_free < now) link_free = now;
            link_free += p->size * 8.0 / bitrate;
            delay = (link_free - now) * 1000;
            delay_sum += delay;
            if (delay > delay_max) delay_max = delay;
            if (delay > 2000.0 / FPS) late++;
            packets++;
            av_packet_unref(p);
        }
    }
    printf("%s %s : %d image(s) clé(s), plus grosse image %d octets, débit moyen %.1f Mbit/s, "
           "encodage %.1f ms par image, envoi au débit visé %.0f ms en moyenne (au plus %.0f), "
           "%d images en retard de plus de 2 images\n",
           enc, after ? "après" : "avant", keys, largest, bytes * 8 * FPS / FRAMES / 1e6,
           encode_ms / packets, delay_sum / packets, delay_max, late);
    avcodec_free_context(&ctx);
    av_frame_free(&f);
    av_packet_free(&p);
    return 0;
}
