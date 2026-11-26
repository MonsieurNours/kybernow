/*
 * kybernow-camera, Station Linux (docs/webcam.md, « Station Linux »).
 *
 * 1. v4l2loopback : un périphérique `/dev/videoN` de ce pilote, celui nommé
 *    « KyberNow Camera » d'abord (créé au démarrage par
 *    server/camera/linux/kybernow-camera.conf, `exclusive_caps=1` : les
 *    logiciels n'y voient une caméra que quand on y écrit), sinon le premier
 *    libre. Images YUYV écrites par write() : Zoom, Discord, Chrome,
 *    Firefox, OBS la lisent toutes.
 * 2. Sinon PipeWire (compilé avec HAVE_PIPEWIRE) : un nœud `Video/Source`
 *    « KyberNow Camera », vu par les logiciels qui lisent les caméras de
 *    PipeWire (navigateurs par le portail Caméra, OBS) ; il n'existe que
 *    tant que la caméra est envoyée.
 */
#include "camera.h"

#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/videodev2.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#ifdef HAVE_PIPEWIRE
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/video/type-info.h>
#endif

/* ---- v4l2loopback ---- */

struct v4l2_out {
    int fd;
    char path[64];
};

/* Le descripteur d'un périphérique v4l2loopback (`card` : son nom, ou NULL
 * pour le premier), ou -1. */
static int v4l2_find(const char *card, char *path, size_t size)
{
    DIR *dir = opendir("/dev");
    if (!dir)
        return -1;
    struct dirent *e;
    int found = -1;
    while (found < 0 && (e = readdir(dir))) {
        if (strncmp(e->d_name, "video", 5) != 0)
            continue;
        char p[64];
        snprintf(p, sizeof p, "/dev/%s", e->d_name);
        int fd = open(p, O_RDWR | O_NONBLOCK);
        if (fd < 0)
            continue;
        struct v4l2_capability cap;
        memset(&cap, 0, sizeof cap);
        if (ioctl(fd, VIDIOC_QUERYCAP, &cap) == 0 && strstr((const char *)cap.driver, "v4l2 loopback")
            && (!card || strcmp((const char *)cap.card, card) == 0)) {
            found = fd;
            snprintf(path, size, "%s", p);
        } else {
            close(fd);
        }
    }
    closedir(dir);
    return found;
}

static int v4l2_open(struct kn_output *out, int w, int h)
{
    struct v4l2_out *v = calloc(1, sizeof *v);
    v->fd = v4l2_find(KN_CAMERA_NAME, v->path, sizeof v->path);
    if (v->fd < 0)
        v->fd = v4l2_find(NULL, v->path, sizeof v->path);
    if (v->fd < 0) {
        free(v);
        return -1;
    }
    /* Écritures bloquantes : le rythme est le nôtre. */
    fcntl(v->fd, F_SETFL, fcntl(v->fd, F_GETFL) & ~O_NONBLOCK);
    struct v4l2_format fmt;
    memset(&fmt, 0, sizeof fmt);
    fmt.type = V4L2_BUF_TYPE_VIDEO_OUTPUT;
    fmt.fmt.pix.width = w;
    fmt.fmt.pix.height = h;
    fmt.fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
    fmt.fmt.pix.field = V4L2_FIELD_NONE;
    fmt.fmt.pix.bytesperline = w * 2;
    fmt.fmt.pix.sizeimage = w * h * 2;
    fmt.fmt.pix.colorspace = V4L2_COLORSPACE_REC709;
    if (ioctl(v->fd, VIDIOC_S_FMT, &fmt) < 0) {
        kn_log("%s: format %dx%d YUYV refused (%s)", v->path, w, h, strerror(errno));
        close(v->fd);
        free(v);
        return -1;
    }
    struct v4l2_streamparm parm;
    memset(&parm, 0, sizeof parm);
    parm.type = V4L2_BUF_TYPE_VIDEO_OUTPUT;
    parm.parm.output.timeperframe.numerator = 1;
    parm.parm.output.timeperframe.denominator = KN_CAMERA_FPS;
    ioctl(v->fd, VIDIOC_S_PARM, &parm);
    kn_log("v4l2loopback on %s", v->path);
    out->priv = v;
    return 0;
}

static int v4l2_write(struct kn_output *out, const uint8_t *data, size_t size, int64_t pts)
{
    (void)pts;
    struct v4l2_out *v = out->priv;
    size_t done = 0;
    while (done < size) {
        ssize_t n = write(v->fd, data + done, size - done);
        if (n < 0) {
            if (errno == EINTR)
                continue;
            return -1;
        }
        done += (size_t)n;
    }
    return 0;
}

static void v4l2_close(struct kn_output *out)
{
    struct v4l2_out *v = out->priv;
    if (v) {
        close(v->fd);
        free(v);
    }
    out->priv = NULL;
}

static struct kn_output v4l2_output = {
    .name = "v4l2loopback",
    .format = KN_YUYV,
    .open = v4l2_open,
    .write = v4l2_write,
    .close = v4l2_close,
};

/* ---- PipeWire ---- */

#ifdef HAVE_PIPEWIRE
struct pw_out {
    struct pw_thread_loop *loop;
    struct pw_stream *stream;
    int width, height;
    int ready;
};

static void pw_state_changed(void *data, enum pw_stream_state old, enum pw_stream_state state, const char *error)
{
    (void)old;
    struct pw_out *p = data;
    if (state == PW_STREAM_STATE_STREAMING || state == PW_STREAM_STATE_PAUSED)
        p->ready = 1;
    if (state == PW_STREAM_STATE_ERROR)
        kn_log("PipeWire: %s", error ? error : "error");
    pw_thread_loop_signal(p->loop, false);
}

static const struct pw_stream_events pw_events = {
    PW_VERSION_STREAM_EVENTS,
    .state_changed = pw_state_changed,
};

static int pipewire_open(struct kn_output *out, int w, int h)
{
    pw_init(NULL, NULL);
    struct pw_out *p = calloc(1, sizeof *p);
    p->width = w;
    p->height = h;
    p->loop = pw_thread_loop_new("kybernow-camera", NULL);
    if (!p->loop || pw_thread_loop_start(p->loop) < 0) {
        free(p);
        return -1;
    }
    pw_thread_loop_lock(p->loop);
    p->stream = pw_stream_new_simple(
        pw_thread_loop_get_loop(p->loop), "kybernow-camera",
        pw_properties_new(PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_ROLE, "Camera",
                          PW_KEY_NODE_NAME, "kybernow-camera", PW_KEY_NODE_DESCRIPTION, KN_CAMERA_NAME,
                          NULL),
        &pw_events, p);
    if (!p->stream) {
        pw_thread_loop_unlock(p->loop);
        pw_thread_loop_destroy(p->loop);
        free(p);
        return -1;
    }
    uint8_t buffer[1024];
    struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof buffer);
    const struct spa_pod *params[1];
    params[0] = spa_pod_builder_add_object(
        &b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
        SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
        SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
        SPA_FORMAT_VIDEO_format, SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2),
        SPA_FORMAT_VIDEO_size, SPA_POD_Rectangle(&SPA_RECTANGLE(w, h)),
        SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(KN_CAMERA_FPS, 1)));
    int r = pw_stream_connect(p->stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
                              PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 1);
    pw_thread_loop_unlock(p->loop);
    if (r < 0) {
        kn_log("PipeWire: stream not connected (%d)", r);
        pw_thread_loop_stop(p->loop);
        pw_stream_destroy(p->stream);
        pw_thread_loop_destroy(p->loop);
        free(p);
        return -1;
    }
    kn_log("PipeWire node \"%s\"", KN_CAMERA_NAME);
    out->priv = p;
    return 0;
}

static int pipewire_write(struct kn_output *out, const uint8_t *data, size_t size, int64_t pts)
{
    struct pw_out *p = out->priv;
    pw_thread_loop_lock(p->loop);
    struct pw_buffer *buf = pw_stream_dequeue_buffer(p->stream);
    if (buf) {
        struct spa_data *d = &buf->buffer->datas[0];
        if (d->data && d->maxsize >= size) {
            memcpy(d->data, data, size);
            d->chunk->offset = 0;
            d->chunk->size = (uint32_t)size;
            d->chunk->stride = p->width * 2;
        }
        (void)pts;
        pw_stream_queue_buffer(p->stream, buf);
        pw_stream_trigger_process(p->stream);
    }
    pw_thread_loop_unlock(p->loop);
    return 0;
}

static void pipewire_close(struct kn_output *out)
{
    struct pw_out *p = out->priv;
    if (!p)
        return;
    pw_thread_loop_stop(p->loop);
    pw_stream_destroy(p->stream);
    pw_thread_loop_destroy(p->loop);
    free(p);
    out->priv = NULL;
    pw_deinit();
}

static struct kn_output pipewire_output = {
    .name = "PipeWire",
    .format = KN_YUYV,
    .open = pipewire_open,
    .write = pipewire_write,
    .close = pipewire_close,
};
#endif

struct kn_output *kn_output_open(int width, int height, const char **reason)
{
    if (v4l2_output.open(&v4l2_output, width, height) == 0)
        return &v4l2_output;
#ifdef HAVE_PIPEWIRE
    if (pipewire_output.open(&pipewire_output, width, height) == 0)
        return &pipewire_output;
    *reason = "no v4l2loopback device, PipeWire unreachable";
#else
    *reason = "no v4l2loopback device";
#endif
    return NULL;
}

int kn_output_probe(const char **reason)
{
    char path[64];
    int fd = v4l2_find(NULL, path, sizeof path);
    if (fd >= 0) {
        close(fd);
        return 0;
    }
#ifdef HAVE_PIPEWIRE
    /* La socket de PipeWire de la session. */
    const char *dir = getenv("XDG_RUNTIME_DIR");
    char sock[256];
    snprintf(sock, sizeof sock, "%s/pipewire-0", dir ? dir : "/run/user/0");
    if (access(sock, F_OK) == 0) {
        *reason = "pipewire";
        return 0;
    }
    *reason = "no v4l2loopback device, no PipeWire";
#else
    *reason = "no v4l2loopback device";
#endif
    return 1;
}

int kn_output_install(int install)
{
    (void)install;
    return 0;
}
