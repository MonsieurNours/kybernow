// KyberNow Station pour Mac : encodeur PyroWave sur la puce Apple.
// Voir kn_pyrowave_metal.h et docs/pyrowave.md (« La Station Mac »).
// Compilé avec ARC, avec le port Metal de PyroWave (build.sh).

#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <IOSurface/IOSurfaceRef.h>
#import <Metal/Metal.h>
#include <mach/mach_time.h>

#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <vector>

#include "kn_pyrowave_metal.h"
#include "pyrowave_metal.h"

// Conversion et réduction sur le GPU. La texture d'entrée est échantillonnée
// en linéaire au centre de chaque point de sortie : à taille égale, c'est la
// valeur exacte ; réduite de moitié (4K → 1080p), la moyenne de 2 × 2 points.
static NSString *const kShaders = @R"MSL(
#include <metal_stdlib>
using namespace metal;

constexpr sampler lin(coord::normalized, filter::linear, address::clamp_to_edge);
// BT.709, plage pleine (contrat KyberNow, SDR).
constant float3 kY = float3(0.2126, 0.7152, 0.0722);
inline float2 cbcr(float3 rgb, float y) {
    return float2((rgb.b - y) / 1.8556 + 0.5, (rgb.r - y) / 1.5748 + 0.5);
}

kernel void rgb_to_444(texture2d<float, access::sample> src [[texture(0)]],
                       texture2d<float, access::write> oy [[texture(1)]],
                       texture2d<float, access::write> ocb [[texture(2)]],
                       texture2d<float, access::write> ocr [[texture(3)]],
                       uint2 gid [[thread_position_in_grid]]) {
    uint w = oy.get_width(), h = oy.get_height();
    if (gid.x >= w || gid.y >= h) return;
    float3 rgb = src.sample(lin, (float2(gid) + 0.5) / float2(w, h)).rgb;
    float y = dot(rgb, kY);
    float2 c = cbcr(rgb, y);
    oy.write(float4(y), gid);
    ocb.write(float4(c.x), gid);
    ocr.write(float4(c.y), gid);
}

// Un fil par point de chroma : ses 2 × 2 points de luminance, la chroma de
// leur moyenne (au centre du bloc).
kernel void rgb_to_420(texture2d<float, access::sample> src [[texture(0)]],
                       texture2d<float, access::write> oy [[texture(1)]],
                       texture2d<float, access::write> ocb [[texture(2)]],
                       texture2d<float, access::write> ocr [[texture(3)]],
                       uint2 gid [[thread_position_in_grid]]) {
    if (gid.x >= ocb.get_width() || gid.y >= ocb.get_height()) return;
    float2 inv = 1.0 / float2(oy.get_width(), oy.get_height());
    float3 sum = 0.0;
    for (uint dy = 0; dy < 2; dy++) {
        for (uint dx = 0; dx < 2; dx++) {
            uint2 p = gid * 2 + uint2(dx, dy);
            float3 rgb = src.sample(lin, (float2(p) + 0.5) * inv).rgb;
            oy.write(float4(dot(rgb, kY)), p);
            sum += rgb;
        }
    }
    sum *= 0.25;
    float2 c = cbcr(sum, dot(sum, kY));
    ocb.write(float4(c.x), gid);
    ocr.write(float4(c.y), gid);
}

// HDR10 : P010 de la capture réduit en P010, sans conversion de couleur.
kernel void biplanar_scale(texture2d<float, access::sample> sy [[texture(0)]],
                           texture2d<float, access::sample> sc [[texture(1)]],
                           texture2d<float, access::write> oy [[texture(2)]],
                           texture2d<float, access::write> oc [[texture(3)]],
                           uint2 gid [[thread_position_in_grid]]) {
    uint w = oy.get_width(), h = oy.get_height();
    if (gid.x < w && gid.y < h)
        oy.write(float4(sy.sample(lin, (float2(gid) + 0.5) / float2(w, h)).r), gid);
    uint cw = oc.get_width(), ch = oc.get_height();
    if (gid.x < cw && gid.y < ch)
        oc.write(float4(sc.sample(lin, (float2(gid) + 0.5) / float2(cw, ch)).rg, 0.0, 0.0), gid);
}
)MSL";

struct kn_pw {
    id<MTLDevice> device;
    id<MTLCommandQueue> queue;
    id<MTLComputePipelineState> to444, to420, scale;
    pyrowave_device pwdev = nullptr;
    pyrowave_encoder enc = nullptr;
    int width = 0, height = 0, chroma444 = 0, hdr = 0;

    // SDR : trois plans R8 (Y, Cb, Cr) sur des IOSurface, que PyroWave lit.
    IOSurfaceRef planes[3] = {};
    id<MTLTexture> plane_tex[3];
    // HDR réduit : un P010 à la définition encodée.
    CVPixelBufferRef p010 = nullptr;
    id<MTLTexture> p010_tex[2];

    // L'image mise bout à bout (kn_pw_frame).
    std::vector<uint8_t> frame;
    double convert_ms = 0, encode_ms = 0;
    uint64_t encode_start = 0;
    char err[256] = {};
    char log[192] = {}; // dernier message de PyroWave
};

static void set_err(kn_pw *pw, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(pw->err, sizeof(pw->err), fmt, ap);
    va_end(ap);
}

static double ticks_ms(uint64_t ticks) {
    static mach_timebase_info_data_t tb;
    if (!tb.denom)
        mach_timebase_info(&tb);
    return (double)ticks * tb.numer / tb.denom / 1e6;
}

static void pw_log(void *userdata, const char *msg) {
    kn_pw *pw = (kn_pw *)userdata;
    if (pw)
        snprintf(pw->log, sizeof(pw->log), "%s", msg);
}

extern "C" int kn_pw_supported(char *name, size_t name_size) {
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (name && name_size)
            snprintf(name, name_size, "%s", device ? device.name.UTF8String : "aucune");
        return device && pyrowave_device_is_supported((__bridge void *)device) ? 1 : 0;
    }
}

static IOSurfaceRef make_r8_surface(int w, int h) {
    size_t row = IOSurfaceAlignProperty(kIOSurfaceBytesPerRow, (size_t)w);
    NSDictionary *props = @{
        (id)kIOSurfaceWidth: @(w),
        (id)kIOSurfaceHeight: @(h),
        (id)kIOSurfaceBytesPerElement: @1,
        (id)kIOSurfaceBytesPerRow: @(row),
        (id)kIOSurfaceAllocSize: @(row * (size_t)h),
        (id)kIOSurfacePixelFormat: @(kCVPixelFormatType_OneComponent8),
    };
    return IOSurfaceCreate((__bridge CFDictionaryRef)props);
}

static id<MTLTexture> surface_texture(id<MTLDevice> device, IOSurfaceRef surface, size_t plane,
                                      MTLPixelFormat format, bool write) {
    MTLTextureDescriptor *d = [MTLTextureDescriptor
        texture2DDescriptorWithPixelFormat:format
                                     width:IOSurfaceGetWidthOfPlane(surface, plane) ?: IOSurfaceGetWidth(surface)
                                    height:IOSurfaceGetHeightOfPlane(surface, plane) ?: IOSurfaceGetHeight(surface)
                                 mipmapped:NO];
    d.usage = MTLTextureUsageShaderRead | (write ? MTLTextureUsageShaderWrite : 0);
    d.storageMode = MTLStorageModeShared;
    return [device newTextureWithDescriptor:d iosurface:surface plane:plane];
}

extern "C" void kn_pw_destroy(kn_pw *pw) {
    if (!pw)
        return;
    @autoreleasepool {
        if (pw->enc)
            pyrowave_encoder_destroy(pw->enc);
        if (pw->pwdev)
            pyrowave_device_destroy(pw->pwdev);
        for (int i = 0; i < 3; i++) {
            pw->plane_tex[i] = nil;
            if (pw->planes[i])
                CFRelease(pw->planes[i]);
        }
        pw->p010_tex[0] = pw->p010_tex[1] = nil;
        if (pw->p010)
            CVPixelBufferRelease(pw->p010);
        delete pw;
    }
}

extern "C" int kn_pw_create(const kn_pw_config *config, kn_pw **out, char *err, size_t err_size) {
    @autoreleasepool {
        auto fail = [&](kn_pw *pw, const char *why) {
            if (err && err_size)
                snprintf(err, err_size, "%s", pw && pw->err[0] ? pw->err : why);
            kn_pw_destroy(pw);
            return -1;
        };
        *out = nullptr;
        kn_pw *pw = new kn_pw;
        pw->width = config->width;
        pw->height = config->height;
        pw->hdr = config->hdr ? 1 : 0;
        // 4:4:4 en SDR seulement (le P010 de la capture est en 4:2:0).
        pw->chroma444 = config->chroma444 && !pw->hdr ? 1 : 0;
        if (pw->width < 16 || pw->height < 16 || pw->width > 16384 || pw->height > 16384 ||
            (!pw->chroma444 && ((pw->width | pw->height) & 1))) {
            set_err(pw, "définition %dx%d refusée (4:2:0 : paire, 16 à 16384)", pw->width, pw->height);
            return fail(pw, "");
        }

        pw->device = MTLCreateSystemDefaultDevice();
        if (!pw->device || !pyrowave_device_is_supported((__bridge void *)pw->device)) {
            set_err(pw, "puce %s : PyroWave demande la famille Apple7 ou plus",
                    pw->device ? pw->device.name.UTF8String : "absente");
            return fail(pw, "");
        }
        pw->queue = [pw->device newCommandQueue];
        pw->queue.label = @"KyberNow PyroWave conversion";

        NSError *error = nil;
        MTLCompileOptions *opts = [MTLCompileOptions new];
        opts.mathMode = MTLMathModeSafe;
        id<MTLLibrary> lib = [pw->device newLibraryWithSource:kShaders options:opts error:&error];
        if (!lib) {
            set_err(pw, "shaders de conversion : %s", error.localizedDescription.UTF8String);
            return fail(pw, "");
        }
        for (auto [name, state] : {std::pair{@"rgb_to_444", &pw->to444}, {@"rgb_to_420", &pw->to420},
                                   {@"biplanar_scale", &pw->scale}}) {
            *state = [pw->device newComputePipelineStateWithFunction:[lib newFunctionWithName:name] error:&error];
            if (!*state) {
                set_err(pw, "pipeline %s : %s", name.UTF8String, error.localizedDescription.UTF8String);
                return fail(pw, "");
            }
        }

        pyrowave_device_create_info dinfo = {};
        dinfo.mtl_device = (__bridge void *)pw->device;
        dinfo.message_callback = pw_log;
        dinfo.message_userdata = pw;
        pyrowave_result r = pyrowave_device_create(&dinfo, &pw->pwdev);
        if (r != PYROWAVE_SUCCESS) {
            pw->pwdev = nullptr;
            set_err(pw, "device PyroWave : %s (%s)", pyrowave_result_to_string(r), pw->log);
            return fail(pw, "");
        }
        pyrowave_encoder_create_info einfo = {};
        einfo.device = pw->pwdev;
        einfo.width = pw->width;
        einfo.height = pw->height;
        einfo.chroma = pw->chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420;
        r = pyrowave_encoder_create(&einfo, &pw->enc);
        if (r != PYROWAVE_SUCCESS) {
            pw->enc = nullptr;
            set_err(pw, "encodeur PyroWave %dx%d : %s (%s)", pw->width, pw->height, pyrowave_result_to_string(r), pw->log);
            return fail(pw, "");
        }

        if (!pw->hdr) {
            const int cw = pw->chroma444 ? pw->width : pw->width / 2;
            const int ch = pw->chroma444 ? pw->height : pw->height / 2;
            for (int i = 0; i < 3; i++) {
                pw->planes[i] = make_r8_surface(i ? cw : pw->width, i ? ch : pw->height);
                if (!pw->planes[i])
                    return fail(pw, "IOSurface des plans refusée");
                pw->plane_tex[i] = surface_texture(pw->device, pw->planes[i], 0, MTLPixelFormatR8Unorm, true);
                if (!pw->plane_tex[i])
                    return fail(pw, "texture des plans refusée");
            }
        }
        *out = pw;
        return 0;
    }
}

static void dispatch(id<MTLComputeCommandEncoder> enc, id<MTLComputePipelineState> state, int w, int h) {
    [enc setComputePipelineState:state];
    const NSUInteger tw = state.threadExecutionWidth;
    const NSUInteger th = MAX(1u, state.maxTotalThreadsPerThreadgroup / tw / 2);
    [enc dispatchThreads:MTLSizeMake((NSUInteger)w, (NSUInteger)h, 1)
        threadsPerThreadgroup:MTLSizeMake(tw, th, 1)];
}

static bool run(kn_pw *pw, void (^encode)(id<MTLComputeCommandEncoder>)) {
    id<MTLCommandBuffer> cmd = [pw->queue commandBuffer];
    id<MTLComputeCommandEncoder> enc = [cmd computeCommandEncoder];
    encode(enc);
    [enc endEncoding];
    [cmd commit];
    // L'encodeur de PyroWave a sa propre file : la conversion doit être
    // finie avant qu'il lise les plans (pas d'ordre entre deux files).
    // STMAC-21 : un MTLSharedEvent attendu par la file de PyroWave éviterait
    // cette attente, mais `pyrowave_encoder_encode_gpu` ne prend ni tampon de
    // commandes ni événement (le décodeur, lui, prend un tampon) : il faut
    // d'abord cette entrée dans PyroWave.
    [cmd waitUntilCompleted];
    if (cmd.status == MTLCommandBufferStatusError) {
        set_err(pw, "conversion : %s", cmd.error.localizedDescription.UTF8String);
        return false;
    }
    return true;
}

extern "C" int kn_pw_encode(kn_pw *pw, void *pixbuf, size_t budget) {
    @autoreleasepool {
        CVPixelBufferRef pb = (CVPixelBufferRef)pixbuf;
        IOSurfaceRef surface = pb ? CVPixelBufferGetIOSurface(pb) : nullptr;
        if (!surface) {
            set_err(pw, "image sans IOSurface");
            return -1;
        }
        const OSType fmt = CVPixelBufferGetPixelFormatType(pb);
        const int w = (int)CVPixelBufferGetWidth(pb), h = (int)CVPixelBufferGetHeight(pb);
        const uint64_t t0 = mach_absolute_time();
        pyrowave_gpu_input input = {};

        if (!pw->hdr && fmt == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) {
            // NV12 BT.709 pleine plage de ScreenCaptureKit (4:2:0 SDR à la
            // définition encodée, correctif Mac 0019) : aucune conversion.
            if (pw->chroma444 || w != pw->width || h != pw->height) {
                set_err(pw, "NV12 %dx%d : seulement en 4:2:0 à la définition encodée (%dx%d)", w, h, pw->width, pw->height);
                return -1;
            }
            input.planes[0] = surface;
        } else if (!pw->hdr) {
            if (fmt != kCVPixelFormatType_32BGRA) {
                set_err(pw, "format d'entrée %c%c%c%c en SDR (BGRA attendu)", fmt >> 24, fmt >> 16, fmt >> 8, fmt);
                return -1;
            }
            id<MTLTexture> src = surface_texture(pw->device, surface, 0, MTLPixelFormatBGRA8Unorm, false);
            if (!src) {
                set_err(pw, "texture de l'image refusée");
                return -1;
            }
            id<MTLComputePipelineState> state = pw->chroma444 ? pw->to444 : pw->to420;
            const int gw = pw->chroma444 ? pw->width : pw->width / 2;
            const int gh = pw->chroma444 ? pw->height : pw->height / 2;
            if (!run(pw, ^(id<MTLComputeCommandEncoder> enc) {
                    [enc setTexture:src atIndex:0];
                    for (int i = 0; i < 3; i++)
                        [enc setTexture:pw->plane_tex[i] atIndex:1 + i];
                    dispatch(enc, state, gw, gh);
                }))
                return -1;
            for (int i = 0; i < 3; i++)
                input.planes[i] = pw->planes[i];
        } else {
            // P010 plage réduite seulement : c'est ce que le paquet de
            // configuration annonce (pyrowave_common.h de FFmpeg), et la capture ne
            // demande pas autre chose ; la pleine plage serait mal lue (STMAC-22).
            if (fmt != kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange) {
                set_err(pw, "format d'entrée %c%c%c%c en HDR (P010 plage réduite attendu)", fmt >> 24, fmt >> 16, fmt >> 8, fmt);
                return -1;
            }
            if (w == pw->width && h == pw->height) {
                // Le P010 de la capture tel quel : aucune copie.
                input.planes[0] = surface;
            } else {
                if (!pw->p010) {
                    NSDictionary *attrs = @{
                        (id)kCVPixelBufferIOSurfacePropertiesKey: @{},
                        (id)kCVPixelBufferMetalCompatibilityKey: @YES,
                    };
                    if (CVPixelBufferCreate(nullptr, pw->width, pw->height, kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
                                            (__bridge CFDictionaryRef)attrs, &pw->p010) != kCVReturnSuccess) {
                        set_err(pw, "P010 réduit refusé");
                        return -1;
                    }
                    IOSurfaceRef out = CVPixelBufferGetIOSurface(pw->p010);
                    pw->p010_tex[0] = surface_texture(pw->device, out, 0, MTLPixelFormatR16Unorm, true);
                    pw->p010_tex[1] = surface_texture(pw->device, out, 1, MTLPixelFormatRG16Unorm, true);
                }
                id<MTLTexture> sy = surface_texture(pw->device, surface, 0, MTLPixelFormatR16Unorm, false);
                id<MTLTexture> sc = surface_texture(pw->device, surface, 1, MTLPixelFormatRG16Unorm, false);
                if (!sy || !sc || !pw->p010_tex[0] || !pw->p010_tex[1]) {
                    set_err(pw, "textures P010 refusées");
                    return -1;
                }
                if (!run(pw, ^(id<MTLComputeCommandEncoder> enc) {
                        [enc setTexture:sy atIndex:0];
                        [enc setTexture:sc atIndex:1];
                        [enc setTexture:pw->p010_tex[0] atIndex:2];
                        [enc setTexture:pw->p010_tex[1] atIndex:3];
                        dispatch(enc, pw->scale, pw->width, pw->height);
                    }))
                    return -1;
                input.planes[0] = CVPixelBufferGetIOSurface(pw->p010);
            }
        }
        const uint64_t t1 = mach_absolute_time();
        pw->convert_ms = ticks_ms(t1 - t0);
        pw->encode_start = t1;

        pyrowave_rate_control rate = {budget & ~(size_t)3};
        pyrowave_result r = pyrowave_encoder_encode_gpu(pw->enc, &input, &rate);
        if (r != PYROWAVE_SUCCESS) {
            set_err(pw, "encodage : %s (%s)", pyrowave_result_to_string(r), pw->log);
            return -1;
        }
        return 0;
    }
}

// L'en-tête de séquence de PyroWave (8 octets aujourd'hui) tient dans cette
// marge, comme dans pyrowave_common.h de FFmpeg.
static const size_t kSequenceHeaderMax = 256;

extern "C" int kn_pw_frame(kn_pw *pw, const uint8_t **data, size_t *size) {
    // L'image est l'en-tête de séquence puis les blocs codés, tous dans le
    // flux encodé : elle n'en dépasse jamais la taille plus l'en-tête
    // (l'ancienne place, budget + n × 64 + 4096, n'était pas une borne : un
    // bloc plus gros que la frontière en fait sortir l'image, et le port
    // Metal la tronque alors sans le dire).
    const void *bits = nullptr, *meta = nullptr;
    size_t bits_size = 0, meta_size = 0;
    pyrowave_result r = pyrowave_encoder_get_mapped_raw_bitstream(pw->enc, &bits, &bits_size, &meta, &meta_size);
    if (r != PYROWAVE_SUCCESS) {
        set_err(pw, "taille du flux encodé : %s", pyrowave_result_to_string(r));
        return -1;
    }
    const size_t capacity = bits_size + kSequenceHeaderMax;
    if (pw->frame.size() < capacity)
        pw->frame.resize(capacity);
    // Un seul paquet : une frontière qu'aucune image n'atteint. PyroWave met
    // les blocs bout à bout quelle que soit la frontière : c'est l'image du
    // contrat, sans compter les paquets ni les déplacer.
    pyrowave_packet whole = {};
    size_t count = 1;
    r = pyrowave_encoder_packetize(pw->enc, &whole, SIZE_MAX, &count, pw->frame.data(), capacity);
    if (r != PYROWAVE_SUCCESS || count != 1 || whole.offset || !whole.size || whole.size > capacity) {
        set_err(pw, "mise en paquets : %s (%zu paquets, %zu octets)", pyrowave_result_to_string(r), count, whole.size);
        return -1;
    }
    *data = pw->frame.data();
    *size = whole.size;
    pw->encode_ms = ticks_ms(mach_absolute_time() - pw->encode_start);
    return 0;
}

extern "C" const char *kn_pw_error(const kn_pw *pw) {
    return pw && pw->err[0] ? pw->err : "erreur inconnue";
}

extern "C" void kn_pw_last_timing(const kn_pw *pw, double *convert_ms, double *encode_ms) {
    if (convert_ms)
        *convert_ms = pw->convert_ms;
    if (encode_ms)
        *encode_ms = pw->encode_ms;
}

// Vérification d'Admin : une mire BGRA 256 × 256 encodée à ~3 bits par point,
// décodée par le décodeur du même port, luminance comparée à celle attendue.
extern "C" int kn_pw_selftest(char *msg, size_t msg_size) {
    @autoreleasepool {
        auto say = [&](int code, const char *fmt, auto... args) {
            if (msg && msg_size)
                snprintf(msg, msg_size, fmt, args...);
            return code;
        };
        char name[128];
        if (!kn_pw_supported(name, sizeof(name)))
            return say(2, "puce %s : PyroWave demande la famille Apple7 ou plus", name);

        const int W = 256, H = 256;
        CVPixelBufferRef pb = nullptr;
        NSDictionary *attrs = @{(id)kCVPixelBufferIOSurfacePropertiesKey: @{},
                                (id)kCVPixelBufferMetalCompatibilityKey: @YES};
        if (CVPixelBufferCreate(nullptr, W, H, kCVPixelFormatType_32BGRA, (__bridge CFDictionaryRef)attrs, &pb))
            return say(4, "%s", "image d'essai refusée");
        std::vector<double> expected(W * H);
        CVPixelBufferLockBaseAddress(pb, 0);
        uint8_t *base = (uint8_t *)CVPixelBufferGetBaseAddress(pb);
        const size_t row = CVPixelBufferGetBytesPerRow(pb);
        for (int y = 0; y < H; y++) {
            for (int x = 0; x < W; x++) {
                const int r = x, g = y, b = ((x / 32 + y / 32) & 1) ? 220 : 30;
                uint8_t *p = base + y * row + x * 4;
                p[0] = (uint8_t)b; p[1] = (uint8_t)g; p[2] = (uint8_t)r; p[3] = 255;
                expected[y * W + x] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            }
        }
        CVPixelBufferUnlockBaseAddress(pb, 0);

        kn_pw_config config = {W, H, 0, 0};
        kn_pw *pw = nullptr;
        char err[256] = {};
        if (kn_pw_create(&config, &pw, err, sizeof(err)) < 0) {
            CVPixelBufferRelease(pb);
            return say(4, "%s", err);
        }
        const uint8_t *frame = nullptr;
        size_t size = 0;
        std::vector<uint8_t> bits;
        int rc = kn_pw_encode(pw, pb, W * H * 3 / 8);
        if (rc == 0)
            rc = kn_pw_frame(pw, &frame, &size);
        if (rc == 0)
            bits.assign(frame, frame + size);
        CVPixelBufferRelease(pb);
        if (rc < 0) {
            int code = say(4, "%s", kn_pw_error(pw));
            kn_pw_destroy(pw);
            return code;
        }

        pyrowave_decoder dec = nullptr;
        pyrowave_decoder_create_info dinfo = {pw->pwdev, W, H, PYROWAVE_CHROMA_SUBSAMPLING_420};
        if (pyrowave_decoder_create(&dinfo, &dec) != PYROWAVE_SUCCESS) {
            kn_pw_destroy(pw);
            return say(5, "%s", "décodeur refusé");
        }
        pyrowave_decoder_push_packet(dec, bits.data(), size);
        const bool ready = pyrowave_decoder_decode_is_ready(dec, false);
        id<MTLTexture> planes[3];
        for (int i = 0; i < 3; i++) {
            MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatR8Unorm
                                                                                         width:i ? W / 2 : W
                                                                                        height:i ? H / 2 : H
                                                                                     mipmapped:NO];
            d.usage = MTLTextureUsageShaderWrite | MTLTextureUsageShaderRead;
            d.storageMode = MTLStorageModeShared;
            planes[i] = [pw->device newTextureWithDescriptor:d];
        }
        pyrowave_gpu_buffers out = {{(__bridge void *)planes[0], (__bridge void *)planes[1], (__bridge void *)planes[2]}};
        id<MTLCommandBuffer> cmd = [pw->queue commandBuffer];
        pyrowave_result r = pyrowave_decoder_decode_gpu_buffer(dec, (__bridge void *)cmd, &out);
        [cmd commit];
        [cmd waitUntilCompleted];
        pyrowave_decoder_destroy(dec);
        kn_pw_destroy(pw);
        if (r != PYROWAVE_SUCCESS || !ready)
            return say(5, "décodage : %s", ready ? pyrowave_result_to_string(r) : "image incomplète");

        std::vector<uint8_t> luma(W * H);
        [planes[0] getBytes:luma.data() bytesPerRow:W fromRegion:MTLRegionMake2D(0, 0, W, H) mipmapLevel:0];
        double se = 0;
        for (int i = 0; i < W * H; i++)
            se += (luma[i] - expected[i]) * (luma[i] - expected[i]);
        const double psnr = se ? 10 * log10(255.0 * 255.0 / (se / (W * H))) : 99;
        if (psnr < 30)
            return say(5, "aller-retour faux : PSNR %.1f dB", psnr);
        return say(0, "PyroWave sur %s : %zu octets, PSNR %.1f dB", name, size, psnr);
    }
}
