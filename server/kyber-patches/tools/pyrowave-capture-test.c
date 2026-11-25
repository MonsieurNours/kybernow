/*
 * KyberNow : essai de l'encodeur PyroWave de la Station Windows
 * (docs/pyrowave.md, « Essai sur le PC »).
 *
 * Capture l'écran principal par la duplication DXGI dans des images D3D11
 * (BGRA, comme txproto et libavgrab), les encode avec l'encodeur FFmpeg
 * `libpyrowave` de la Station (texture partagée avec Vulkan, fence partagée,
 * sans copie par le processeur), puis décode la dernière image avec le
 * décodeur PyroWave (sur la carte, relue en mémoire) pour vérifier le flux.
 *
 * Usage : pyrowave-capture-test [images] [débit en Mbit/s] [largeur hauteur]
 *         (120 images, 200 Mbit/s, taille de l'écran par défaut ; une taille
 *         plus petite essaie l'image réduite d'un invité)
 * Écrit pyrowave-test.pwv (les paquets, chacun précédé de sa taille en u32 LE)
 * et pyrowave-test.y4m (la dernière image décodée, lisible par VLC ou ffplay).
 * Code de sortie : 0 si tout va bien.
 */

#define COBJMACROS
#include <initguid.h>
#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_d3d11va.h>
#include <libavutil/opt.h>

#include <vulkan/vulkan.h>
#include <pyrowave.h>

static double now_ms(void)
{
    LARGE_INTEGER f, c;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart * 1000.0 / (double)f.QuadPart;
}

static int fail(const char *what, long code)
{
    fprintf(stderr, "ÉCHEC : %s (0x%lx)\n", what, code);
    return 1;
}

/* Décode le dernier paquet (4:2:0 ou 4:4:4) et l'écrit en y4m. */
static int decode_check(const AVCodecContext *enc, const AVPacket *pkt, LUID luid)
{
    const uint8_t *h = enc->extradata;
    int chroma444 = h[5], hdr = h[6];
    int w = (int)(h[8] | h[9] << 8 | h[10] << 16 | (uint32_t)h[11] << 24);
    int hgt = (int)(h[12] | h[13] << 8 | h[14] << 16 | (uint32_t)h[15] << 24);
    pyrowave_device dev = NULL;
    pyrowave_decoder dec = NULL;
    pyrowave_result r;
    int cw = chroma444 ? w : w / 2, ch = chroma444 ? hgt : hgt / 2, ret = 1;
    size_t bpc = hdr ? 2 : 1;

    r = pyrowave_create_device_by_compat(0, 0, NULL, NULL, (const pyrowave_luid *)&luid,
                                         VK_QUEUE_GLOBAL_PRIORITY_MEDIUM, &dev);
    if (r != PYROWAVE_SUCCESS)
        return fail("décodeur : périphérique Vulkan", r);
    pyrowave_decoder_create_info info = { dev, w, hgt,
        chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420, false };
    if ((r = pyrowave_decoder_create(&info, &dec)) != PYROWAVE_SUCCESS) {
        fail("décodeur : création", r);
        goto end;
    }
    if ((r = pyrowave_decoder_push_packet(dec, pkt->data, pkt->size)) != PYROWAVE_SUCCESS) {
        fail("décodeur : paquet refusé", r);
        goto end;
    }
    if (!pyrowave_decoder_decode_is_ready(dec, false)) {
        fail("décodeur : image incomplète", 0);
        goto end;
    }
    uint8_t *planes = malloc((size_t)w * hgt * bpc + 2 * (size_t)cw * ch * bpc);
    pyrowave_cpu_buffer buf = { 0 };
    buf.data[0] = planes;
    buf.data[1] = planes + (size_t)w * hgt * bpc;
    buf.data[2] = (uint8_t *)buf.data[1] + (size_t)cw * ch * bpc;
    buf.row_stride_in_bytes[0] = w * bpc;
    buf.row_stride_in_bytes[1] = buf.row_stride_in_bytes[2] = cw * bpc;
    buf.plane_size_in_bytes[0] = (size_t)w * hgt * bpc;
    buf.plane_size_in_bytes[1] = buf.plane_size_in_bytes[2] = (size_t)cw * ch * bpc;
    buf.width = w;
    buf.height = hgt;
    buf.format = chroma444 ? (hdr ? PYROWAVE_CPU_BUFFER_FORMAT_YUV444P16 : PYROWAVE_CPU_BUFFER_FORMAT_YUV444P)
                           : (hdr ? PYROWAVE_CPU_BUFFER_FORMAT_YUV420P16 : PYROWAVE_CPU_BUFFER_FORMAT_YUV420P);
    if ((r = pyrowave_decoder_decode_cpu_buffer_synchronous(dec, &buf)) != PYROWAVE_SUCCESS) {
        fail("décodeur : décodage", r);
        free(planes);
        goto end;
    }
    FILE *y4m = fopen("pyrowave-test.y4m", "wb");
    if (y4m) {
        fprintf(y4m, "YUV4MPEG2 W%d H%d F60:1 Ip A1:1 C%s XCOLORRANGE=%s\nFRAME\n", w, hgt,
                chroma444 ? (hdr ? "444p16" : "444") : (hdr ? "420p16" : "420jpeg"), hdr ? "LIMITED" : "FULL");
        fwrite(planes, 1, buf.plane_size_in_bytes[0] + 2 * buf.plane_size_in_bytes[1], y4m);
        fclose(y4m);
    }
    /* Une image décodée toute noire ou toute plate trahit un import raté. */
    uint64_t sum = 0, sum2 = 0;
    for (int y = 0; y < hgt; y += 4)
        for (int x = 0; x < w; x += 4) {
            unsigned v = bpc == 2 ? ((uint16_t *)planes)[(size_t)y * w + x] >> 8 : planes[(size_t)y * w + x];
            sum += v;
            sum2 += (uint64_t)v * v;
        }
    double n = (double)((hgt + 3) / 4) * ((w + 3) / 4), mean = sum / n, var = sum2 / n - mean * mean;
    printf("Décodée : %dx%d, luma moyenne %.1f, écart type %.1f (%s)\n", w, hgt, mean, var > 0 ? sqrt(var) : 0,
           var > 4 ? "image plausible" : "image uniforme : écran vide, ou import raté ?");
    printf("Dernière image écrite dans pyrowave-test.y4m\n");
    free(planes);
    ret = 0;
end:
    if (dec)
        pyrowave_decoder_destroy(dec);
    pyrowave_device_destroy(dev);
    return ret;
}

int main(int argc, char **argv)
{
    int frames = argc > 1 ? atoi(argv[1]) : 120;
    int mbps = argc > 2 ? atoi(argv[2]) : 200;
    int out_w = argc > 4 ? atoi(argv[3]) : 0, out_h = argc > 4 ? atoi(argv[4]) : 0;
    AVBufferRef *device_ref = NULL, *frames_ref = NULL;
    AVCodecContext *enc = NULL;
    IDXGIOutputDuplication *dup = NULL;
    HRESULT hr;
    int ret;

    /* Carte et écran principal : adaptateur 0, sortie 0 (comme txproto). */
    IDXGIFactory1 *factory = NULL;
    IDXGIAdapter1 *adapter = NULL;
    IDXGIOutput *output = NULL;
    IDXGIOutput1 *output1 = NULL;
    DXGI_ADAPTER_DESC1 adesc;
    if (FAILED(hr = CreateDXGIFactory1(&IID_IDXGIFactory1, (void **)&factory)) ||
        FAILED(hr = IDXGIFactory1_EnumAdapters1(factory, 0, &adapter)) ||
        FAILED(hr = IDXGIAdapter1_EnumOutputs(adapter, 0, &output)) ||
        FAILED(hr = IDXGIOutput_QueryInterface(output, &IID_IDXGIOutput1, (void **)&output1)))
        return fail("écran principal", hr);
    IDXGIAdapter1_GetDesc1(adapter, &adesc);
    printf("Carte : %ls\n", adesc.Description);

    char index[8] = "0";
    if ((ret = av_hwdevice_ctx_create(&device_ref, AV_HWDEVICE_TYPE_D3D11VA, index, NULL, 0)) < 0)
        return fail("périphérique D3D11 de FFmpeg", ret);
    AVD3D11VADeviceContext *d3d = ((AVHWDeviceContext *)device_ref->data)->hwctx;
    if (FAILED(hr = IDXGIOutput1_DuplicateOutput(output1, (IUnknown *)d3d->device, &dup)))
        return fail("duplication de l'écran (lancer depuis la session ouverte, pas en service)", hr);
    DXGI_OUTDUPL_DESC ddesc;
    IDXGIOutputDuplication_GetDesc(dup, &ddesc);
    int w = ddesc.ModeDesc.Width, h = ddesc.ModeDesc.Height;
    printf("Écran : %dx%d, format DXGI %d\n", w, h, ddesc.ModeDesc.Format);

    frames_ref = av_hwframe_ctx_alloc(device_ref);
    AVHWFramesContext *fc = (AVHWFramesContext *)frames_ref->data;
    fc->format = AV_PIX_FMT_D3D11;
    fc->sw_format = AV_PIX_FMT_BGRA;
    fc->width = w;
    fc->height = h;
    fc->initial_pool_size = 4;
    ((AVD3D11VAFramesContext *)fc->hwctx)->BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
    if ((ret = av_hwframe_ctx_init(frames_ref)) < 0)
        return fail("images D3D11", ret);

    const AVCodec *codec = avcodec_find_encoder_by_name("libpyrowave");
    if (!codec)
        return fail("encodeur libpyrowave absent de ce FFmpeg", 0);
    enc = avcodec_alloc_context3(codec);
    enc->width = w;
    enc->height = h;
    enc->pix_fmt = AV_PIX_FMT_D3D11;
    enc->time_base = (AVRational){ 1, 60 };
    enc->framerate = (AVRational){ 60, 1 };
    enc->bit_rate = (int64_t)mbps * 1000000;
    enc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
    enc->hw_frames_ctx = av_buffer_ref(frames_ref);
    if (out_w > 0 && out_h > 0) {
        av_opt_set_int(enc->priv_data, "out_width", out_w, 0);
        av_opt_set_int(enc->priv_data, "out_height", out_h, 0);
    }
    av_log_set_level(AV_LOG_VERBOSE);
    if ((ret = avcodec_open2(enc, codec, NULL)) < 0)
        return fail("ouverture de l'encodeur (voir le journal ci-dessus)", ret);
    printf("En-tête : ");
    for (int i = 0; i < enc->extradata_size; i++)
        printf("%02x", enc->extradata[i]);
    printf(" (\"KNPW\", version, chroma, couleur, plage, largeur, hauteur)\n");

    FILE *pwv = fopen("pyrowave-test.pwv", "wb");
    AVPacket *pkt = av_packet_alloc(), *last = av_packet_alloc();
    /* Une seule image D3D11, mise à jour quand l'écran change : un écran
     * immobile est réencodé tel quel (l'encodeur la copie à chaque fois). */
    AVFrame *frame = av_frame_alloc();
    if ((ret = av_hwframe_get_buffer(frames_ref, frame, 0)) < 0)
        return fail("image D3D11 libre", ret);
    int captured = 0;
    double total_ms = 0, max_ms = 0;
    int64_t total_bytes = 0;
    int done = 0;
    for (int i = 0; done < frames && i < frames * 20; i++) {
        DXGI_OUTDUPL_FRAME_INFO fi;
        IDXGIResource *res = NULL;
        hr = IDXGIOutputDuplication_AcquireNextFrame(dup, 16, &fi, &res);
        if (FAILED(hr) && hr != DXGI_ERROR_WAIT_TIMEOUT)
            return fail("image de l'écran", hr);
        if (!res && !captured)
            continue; /* pas encore de première image */
        if (res) {
            captured = 1;
            ID3D11Texture2D *tex = NULL;
            IDXGIResource_QueryInterface(res, &IID_ID3D11Texture2D, (void **)&tex);
            d3d->lock(d3d->lock_ctx);
            ID3D11DeviceContext_CopySubresourceRegion(d3d->device_context, (ID3D11Resource *)frame->data[0],
                                                      (UINT)(intptr_t)frame->data[1], 0, 0, 0,
                                                      (ID3D11Resource *)tex, 0, NULL);
            d3d->unlock(d3d->lock_ctx);
            ID3D11Texture2D_Release(tex);
            IDXGIResource_Release(res);
            IDXGIOutputDuplication_ReleaseFrame(dup);
        }
        frame->pts = done;
        double t0 = now_ms();
        if ((ret = avcodec_send_frame(enc, frame)) < 0)
            return fail("envoi à l'encodeur", ret);
        if ((ret = avcodec_receive_packet(enc, pkt)) < 0)
            return fail("paquet de l'encodeur", ret);
        double dt = now_ms() - t0;
        total_ms += dt;
        if (dt > max_ms)
            max_ms = dt;
        total_bytes += pkt->size;
        uint32_t size = pkt->size;
        fwrite(&size, 4, 1, pwv);
        fwrite(pkt->data, 1, pkt->size, pwv);
        av_packet_unref(last);
        av_packet_move_ref(last, pkt);
        done++;
    }
    fclose(pwv);
    printf("%d images encodées : %.2f ms en moyenne (copie + encodage + paquets), %.2f ms au plus, "
           "%.0f Ko par image en moyenne (budget %d Ko à %d Mbit/s et 60 i/s)\n",
           done, total_ms / done, max_ms, total_bytes / 1024.0 / done, mbps * 1000000 / 8 / 60 / 1024, mbps);
    printf("Paquets écrits dans pyrowave-test.pwv\n");

    ret = decode_check(enc, last, adesc.AdapterLuid);
    avcodec_free_context(&enc);
    av_buffer_unref(&frames_ref);
    av_buffer_unref(&device_ref);
    printf(ret ? "ÉCHEC\n" : "OK\n");
    return ret;
}
