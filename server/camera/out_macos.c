/*
 * kybernow-camera, Station Mac (docs/webcam.md, « Station Mac ») : l'image
 * va à l'extension caméra de KyberNow Station (CoreMediaIO,
 * server/macos/camera-extension), par son flux d'entrée (« sink ») : le
 * périphérique « KyberNow Camera » a deux flux, celui que lisent les apps et
 * celui-ci, que l'extension recopie dans le premier.
 *
 * L'API C de CoreMediaIO : le périphérique trouvé par son nom, son flux de
 * direction 1 (entrée), sa file (CMIOStreamCopyBufferQueue), puis chaque
 * image en CMSampleBuffer NV12 (CVPixelBuffer « 420v »).
 */
#include "camera.h"

#include <CoreFoundation/CoreFoundation.h>
#include <CoreMedia/CoreMedia.h>
#include <CoreMediaIO/CMIOHardware.h>
#include <CoreVideo/CoreVideo.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct mac_out {
    CMIODeviceID device;
    CMIOStreamID stream;
    CMSimpleQueueRef queue;
    CVPixelBufferPoolRef pool;
    CMVideoFormatDescriptionRef format;
    int width, height;
    int64_t first_pts;
};

static OSStatus get_property(CMIOObjectID object, CMIOObjectPropertySelector selector, CMIOObjectPropertyScope scope,
                             UInt32 size, void *data, UInt32 *used)
{
    CMIOObjectPropertyAddress a = { selector, scope, kCMIOObjectPropertyElementMain };
    return CMIOObjectGetPropertyData(object, &a, 0, NULL, size, used, data);
}

/* Les objets d'une propriété (périphériques, flux) ; à libérer. */
static CMIOObjectID *object_list(CMIOObjectID object, CMIOObjectPropertySelector selector, CMIOObjectPropertyScope scope,
                                 UInt32 *count)
{
    CMIOObjectPropertyAddress a = { selector, scope, kCMIOObjectPropertyElementMain };
    UInt32 size = 0;
    *count = 0;
    if (CMIOObjectGetPropertyDataSize(object, &a, 0, NULL, &size) != noErr || size == 0)
        return NULL;
    CMIOObjectID *ids = malloc(size);
    UInt32 used = 0;
    if (CMIOObjectGetPropertyData(object, &a, 0, NULL, size, &used, ids) != noErr) {
        free(ids);
        return NULL;
    }
    *count = used / sizeof(CMIOObjectID);
    return ids;
}

static int find_device(CMIODeviceID *device, CMIOStreamID *stream)
{
    /* Les périphériques des extensions sont visibles sans réglage ; on
     * autorise quand même ceux d'écran, comme les apps de visio. */
    CMIOObjectPropertyAddress allow = { kCMIOHardwarePropertyAllowScreenCaptureDevices, kCMIOObjectPropertyScopeGlobal,
                                        kCMIOObjectPropertyElementMain };
    UInt32 yes = 1;
    CMIOObjectSetPropertyData(kCMIOObjectSystemObject, &allow, 0, NULL, sizeof yes, &yes);

    UInt32 count = 0;
    CMIOObjectID *devices = object_list(kCMIOObjectSystemObject, kCMIOHardwarePropertyDevices,
                                        kCMIOObjectPropertyScopeGlobal, &count);
    int found = -1;
    for (UInt32 i = 0; i < count && found < 0; i++) {
        CFStringRef name = NULL;
        UInt32 used = 0;
        if (get_property(devices[i], kCMIOObjectPropertyName, kCMIOObjectPropertyScopeGlobal, sizeof name, &name, &used) != noErr
            || !name)
            continue;
        Boolean ours = CFStringCompare(name, CFSTR(KN_CAMERA_NAME), 0) == kCFCompareEqualTo;
        CFRelease(name);
        if (!ours)
            continue;
        UInt32 n = 0;
        CMIOObjectID *streams = object_list(devices[i], kCMIODevicePropertyStreams, kCMIOObjectPropertyScopeGlobal, &n);
        for (UInt32 s = 0; s < n; s++) {
            UInt32 direction = 0;
            if (get_property(streams[s], kCMIOStreamPropertyDirection, kCMIOObjectPropertyScopeGlobal, sizeof direction,
                             &direction, &used) == noErr
                && direction == 1) {
                *device = devices[i];
                *stream = streams[s];
                found = 0;
                break;
            }
        }
        free(streams);
    }
    free(devices);
    return found;
}

static void queue_altered(CMIOStreamID stream, void *token, void *refcon)
{
    (void)stream;
    (void)token;
    (void)refcon;
}

static int mac_open(struct kn_output *out, int w, int h)
{
    struct mac_out *m = calloc(1, sizeof *m);
    /* L'extension peut mettre un instant à apparaître après son activation. */
    for (int tries = 0; tries < 10; tries++) {
        if (find_device(&m->device, &m->stream) == 0)
            break;
        m->device = 0;
        usleep(200000);
    }
    if (!m->device) {
        free(m);
        return -1;
    }
    if (CMIOStreamCopyBufferQueue(m->stream, queue_altered, NULL, &m->queue) != noErr
        || CMIODeviceStartStream(m->device, m->stream) != noErr) {
        kn_log("camera extension sink not opened");
        if (m->queue)
            CFRelease(m->queue);
        free(m);
        return -1;
    }
    const void *keys[] = { kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey, kCVPixelBufferHeightKey,
                           kCVPixelBufferIOSurfacePropertiesKey };
    int32_t fmt = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
    CFNumberRef nf = CFNumberCreate(NULL, kCFNumberSInt32Type, &fmt);
    CFNumberRef nw = CFNumberCreate(NULL, kCFNumberIntType, &w);
    CFNumberRef nh = CFNumberCreate(NULL, kCFNumberIntType, &h);
    CFDictionaryRef io = CFDictionaryCreate(NULL, NULL, NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    const void *values[] = { nf, nw, nh, io };
    CFDictionaryRef attrs = CFDictionaryCreate(NULL, keys, values, 4, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CVPixelBufferPoolCreate(NULL, NULL, attrs, &m->pool);
    CFRelease(attrs);
    CFRelease(io);
    CFRelease(nf);
    CFRelease(nw);
    CFRelease(nh);
    m->width = w;
    m->height = h;
    m->first_pts = -1;
    kn_log("camera extension of KyberNow Station");
    out->priv = m;
    return m->pool ? 0 : -1;
}

static int mac_write(struct kn_output *out, const uint8_t *data, size_t size, int64_t pts)
{
    struct mac_out *m = out->priv;
    if ((size_t)m->width * m->height * 3 / 2 != size)
        return -1;
    if (CMSimpleQueueGetCount(m->queue) >= CMSimpleQueueGetCapacity(m->queue))
        return 0; /* l'extension ne suit pas : cette image saute */
    CVPixelBufferRef pb = NULL;
    if (CVPixelBufferPoolCreatePixelBuffer(NULL, m->pool, &pb) != kCVReturnSuccess)
        return -1;
    CVPixelBufferLockBaseAddress(pb, 0);
    const uint8_t *src = data;
    for (int plane = 0; plane < 2; plane++) {
        uint8_t *dst = CVPixelBufferGetBaseAddressOfPlane(pb, plane);
        size_t stride = CVPixelBufferGetBytesPerRowOfPlane(pb, plane);
        int rows = plane == 0 ? m->height : m->height / 2;
        for (int y = 0; y < rows; y++)
            memcpy(dst + (size_t)y * stride, src + (size_t)y * m->width, (size_t)m->width);
        src += (size_t)m->width * m->height;
    }
    CVPixelBufferUnlockBaseAddress(pb, 0);
    if (!m->format)
        CMVideoFormatDescriptionCreateForImageBuffer(NULL, pb, &m->format);
    if (m->first_pts < 0)
        m->first_pts = pts;
    CMSampleTimingInfo timing = { CMTimeMake(1, KN_CAMERA_FPS), CMTimeMake(pts - m->first_pts, 1000000), kCMTimeInvalid };
    CMSampleBufferRef sample = NULL;
    OSStatus st = CMSampleBufferCreateReadyWithImageBuffer(NULL, pb, m->format, &timing, &sample);
    CVPixelBufferRelease(pb);
    if (st != noErr || !sample)
        return -1;
    /* La file garde l'image ; l'extension la libère. */
    if (CMSimpleQueueEnqueue(m->queue, sample) != noErr)
        CFRelease(sample);
    return 0;
}

static void mac_close(struct kn_output *out)
{
    struct mac_out *m = out->priv;
    if (!m)
        return;
    CMIODeviceStopStream(m->device, m->stream);
    if (m->queue) {
        CFTypeRef left;
        while ((left = CMSimpleQueueDequeue(m->queue)))
            CFRelease(left);
        CFRelease(m->queue);
    }
    if (m->pool)
        CVPixelBufferPoolRelease(m->pool);
    if (m->format)
        CFRelease(m->format);
    free(m);
    out->priv = NULL;
}

static struct kn_output mac_output = {
    .name = "KyberNow Station camera extension",
    .format = KN_NV12,
    .open = mac_open,
    .write = mac_write,
    .close = mac_close,
};

struct kn_output *kn_output_open(int width, int height, const char **reason)
{
    if (mac_output.open(&mac_output, width, height) == 0)
        return &mac_output;
    *reason = "camera extension not active (System Settings, Login Items & Extensions)";
    return NULL;
}

int kn_output_probe(const char **reason)
{
    CMIODeviceID device;
    CMIOStreamID stream;
    if (find_device(&device, &stream) == 0)
        return 0;
    *reason = "camera extension not active";
    return 1;
}

int kn_output_install(int install)
{
    (void)install;
    return 0;
}
