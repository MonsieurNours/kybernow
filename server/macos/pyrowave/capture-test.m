// Essai de l'encodeur PyroWave de la Station Mac (docs/pyrowave.md, « Essai
// sur le Mac ») : capture ScreenCaptureKit d'un écran, comme txproto (BGRA en
// SDR ; en HDR10, P010 PQ BT.2100 comme le correctif 0010), images
// VideoToolbox données à l'encodeur FFmpeg `libpyrowave_metal` de la Station,
// paquets écrits pour le banc de décodage de l'app Mac
// (apps/macos/tests/pyrowave/bench).
//
//   pyrowave-capture-test -o sortie.pyrowave [-src source.yuv] [-n 240]
//       [-size 3840x2160] [-mbps 600] [-444] [-hdr] [-nv12] [-scale 1920x1080] [-display 0]
//
// L'image la plus récente est encodée à 60 i/s, qu'elle ait changé ou non
// (ScreenCaptureKit n'envoie une image que quand l'écran change) : le temps
// mesuré est celui de la Station, de l'envoi de l'image à l'encodeur au
// paquet prêt (conversion sur le GPU, encodage, mise en paquets).
// -src : la première image encodée, en YCbCr 8 bits (Y, Cb, Cr), pour le
// PSNR du banc (sans -scale).
// Il faut l'autorisation « Enregistrement de l'écran » pour le programme
// qui le lance (Terminal, par exemple).

#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#include <mach/mach_time.h>

#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/opt.h>

@interface Grabber : NSObject <SCStreamOutput>
@property(atomic) CVPixelBufferRef latest;
@property(atomic) int received;
@end

@implementation Grabber
- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sb ofType:(SCStreamOutputType)type {
    if (type != SCStreamOutputTypeScreen)
        return;
    CFArrayRef att = CMSampleBufferGetSampleAttachmentsArray(sb, false);
    if (att && CFArrayGetCount(att)) {
        NSDictionary *d = (__bridge NSDictionary *)CFArrayGetValueAtIndex(att, 0);
        NSNumber *status = d[SCStreamFrameInfoStatus];
        if (status && status.integerValue != SCFrameStatusComplete)
            return;
    }
    CVPixelBufferRef pb = CMSampleBufferGetImageBuffer(sb);
    if (!pb)
        return;
    CVPixelBufferRetain(pb);
    @synchronized(self) {
        CVPixelBufferRef old = _latest;
        _latest = pb;
        if (old)
            CVPixelBufferRelease(old);
    }
    self.received++;
}
- (CVPixelBufferRef)copyLatest {
    @synchronized(self) {
        return _latest ? CVPixelBufferRetain(_latest) : NULL;
    }
}
@end

static double ms_since(uint64_t t0) {
    static mach_timebase_info_data_t tb;
    if (!tb.denom)
        mach_timebase_info(&tb);
    return (double)(mach_absolute_time() - t0) * tb.numer / tb.denom / 1e6;
}

static void buffer_free(void *opaque, uint8_t *data) {
    CVPixelBufferRelease((CVPixelBufferRef)data);
}

// La première image, en YCbCr 8 bits plans séparés, comme l'encodeur la voit.
static void write_source(CVPixelBufferRef pb, int c444, int hdr, const char *path) {
    CVPixelBufferLockBaseAddress(pb, kCVPixelBufferLock_ReadOnly);
    const int w = (int)CVPixelBufferGetWidth(pb), h = (int)CVPixelBufferGetHeight(pb);
    const int cw = c444 ? w : w / 2, ch = c444 ? h : h / 2;
    NSMutableData *out = [NSMutableData dataWithLength:(size_t)w * h + 2 * (size_t)cw * ch];
    uint8_t *y = out.mutableBytes, *cb = y + (size_t)w * h, *cr = cb + (size_t)cw * ch;
    if (CVPixelBufferGetPixelFormatType(pb) == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) {
        const uint8_t *py = CVPixelBufferGetBaseAddressOfPlane(pb, 0);
        const uint8_t *pc = CVPixelBufferGetBaseAddressOfPlane(pb, 1);
        const size_t ry = CVPixelBufferGetBytesPerRowOfPlane(pb, 0), rc = CVPixelBufferGetBytesPerRowOfPlane(pb, 1);
        for (int j = 0; j < h; j++)
            memcpy(y + j * w, py + j * ry, w);
        for (int j = 0; j < ch; j++)
            for (int i = 0; i < cw; i++) {
                cb[j * cw + i] = pc[j * rc + 2 * i];
                cr[j * cw + i] = pc[j * rc + 2 * i + 1];
            }
    } else if (hdr) {
        const uint8_t *py = CVPixelBufferGetBaseAddressOfPlane(pb, 0);
        const uint8_t *pc = CVPixelBufferGetBaseAddressOfPlane(pb, 1);
        const size_t ry = CVPixelBufferGetBytesPerRowOfPlane(pb, 0), rc = CVPixelBufferGetBytesPerRowOfPlane(pb, 1);
        for (int j = 0; j < h; j++)
            for (int i = 0; i < w; i++)
                y[j * w + i] = ((const uint16_t *)(py + j * ry))[i] >> 8;
        for (int j = 0; j < ch; j++)
            for (int i = 0; i < cw; i++) {
                const uint16_t *c = (const uint16_t *)(pc + j * rc) + 2 * i;
                cb[j * cw + i] = c[0] >> 8;
                cr[j * cw + i] = c[1] >> 8;
            }
    } else {
        const uint8_t *base = CVPixelBufferGetBaseAddress(pb);
        const size_t row = CVPixelBufferGetBytesPerRow(pb);
        float *rgb = malloc(sizeof(float) * 3 * w * h);
        for (int j = 0; j < h; j++)
            for (int i = 0; i < w; i++) {
                const uint8_t *p = base + j * row + 4 * i;
                float *q = rgb + 3 * (j * w + i);
                q[0] = p[2] / 255.f; q[1] = p[1] / 255.f; q[2] = p[0] / 255.f;
                y[j * w + i] = (uint8_t)lrintf(255.f * (0.2126f * q[0] + 0.7152f * q[1] + 0.0722f * q[2]));
            }
        const int s = c444 ? 1 : 2;
        for (int j = 0; j < ch; j++)
            for (int i = 0; i < cw; i++) {
                float r = 0, g = 0, b = 0;
                for (int dy = 0; dy < s; dy++)
                    for (int dx = 0; dx < s; dx++) {
                        const float *q = rgb + 3 * ((j * s + dy) * w + i * s + dx);
                        r += q[0]; g += q[1]; b += q[2];
                    }
                r /= s * s; g /= s * s; b /= s * s;
                const float l = 0.2126f * r + 0.7152f * g + 0.0722f * b;
                cb[j * cw + i] = (uint8_t)lrintf(255.f * fminf(fmaxf((b - l) / 1.8556f + 0.5f, 0), 1));
                cr[j * cw + i] = (uint8_t)lrintf(255.f * fminf(fmaxf((r - l) / 1.5748f + 0.5f, 0), 1));
            }
        free(rgb);
    }
    CVPixelBufferUnlockBaseAddress(pb, kCVPixelBufferLock_ReadOnly);
    [out writeToFile:@(path) atomically:NO];
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        const char *outPath = NULL, *srcPath = NULL;
        int nv12 = 0, frames = 240, width = 0, height = 0, c444 = 0, hdr = 0, sw = 0, sh = 0, display = 0;
        double mbps = 0;
        for (int i = 1; i < argc; i++) {
            const char *a = argv[i], *v = i + 1 < argc ? argv[i + 1] : "";
            if (!strcmp(a, "-o")) outPath = v, i++;
            else if (!strcmp(a, "-src")) srcPath = v, i++;
            else if (!strcmp(a, "-n")) frames = atoi(v), i++;
            else if (!strcmp(a, "-size")) sscanf(v, "%dx%d", &width, &height), i++;
            else if (!strcmp(a, "-scale")) sscanf(v, "%dx%d", &sw, &sh), i++;
            else if (!strcmp(a, "-mbps")) mbps = atof(v), i++;
            else if (!strcmp(a, "-display")) display = atoi(v), i++;
            else if (!strcmp(a, "-444")) c444 = 1;
            else if (!strcmp(a, "-hdr")) hdr = 1;
            else if (!strcmp(a, "-nv12")) nv12 = 1;
            else { fprintf(stderr, "option inconnue : %s\n", a); return 2; }
        }
        if (!outPath) {
            fprintf(stderr, "usage : %s -o sortie.pyrowave [-src source.yuv] [-n 240] [-size WxH] [-mbps M] [-444] [-hdr] [-scale WxH] [-display N]\n", argv[0]);
            return 2;
        }

        // Écran et capture, comme txproto.
        __block SCShareableContent *content = nil;
        __block NSError *error = nil;
        dispatch_semaphore_t sem = dispatch_semaphore_create(0);
        [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *c, NSError *e) {
            content = c;
            error = e;
            dispatch_semaphore_signal(sem);
        }];
        dispatch_semaphore_wait(sem, DISPATCH_TIME_FOREVER);
        if (!content || display >= (int)content.displays.count) {
            fprintf(stderr, "Pas d'écran à capturer (%s) : autoriser l'enregistrement de l'écran ?\n",
                    error ? error.localizedDescription.UTF8String : "aucun écran");
            return 3;
        }
        SCDisplay *d = content.displays[display];
        if (!width) {
            CGDisplayModeRef mode = CGDisplayCopyDisplayMode(d.displayID);
            width = (int)CGDisplayModeGetPixelWidth(mode);
            height = (int)CGDisplayModeGetPixelHeight(mode);
            CGDisplayModeRelease(mode);
        }
        SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:d excludingWindows:@[]];
        SCStreamConfiguration *cfg = [SCStreamConfiguration new];
        cfg.width = width;
        cfg.height = height;
        cfg.scalesToFit = YES;
        cfg.showsCursor = NO;
        cfg.queueDepth = 3;
        cfg.minimumFrameInterval = CMTimeMake(1, 60);
        if (hdr) {
            cfg.captureDynamicRange = SCCaptureDynamicRangeHDRCanonicalDisplay;
            cfg.pixelFormat = kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange;
            cfg.colorSpaceName = kCGColorSpaceITUR_2100_PQ;
        } else if (nv12) {
            // Comme le correctif Mac 0019 : NV12 BT.709 pleine plage, sans conversion.
            cfg.pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarFullRange;
            cfg.colorMatrix = kCGDisplayStreamYCbCrMatrix_ITU_R_709_2;
        } else {
            cfg.pixelFormat = kCVPixelFormatType_32BGRA;
        }
        Grabber *grabber = [Grabber new];
        SCStream *stream = [[SCStream alloc] initWithFilter:filter configuration:cfg delegate:nil];
        dispatch_queue_t q = dispatch_queue_create("capture", DISPATCH_QUEUE_SERIAL);
        if (![stream addStreamOutput:grabber type:SCStreamOutputTypeScreen sampleHandlerQueue:q error:&error]) {
            fprintf(stderr, "Capture refusée : %s\n", error.localizedDescription.UTF8String);
            return 3;
        }
        [stream startCaptureWithCompletionHandler:^(NSError *e) {
            error = e;
            dispatch_semaphore_signal(sem);
        }];
        dispatch_semaphore_wait(sem, DISPATCH_TIME_FOREVER);
        if (error) {
            fprintf(stderr, "Capture refusée : %s\n", error.localizedDescription.UTF8String);
            return 3;
        }
        for (int i = 0; i < 300 && !grabber.received; i++)
            usleep(10000);
        CVPixelBufferRef first = [grabber copyLatest];
        if (!first) {
            fprintf(stderr, "Aucune image en 3 s\n");
            return 3;
        }
        const OSType fourcc = CVPixelBufferGetPixelFormatType(first);
        printf("Écran %u : capture %dx%d %c%c%c%c (%s)\n", d.displayID, width, height,
               fourcc >> 24, fourcc >> 16, fourcc >> 8, fourcc, hdr ? "HDR10, PQ BT.2100" : "SDR");

        // L'encodeur de la Station, sur le contexte d'images de la capture.
        AVBufferRef *device = NULL;
        if (av_hwdevice_ctx_create(&device, AV_HWDEVICE_TYPE_VIDEOTOOLBOX, NULL, NULL, 0) < 0)
            return 4;
        AVBufferRef *framesRef = av_hwframe_ctx_alloc(device);
        AVHWFramesContext *fc = (AVHWFramesContext *)framesRef->data;
        fc->format = AV_PIX_FMT_VIDEOTOOLBOX;
        fc->sw_format = hdr ? AV_PIX_FMT_P010 : nv12 ? AV_PIX_FMT_NV12 : AV_PIX_FMT_BGRA;
        fc->width = width;
        fc->height = height;
        if (av_hwframe_ctx_init(framesRef) < 0)
            return 4;
        const AVCodec *codec = avcodec_find_encoder_by_name("libpyrowave_metal");
        if (!codec) {
            fprintf(stderr, "Encodeur libpyrowave_metal absent de FFmpeg\n");
            return 4;
        }
        AVCodecContext *ctx = avcodec_alloc_context3(codec);
        ctx->width = width;
        ctx->height = height;
        ctx->pix_fmt = AV_PIX_FMT_VIDEOTOOLBOX;
        ctx->time_base = (AVRational){1, 1000000};
        ctx->framerate = (AVRational){60, 1};
        ctx->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
        ctx->hw_frames_ctx = av_buffer_ref(framesRef);
        const int ow = sw ? sw : width, oh = sh ? sh : height;
        ctx->bit_rate = (int64_t)((mbps > 0 ? mbps : ow * (double)oh * 60 * 1.2 / 1e6) * 1e6);
        av_opt_set_int(ctx->priv_data, "chroma444", c444, 0);
        if (sw) {
            av_opt_set_int(ctx->priv_data, "out_width", sw, 0);
            av_opt_set_int(ctx->priv_data, "out_height", sh, 0);
        }
        uint64_t t0 = mach_absolute_time();
        int ret = avcodec_open2(ctx, codec, NULL);
        if (ret < 0) {
            fprintf(stderr, "Encodeur refusé : %s\n", av_err2str(ret));
            return 4;
        }
        printf("Encodeur prêt en %.0f ms, %.0f Mbit/s (%.2f bit/point)\n", ms_since(t0), ctx->bit_rate / 1e6,
               ctx->bit_rate / (ow * (double)oh * 60));
        printf("En-tête :");
        for (int i = 0; i < ctx->extradata_size; i++)
            printf(" %02x", ctx->extradata[i]);
        printf("\n");

        FILE *out = fopen(outPath, "wb");
        fwrite(ctx->extradata, 1, ctx->extradata_size, out);
        double *times = calloc(frames, sizeof(double));
        double bytes = 0;
        AVPacket *pkt = av_packet_alloc();
        uint64_t next = mach_absolute_time();
        mach_timebase_info_data_t tb;
        mach_timebase_info(&tb);
        const uint64_t period = (uint64_t)(1e9 / 60 * tb.denom / tb.numer);
        int done = 0;
        for (int i = 0; i < frames; i++) {
            CVPixelBufferRef pb = i ? [grabber copyLatest] : first;
            if (i == 0 && srcPath && !sw)
                write_source(pb, c444 && !hdr, hdr, srcPath);
            AVFrame *f = av_frame_alloc();
            f->format = AV_PIX_FMT_VIDEOTOOLBOX;
            f->width = width;
            f->height = height;
            f->data[3] = (uint8_t *)pb;
            f->buf[0] = av_buffer_create((uint8_t *)pb, sizeof(pb), buffer_free, NULL, AV_BUFFER_FLAG_READONLY);
            f->hw_frames_ctx = av_buffer_ref(framesRef);
            f->pts = (int64_t)i * 16667;
            const uint64_t t = mach_absolute_time();
            ret = avcodec_send_frame(ctx, f);
            if (ret >= 0)
                ret = avcodec_receive_packet(ctx, pkt);
            times[i] = ms_since(t);
            av_frame_free(&f);
            if (ret < 0) {
                fprintf(stderr, "Image %d : %s\n", i, av_err2str(ret));
                return 5;
            }
            uint32_t n = (uint32_t)pkt->size;
            fwrite(&n, 4, 1, out);
            fwrite(pkt->data, 1, pkt->size, out);
            bytes += pkt->size;
            av_packet_unref(pkt);
            done++;
            next += period;
            if (next > mach_absolute_time())
                mach_wait_until(next);
        }
        fclose(out);
        [stream stopCaptureWithCompletionHandler:^(NSError *e) { dispatch_semaphore_signal(sem); }];
        dispatch_semaphore_wait(sem, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC));

        // Statistiques, sans les 10 premières (pipelines, mémoire).
        const int skip = done > 20 ? 10 : 0, n = done - skip;
        double *s = times + skip;
        for (int i = 1; i < n; i++)
            for (int j = i; j > 0 && s[j] < s[j - 1]; j--) {
                double tmp = s[j]; s[j] = s[j - 1]; s[j - 1] = tmp;
            }
        double sum = 0;
        for (int i = 0; i < n; i++)
            sum += s[i];
        printf("%d images encodées (%d reçues de la capture) : moyenne %.2f ms, médiane %.2f, 95e centile %.2f, max %.2f ; %.0f octets par image (%.0f Mbit/s à 60 i/s)\n",
               done, grabber.received, sum / n, s[n / 2], s[n * 95 / 100], s[n - 1], bytes / done,
               bytes / done * 8 * 60 / 1e6);
        avcodec_free_context(&ctx);
        av_buffer_unref(&framesRef);
        av_buffer_unref(&device);
        free(times);
        return 0;
    }
}
