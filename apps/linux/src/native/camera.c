// La caméra de l'appareil vers l'ordinateur (docs/webcam.md) : une webcam
// V4L2 (USB, intégrée) lue en NV12, YUYV ou MJPEG, mise en NV12, puis
// encodée en H.264 par le FFmpeg de l'app : VA-API (cartes Intel et AMD, le
// Steam Deck) d'abord, sinon V4L2 M2M (Raspberry Pi 4). Temps réel : débit
// constant, sans images B, une image clé toutes les 2 s et à la demande.
// Chaque image encodée (Annex B, SPS et PPS devant les images clés) part au
// cœur par `src/device/camera.rs`.

#include <errno.h>
#include <fcntl.h>
#include <linux/videodev2.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>

#define BUFFERS 4

// Les webcams, une par ligne : chemin, tabulation, nom. Rend le nombre.
int kn_camera_list(char *out, int size)
{
    int n = 0, used = 0;
    if (size > 0)
        out[0] = 0;
    for (int i = 0; i < 64; i++) {
        char path[32];
        snprintf(path, sizeof path, "/dev/video%d", i);
        int fd = open(path, O_RDWR | O_NONBLOCK);
        if (fd < 0)
            continue;
        struct v4l2_capability cap;
        memset(&cap, 0, sizeof cap);
        int ok = ioctl(fd, VIDIOC_QUERYCAP, &cap) == 0;
        close(fd);
        uint32_t caps = cap.capabilities & V4L2_CAP_DEVICE_CAPS ? cap.device_caps : cap.capabilities;
        // Une vraie caméra (pas les nœuds de métadonnées, pas une caméra virtuelle de KyberNow).
        if (!ok || !(caps & V4L2_CAP_VIDEO_CAPTURE) || !(caps & V4L2_CAP_STREAMING)
            || strcmp((const char *)cap.card, "KyberNow Camera") == 0)
            continue;
        int w = snprintf(out + used, size > used ? size - used : 0, "%s\t%s\n", path, (const char *)cap.card);
        if (w > 0 && used + w < size)
            used += w;
        n++;
    }
    return n;
}

typedef struct kn_cam {
    int fd;
    int width, height;
    uint32_t fourcc;
    struct { void *data; size_t size; } bufs[BUFFERS];
    AVCodecContext *enc;
    AVBufferRef *device;
    int hw;                 // VA-API : l'image passe par une surface
    AVFrame *sw;            // l'image NV12 en mémoire
    AVPacket *pkt;
    AVCodecContext *mjpeg;  // webcam en MJPEG
    AVFrame *decoded;
    int64_t start_us;
    int force_key;
    uint8_t *out;           // l'image rendue (paramètres devant une image clé)
    size_t out_cap;
    int stride;             // octets par ligne de la webcam (bytesperline : un pilote peut aligner ses lignes)
    uint8_t *jpeg;          // copie d'une image MJPEG avec la marge que FFmpeg exige
    size_t jpeg_cap;
} kn_cam;

static int64_t now_us(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;
}

static int xioctl(int fd, unsigned long req, void *arg)
{
    int r;
    do
        r = ioctl(fd, req, arg);
    while (r < 0 && errno == EINTR);
    return r;
}

static int try_format(int fd, uint32_t fourcc, int w, int h, struct v4l2_format *fmt)
{
    memset(fmt, 0, sizeof *fmt);
    fmt->type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    fmt->fmt.pix.width = w;
    fmt->fmt.pix.height = h;
    fmt->fmt.pix.pixelformat = fourcc;
    fmt->fmt.pix.field = V4L2_FIELD_ANY;
    return xioctl(fd, VIDIOC_S_FMT, fmt) == 0 && fmt->fmt.pix.pixelformat == fourcc;
}

// Le débit de la webcam à cette taille et ce format, en images par seconde (0 : inconnu).
static int max_fps(int fd, uint32_t fourcc, int w, int h)
{
    struct v4l2_frmivalenum iv;
    int best = 0;
    for (int i = 0;; i++) {
        memset(&iv, 0, sizeof iv);
        iv.index = i;
        iv.pixel_format = fourcc;
        iv.width = w;
        iv.height = h;
        if (xioctl(fd, VIDIOC_ENUM_FRAMEINTERVALS, &iv) < 0)
            break;
        if (iv.type == V4L2_FRMIVAL_TYPE_DISCRETE && iv.discrete.numerator) {
            int fps = iv.discrete.denominator / iv.discrete.numerator;
            if (fps > best)
                best = fps;
        } else {
            return 30;
        }
    }
    return best;
}

static AVCodecContext *open_encoder(const char *name, int w, int h, int bitrate, AVBufferRef **device, int *hw)
{
    const AVCodec *codec = avcodec_find_encoder_by_name(name);
    if (!codec)
        return NULL;
    AVCodecContext *c = avcodec_alloc_context3(codec);
    c->width = w;
    c->height = h;
    c->time_base = (AVRational){ 1, 1000000 };
    c->framerate = (AVRational){ 30, 1 };
    c->bit_rate = bitrate;
    c->rc_max_rate = bitrate;
    c->rc_buffer_size = bitrate / 2;
    c->gop_size = 60;
    c->max_b_frames = 0;
    c->flags |= AV_CODEC_FLAG_LOW_DELAY;
    *hw = 0;
    if (strcmp(name, "h264_vaapi") == 0) {
        if (av_hwdevice_ctx_create(device, AV_HWDEVICE_TYPE_VAAPI, NULL, NULL, 0) < 0) {
            avcodec_free_context(&c);
            return NULL;
        }
        AVBufferRef *frames = av_hwframe_ctx_alloc(*device);
        AVHWFramesContext *fc = (AVHWFramesContext *)frames->data;
        fc->format = AV_PIX_FMT_VAAPI;
        fc->sw_format = AV_PIX_FMT_NV12;
        fc->width = w;
        fc->height = h;
        fc->initial_pool_size = 8;
        if (av_hwframe_ctx_init(frames) < 0) {
            av_buffer_unref(&frames);
            av_buffer_unref(device);
            avcodec_free_context(&c);
            return NULL;
        }
        c->pix_fmt = AV_PIX_FMT_VAAPI;
        c->hw_frames_ctx = frames;
        av_opt_set(c->priv_data, "rc_mode", "CBR", 0);
        av_opt_set_int(c->priv_data, "low_power", 0, 0);
        *hw = 1;
    } else {
        c->pix_fmt = AV_PIX_FMT_NV12;
    }
    if (avcodec_open2(c, codec, NULL) < 0) {
        av_buffer_unref(device);
        avcodec_free_context(&c);
        return NULL;
    }
    return c;
}

// Un encodeur H.264 matériel s'ouvre sur cet appareil (VA-API, sinon V4L2 M2M).
int kn_camera_encoder_available(void)
{
    static int known = -1;
    if (known >= 0)
        return known;
    AVBufferRef *dev = NULL;
    int hw = 0;
    AVCodecContext *c = open_encoder("h264_vaapi", 1280, 720, 1500000, &dev, &hw);
    if (!c)
        c = open_encoder("h264_v4l2m2m", 1280, 720, 1500000, &dev, &hw);
    known = c != NULL;
    avcodec_free_context(&c);
    av_buffer_unref(&dev);
    return known;
}

void kn_cam_close(kn_cam *c)
{
    if (!c)
        return;
    if (c->fd >= 0) {
        enum v4l2_buf_type t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        xioctl(c->fd, VIDIOC_STREAMOFF, &t);
        for (int i = 0; i < BUFFERS; i++)
            if (c->bufs[i].data)
                munmap(c->bufs[i].data, c->bufs[i].size);
        close(c->fd);
    }
    avcodec_free_context(&c->enc);
    avcodec_free_context(&c->mjpeg);
    av_buffer_unref(&c->device);
    av_frame_free(&c->sw);
    av_frame_free(&c->decoded);
    av_packet_free(&c->pkt);
    free(c->out);
    free(c->jpeg);
    free(c);
}

// Ouvre la webcam `path` à w×h (ou la plus proche), 30 images par seconde, et
// l'encodeur. `error` : 1 prise par une autre app, 2 absente, 3 pas d'encodeur.
kn_cam *kn_cam_open(const char *path, int w, int h, int bitrate, int *out_w, int *out_h, int *error)
{
    *error = 0;
    kn_cam *c = calloc(1, sizeof *c);
    if (!c) {
        *error = 2;
        return NULL;
    }
    c->fd = open(path, O_RDWR | O_NONBLOCK);
    if (c->fd < 0) {
        *error = errno == EBUSY ? 1 : 2;
        free(c);
        return NULL;
    }
    // NV12 ou YUYV s'ils tiennent 30 images par seconde à cette taille ; sinon MJPEG (USB 2).
    struct v4l2_format fmt;
    uint32_t order[] = { V4L2_PIX_FMT_NV12, V4L2_PIX_FMT_YUYV, V4L2_PIX_FMT_MJPEG };
    int chosen = 0;
    for (int i = 0; i < 3 && !chosen; i++) {
        if (!try_format(c->fd, order[i], w, h, &fmt))
            continue;
        if (order[i] != V4L2_PIX_FMT_MJPEG && max_fps(c->fd, order[i], fmt.fmt.pix.width, fmt.fmt.pix.height) < 25)
            continue;
        chosen = 1;
    }
    if (!chosen && !try_format(c->fd, V4L2_PIX_FMT_YUYV, w, h, &fmt)) {
        *error = 2;
        kn_cam_close(c);
        return NULL;
    }
    c->fourcc = fmt.fmt.pix.pixelformat;
    c->width = fmt.fmt.pix.width & ~1;
    // Le pas des lignes que donne le pilote (caméra CSI du Pi, ISP : lignes alignées), sinon la largeur.
    {
        int min = (int)fmt.fmt.pix.width * (fmt.fmt.pix.pixelformat == V4L2_PIX_FMT_YUYV ? 2 : 1);
        c->stride = (int)fmt.fmt.pix.bytesperline >= min ? (int)fmt.fmt.pix.bytesperline : min;
    }
    c->height = fmt.fmt.pix.height & ~1;
    struct v4l2_streamparm parm;
    memset(&parm, 0, sizeof parm);
    parm.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    parm.parm.capture.timeperframe.numerator = 1;
    parm.parm.capture.timeperframe.denominator = 30;
    xioctl(c->fd, VIDIOC_S_PARM, &parm);

    struct v4l2_requestbuffers req;
    memset(&req, 0, sizeof req);
    req.count = BUFFERS;
    req.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    req.memory = V4L2_MEMORY_MMAP;
    if (xioctl(c->fd, VIDIOC_REQBUFS, &req) < 0 || req.count < 2) {
        *error = errno == EBUSY ? 1 : 2;
        kn_cam_close(c);
        return NULL;
    }
    for (unsigned i = 0; i < req.count && i < BUFFERS; i++) {
        struct v4l2_buffer b;
        memset(&b, 0, sizeof b);
        b.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        b.memory = V4L2_MEMORY_MMAP;
        b.index = i;
        if (xioctl(c->fd, VIDIOC_QUERYBUF, &b) < 0)
            break;
        c->bufs[i].size = b.length;
        c->bufs[i].data = mmap(NULL, b.length, PROT_READ | PROT_WRITE, MAP_SHARED, c->fd, b.m.offset);
        if (c->bufs[i].data == MAP_FAILED) {
            c->bufs[i].data = NULL;
            break;
        }
        xioctl(c->fd, VIDIOC_QBUF, &b);
    }
    enum v4l2_buf_type t = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    if (xioctl(c->fd, VIDIOC_STREAMON, &t) < 0) {
        *error = errno == EBUSY ? 1 : 2;
        kn_cam_close(c);
        return NULL;
    }

    c->enc = open_encoder("h264_vaapi", c->width, c->height, bitrate, &c->device, &c->hw);
    if (!c->enc)
        c->enc = open_encoder("h264_v4l2m2m", c->width, c->height, bitrate, &c->device, &c->hw);
    if (!c->enc) {
        *error = 3;
        kn_cam_close(c);
        return NULL;
    }
    if (c->fourcc == V4L2_PIX_FMT_MJPEG) {
        const AVCodec *d = avcodec_find_decoder(AV_CODEC_ID_MJPEG);
        c->mjpeg = d ? avcodec_alloc_context3(d) : NULL;
        if (!c->mjpeg || avcodec_open2(c->mjpeg, d, NULL) < 0) {
            *error = 3;
            kn_cam_close(c);
            return NULL;
        }
        c->decoded = av_frame_alloc();
    }
    c->sw = av_frame_alloc();
    c->pkt = av_packet_alloc();
    if (!c->sw || !c->pkt) {
        *error = 3;
        kn_cam_close(c);
        return NULL;
    }
    c->sw->format = AV_PIX_FMT_NV12;
    c->sw->width = c->width;
    c->sw->height = c->height;
    if (av_frame_get_buffer(c->sw, 32) < 0) {
        *error = 3;
        kn_cam_close(c);
        return NULL;
    }
    c->start_us = now_us();
    c->force_key = 1;
    *out_w = c->width;
    *out_h = c->height;
    return c;
}

// YUYV (Y0 U Y1 V) vers NV12.
static void yuyv_to_nv12(const uint8_t *src, int stride, AVFrame *f)
{
    for (int y = 0; y < f->height; y++) {
        const uint8_t *s = src + (size_t)y * stride;
        uint8_t *dy = f->data[0] + (size_t)y * f->linesize[0];
        for (int x = 0; x < f->width; x++)
            dy[x] = s[2 * x];
        if (!(y & 1)) {
            uint8_t *duv = f->data[1] + (size_t)(y / 2) * f->linesize[1];
            for (int x = 0; x < f->width; x += 2) {
                duv[x] = s[2 * x + 1];
                duv[x + 1] = s[2 * x + 3];
            }
        }
    }
}

// YUV planaire (4:2:0 ou 4:2:2, MJPEG décodé) vers NV12.
static void planar_to_nv12(const AVFrame *p, AVFrame *f)
{
    int vsub = p->format == AV_PIX_FMT_YUVJ420P || p->format == AV_PIX_FMT_YUV420P ? 1 : 0;
    for (int y = 0; y < f->height; y++)
        memcpy(f->data[0] + (size_t)y * f->linesize[0], p->data[0] + (size_t)y * p->linesize[0], f->width);
    for (int y = 0; y < f->height / 2; y++) {
        int sy = vsub ? y : 2 * y;
        const uint8_t *u = p->data[1] + (size_t)sy * p->linesize[1];
        const uint8_t *v = p->data[2] + (size_t)sy * p->linesize[2];
        uint8_t *d = f->data[1] + (size_t)y * f->linesize[1];
        for (int x = 0; x < f->width / 2; x++) {
            d[2 * x] = u[x];
            d[2 * x + 1] = v[x];
        }
    }
}

static int starts_with_sps(const uint8_t *d, int n)
{
    int at = n > 4 && !d[0] && !d[1] && !d[2] && d[3] == 1 ? 4 : n > 3 && !d[0] && !d[1] && d[2] == 1 ? 3 : -1;
    return at > 0 && (d[at] & 0x1f) == 7;
}

void kn_cam_force_key(kn_cam *c)
{
    c->force_key = 1;
}

void kn_cam_set_bitrate(kn_cam *c, int bitrate)
{
    // h264_vaapi et h264_v4l2m2m ne relisent pas bit_rate après l'ouverture : le débit fixé à l'ouverture
    // reste (docs/webcam.md, « le débit ne change pas en route »). Gardé pour un encodeur qui le relirait ;
    // la caméra suivante (ou la même rallumée) prend le nouveau débit.
    if (bitrate > 0 && c->enc) {
        c->enc->bit_rate = bitrate;
        c->enc->rc_max_rate = bitrate;
    }
}

// La prochaine image encodée (attend la webcam 1 s au plus) : 1 une image,
// 0 rien encore, -1 la webcam est partie.
int kn_cam_next(kn_cam *c, const uint8_t **data, int *size, int64_t *pts_us, int *key)
{
    // Une sortie déjà prête de l'encodeur ?
    int r = avcodec_receive_packet(c->enc, c->pkt);
    if (r == AVERROR(EAGAIN)) {
        struct pollfd p = { c->fd, POLLIN, 0 };
        int pr = poll(&p, 1, 1000);
        if (pr == 0)
            return 0;
        if (pr < 0 || (p.revents & (POLLERR | POLLHUP)))
            return -1;
        struct v4l2_buffer b;
        memset(&b, 0, sizeof b);
        b.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        b.memory = V4L2_MEMORY_MMAP;
        if (xioctl(c->fd, VIDIOC_DQBUF, &b) < 0)
            return errno == EAGAIN ? 0 : -1;
        int ok = 1;
        av_frame_make_writable(c->sw);
        const uint8_t *src = c->bufs[b.index].data;
        if (c->fourcc == V4L2_PIX_FMT_YUYV) {
            yuyv_to_nv12(src, c->stride, c->sw);
        } else if (c->fourcc == V4L2_PIX_FMT_NV12) {
            for (int y = 0; y < c->height; y++)
                memcpy(c->sw->data[0] + (size_t)y * c->sw->linesize[0], src + (size_t)y * c->stride, c->width);
            for (int y = 0; y < c->height / 2; y++)
                memcpy(c->sw->data[1] + (size_t)y * c->sw->linesize[1], src + (size_t)c->stride * c->height + (size_t)y * c->stride, c->width);
        } else {
            // Le décodeur MJPEG lit au-delà de la fin du paquet : une copie, complétée de
            // AV_INPUT_BUFFER_PADDING_SIZE octets nuls (le tampon de la webcam n'en a pas).
            size_t need = (size_t)b.bytesused + AV_INPUT_BUFFER_PADDING_SIZE;
            if (need > c->jpeg_cap) {
                uint8_t *n = realloc(c->jpeg, need);
                if (!n) {
                    xioctl(c->fd, VIDIOC_QBUF, &b);
                    return 0;
                }
                c->jpeg = n;
                c->jpeg_cap = need;
            }
            memcpy(c->jpeg, src, b.bytesused);
            memset(c->jpeg + b.bytesused, 0, AV_INPUT_BUFFER_PADDING_SIZE);
            AVPacket *jp = av_packet_alloc();
            if (!jp) {
                xioctl(c->fd, VIDIOC_QBUF, &b);
                return 0;
            }
            jp->data = c->jpeg;
            jp->size = b.bytesused;
            ok = avcodec_send_packet(c->mjpeg, jp) >= 0 && avcodec_receive_frame(c->mjpeg, c->decoded) >= 0
                 && c->decoded->width >= c->width && c->decoded->height >= c->height;
            if (ok)
                planar_to_nv12(c->decoded, c->sw);
            av_frame_unref(c->decoded);
            av_packet_free(&jp);
        }
        xioctl(c->fd, VIDIOC_QBUF, &b);
        if (!ok)
            return 0;
        AVFrame *in = c->sw;
        AVFrame *surface = NULL;
        if (c->hw) {
            surface = av_frame_alloc();
            if (av_hwframe_get_buffer(c->enc->hw_frames_ctx, surface, 0) < 0 || av_hwframe_transfer_data(surface, c->sw, 0) < 0) {
                av_frame_free(&surface);
                return 0;
            }
            in = surface;
        }
        in->pts = now_us() - c->start_us;
        in->pict_type = c->force_key ? AV_PICTURE_TYPE_I : AV_PICTURE_TYPE_NONE;
        c->force_key = 0;
        r = avcodec_send_frame(c->enc, in);
        av_frame_free(&surface);
        if (r < 0)
            return 0;
        r = avcodec_receive_packet(c->enc, c->pkt);
        if (r == AVERROR(EAGAIN))
            return 0;
    }
    if (r < 0)
        return 0;
    int k = (c->pkt->flags & AV_PKT_FLAG_KEY) != 0;
    int extra = k && !starts_with_sps(c->pkt->data, c->pkt->size) ? c->enc->extradata_size : 0;
    size_t need = (size_t)c->pkt->size + extra;
    if (need > c->out_cap) {
        free(c->out);
        c->out_cap = need * 2;
        c->out = malloc(c->out_cap);
        if (!c->out) {
            c->out_cap = 0;
            av_packet_unref(c->pkt);
            return 0;
        }
    }
    if (extra)
        memcpy(c->out, c->enc->extradata, extra);
    memcpy(c->out + extra, c->pkt->data, c->pkt->size);
    *data = c->out;
    *size = (int)need;
    *pts_us = c->pkt->pts;
    *key = k;
    av_packet_unref(c->pkt);
    return 1;
}
