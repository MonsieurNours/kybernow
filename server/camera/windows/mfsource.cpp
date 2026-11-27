// KyberNowCameraSource.dll : la source de la caméra virtuelle de Windows 11
// (docs/webcam.md, « Station Windows »). Inscrite par
// `kybernow-camera --install` (COM, HKLM) puis donnée à MFCreateVirtualCamera :
// le service Frame Server la charge quand une app filme, et toutes les apps
// (Teams, Zoom, Discord, OBS, Caméra de Windows, navigateurs) voient
// « KyberNow Camera ».
//
// Un flux vidéo NV12, 1280 × 720 ou 1920 × 1080 à 30 images par seconde,
// lu dans la mémoire partagée de kybernow-camera.exe (kn_shared.h) ; sans
// appareil qui envoie, du gris foncé. Écrit d'après la documentation de
// Media Foundation (IMFMediaSourceEx, IMFMediaStream2, IKsControl, que Frame
// Server demande à une source de caméra).
#include <windows.h>
#include <mfapi.h>
#include <mferror.h>
#include <mfidl.h>
#include <dmksctrl.h>
#include <new>

#include "kn_shared.h"

namespace {

const GUID SOURCE_CLSID = KN_SOURCE_CLSID_INIT;

// mfidl.h du SDK de Windows, absents de MinGW.
const GUID MF_DEVICESTREAM_STREAM_CATEGORY_ = { 0x2939e7b8, 0xa62e, 0x4579, { 0xb6, 0x74, 0xd4, 0x07, 0x3d, 0xfa, 0xbb, 0xba } };
const GUID MF_DEVICESTREAM_STREAM_ID_ = { 0x11bd5120, 0xd124, 0x446b, { 0x88, 0xe6, 0x17, 0x06, 0x02, 0x57, 0xff, 0xf9 } };
const GUID MF_DEVICESTREAM_FRAMESERVER_SHARED_ = { 0x1cb378e9, 0xb279, 0x41d4, { 0xaf, 0x97, 0x34, 0xa2, 0x43, 0xe6, 0x83, 0x20 } };
const GUID MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES_ = { 0x17145fd1, 0x1b2b, 0x423c, { 0x80, 0x01, 0x2b, 0x68, 0x33, 0xed, 0x35, 0x88 } };
const GUID PINNAME_VIDEO_CAPTURE_ = { 0xfb6c4281, 0x0353, 0x11d1, { 0x90, 0x5f, 0x00, 0x00, 0xc0, 0xcc, 0x16, 0xba } };
const UINT32 MFFrameSourceTypes_Color_ = 0x0001;
// ks.h / ksproxy.h : l'interface que Frame Server demande à une source.
const GUID IID_IKsControl_ = { 0x28f54685, 0x06fd, 0x11d2, { 0xb2, 0x7a, 0x00, 0xa0, 0xc9, 0x22, 0x31, 0x96 } };

// IMFMediaStream2 (mfidl.h, Windows 8 et après).
const GUID IID_IMFMediaStream2_ = { 0xc5bc37d6, 0x75c7, 0x46a1, { 0xa1, 0x32, 0x81, 0xb5, 0xf7, 0x23, 0xc2, 0x0f } };
struct IMFMediaStream2_ : public IMFMediaStream {
    virtual HRESULT STDMETHODCALLTYPE SetStreamState(MF_STREAM_STATE value) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetStreamState(MF_STREAM_STATE *value) = 0;
};

LONG g_objects = 0;
LONG g_locks = 0;
HMODULE g_module = NULL;

const int SIZES[2][2] = { { 1280, 720 }, { 1920, 1080 } };
const LONGLONG FRAME_100NS = 10000000 / 30;

HRESULT make_type(int w, int h, IMFMediaType **out)
{
    IMFMediaType *t = NULL;
    HRESULT hr = MFCreateMediaType(&t);
    if (FAILED(hr))
        return hr;
    t->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Video);
    t->SetGUID(MF_MT_SUBTYPE, MFVideoFormat_NV12);
    MFSetAttributeSize(t, MF_MT_FRAME_SIZE, w, h);
    MFSetAttributeRatio(t, MF_MT_FRAME_RATE, 30, 1);
    MFSetAttributeRatio(t, MF_MT_PIXEL_ASPECT_RATIO, 1, 1);
    t->SetUINT32(MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive);
    t->SetUINT32(MF_MT_ALL_SAMPLES_INDEPENDENT, TRUE);
    t->SetUINT32(MF_MT_DEFAULT_STRIDE, (UINT32)w);
    t->SetUINT32(MF_MT_SAMPLE_SIZE, (UINT32)(w * h * 3 / 2));
    t->SetUINT32(MF_MT_AVG_BITRATE, (UINT32)(w * h * 3 / 2 * 8 * 30));
    *out = t;
    return S_OK;
}

class Source;

class Stream final : public IMFMediaStream2_ {
public:
    Stream(Source *source, IMFStreamDescriptor *sd) : ref_(1), source_(source), sd_(sd)
    {
        InitializeCriticalSection(&lock_);
        sd_->AddRef();
        MFCreateEventQueue(&queue_);
        InterlockedIncrement(&g_objects);
    }

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (!ppv)
            return E_POINTER;
        if (iid == IID_IUnknown || iid == IID_IMFMediaEventGenerator || iid == IID_IMFMediaStream || iid == IID_IMFMediaStream2_)
            *ppv = static_cast<IMFMediaStream2_ *>(this);
        else {
            *ppv = NULL;
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&ref_); }
    STDMETHODIMP_(ULONG) Release() override
    {
        LONG r = InterlockedDecrement(&ref_);
        if (!r)
            delete this;
        return r;
    }

    // IMFMediaEventGenerator : la file est prise sous le verrou (Shutdown la
    // libère depuis un autre fil), puis utilisée hors du verrou.
    IMFMediaEventQueue *queue()
    {
        EnterCriticalSection(&lock_);
        IMFMediaEventQueue *q = queue_;
        if (q)
            q->AddRef();
        LeaveCriticalSection(&lock_);
        return q;
    }
    STDMETHODIMP BeginGetEvent(IMFAsyncCallback *cb, IUnknown *state) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->BeginGetEvent(cb, state);
        q->Release();
        return hr;
    }
    STDMETHODIMP EndGetEvent(IMFAsyncResult *r, IMFMediaEvent **e) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->EndGetEvent(r, e);
        q->Release();
        return hr;
    }
    STDMETHODIMP GetEvent(DWORD flags, IMFMediaEvent **e) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->GetEvent(flags, e);
        q->Release();
        return hr;
    }
    STDMETHODIMP QueueEvent(MediaEventType t, REFGUID ext, HRESULT status, const PROPVARIANT *v) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->QueueEventParamVar(t, ext, status, v);
        q->Release();
        return hr;
    }

    // IMFMediaStream
    STDMETHODIMP GetMediaSource(IMFMediaSource **out) override;
    STDMETHODIMP GetStreamDescriptor(IMFStreamDescriptor **out) override
    {
        if (!out)
            return E_POINTER;
        *out = sd_;
        sd_->AddRef();
        return S_OK;
    }
    STDMETHODIMP RequestSample(IUnknown *token) override
    {
        EnterCriticalSection(&lock_);
        if (!queue_) {
            LeaveCriticalSection(&lock_);
            return MF_E_SHUTDOWN;
        }
        if (state_ != MF_STREAM_STATE_RUNNING) {
            LeaveCriticalSection(&lock_);
            return MF_E_INVALIDREQUEST;
        }
        int w = width_, h = height_;
        LONGLONG last = last_;
        LeaveCriticalSection(&lock_);

        // Une image toutes les 1/30 s, quel que soit le rythme des demandes
        // (l'attente se fait hors du verrou : Shutdown n'attend pas).
        LONGLONG now = MFGetSystemTime();
        if (last && now < last + FRAME_100NS) {
            Sleep((DWORD)((last + FRAME_100NS - now) / 10000));
            now = MFGetSystemTime();
        }
        EnterCriticalSection(&lock_);
        last_ = now;
        LeaveCriticalSection(&lock_);

        DWORD size = (DWORD)(w * h * 3 / 2);
        IMFSample *sample = NULL;
        IMFMediaBuffer *buffer = NULL;
        HRESULT hr = MFCreateSample(&sample);
        if (SUCCEEDED(hr))
            hr = MFCreateMemoryBuffer(size, &buffer);
        if (SUCCEEDED(hr)) {
            BYTE *data = NULL;
            hr = buffer->Lock(&data, NULL, NULL);
            if (SUCCEEDED(hr)) {
                // Lue sous le verrou : Shutdown (l'app qui filme se ferme)
                // démappe la vue ; une lecture en cours ferait tomber Frame
                // Server, et une lecture après rouvrirait la mémoire pour rien.
                EnterCriticalSection(&lock_);
                if (queue_)
                    kn_shared_read(&mapping_, &view_, data, w, h);
                else
                    hr = MF_E_SHUTDOWN;
                LeaveCriticalSection(&lock_);
                buffer->Unlock();
                buffer->SetCurrentLength(size);
                hr = sample->AddBuffer(buffer);
            }
        }
        if (SUCCEEDED(hr)) {
            sample->SetSampleTime(now);
            sample->SetSampleDuration(FRAME_100NS);
            sample->SetUINT32(MFSampleExtension_CleanPoint, TRUE);
            if (token)
                sample->SetUnknown(MFSampleExtension_Token, token);
            EnterCriticalSection(&lock_);
            hr = queue_ ? queue_->QueueEventParamUnk(MEMediaSample, GUID_NULL, S_OK, sample) : MF_E_SHUTDOWN;
            LeaveCriticalSection(&lock_);
        }
        if (buffer)
            buffer->Release();
        if (sample)
            sample->Release();
        return hr;
    }

    // IMFMediaStream2
    STDMETHODIMP SetStreamState(MF_STREAM_STATE value) override
    {
        EnterCriticalSection(&lock_);
        state_ = value;
        LeaveCriticalSection(&lock_);
        return S_OK;
    }
    STDMETHODIMP GetStreamState(MF_STREAM_STATE *value) override
    {
        if (!value)
            return E_POINTER;
        EnterCriticalSection(&lock_);
        *value = state_;
        LeaveCriticalSection(&lock_);
        return S_OK;
    }

    // De la source.
    HRESULT Start()
    {
        IMFMediaTypeHandler *handler = NULL;
        IMFMediaType *type = NULL;
        UINT32 w = 1280, h = 720;
        if (SUCCEEDED(sd_->GetMediaTypeHandler(&handler)) && SUCCEEDED(handler->GetCurrentMediaType(&type))) {
            MFGetAttributeSize(type, MF_MT_FRAME_SIZE, &w, &h);
            type->Release();
        }
        if (handler)
            handler->Release();
        EnterCriticalSection(&lock_);
        width_ = (int)w;
        height_ = (int)h;
        state_ = MF_STREAM_STATE_RUNNING;
        last_ = 0;
        LeaveCriticalSection(&lock_);
        return QueueEvent(MEStreamStarted, GUID_NULL, S_OK, NULL);
    }
    HRESULT Stop()
    {
        EnterCriticalSection(&lock_);
        state_ = MF_STREAM_STATE_STOPPED;
        LeaveCriticalSection(&lock_);
        return QueueEvent(MEStreamStopped, GUID_NULL, S_OK, NULL);
    }
    void Shutdown()
    {
        EnterCriticalSection(&lock_);
        if (queue_) {
            queue_->Shutdown();
            queue_->Release();
            queue_ = NULL;
        }
        state_ = MF_STREAM_STATE_STOPPED;
        if (view_)
            UnmapViewOfFile(view_);
        if (mapping_)
            CloseHandle(mapping_);
        view_ = NULL;
        mapping_ = NULL;
        source_ = NULL;
        LeaveCriticalSection(&lock_);
    }
    bool running()
    {
        EnterCriticalSection(&lock_);
        bool r = state_ == MF_STREAM_STATE_RUNNING;
        LeaveCriticalSection(&lock_);
        return r;
    }

private:
    ~Stream()
    {
        Shutdown();
        sd_->Release();
        DeleteCriticalSection(&lock_);
        InterlockedDecrement(&g_objects);
    }

    LONG ref_;
    CRITICAL_SECTION lock_;
    Source *source_;
    IMFStreamDescriptor *sd_;
    IMFMediaEventQueue *queue_ = NULL;
    MF_STREAM_STATE state_ = MF_STREAM_STATE_STOPPED;
    int width_ = 1280, height_ = 720;
    LONGLONG last_ = 0;
    HANDLE mapping_ = NULL;
    struct kn_shared *view_ = NULL;
};

class Source final : public IMFMediaSourceEx, public IMFGetService, public IKsControl {
public:
    static HRESULT Create(Source **out)
    {
        Source *s = new (std::nothrow) Source();
        if (!s)
            return E_OUTOFMEMORY;
        HRESULT hr = s->Init();
        if (FAILED(hr)) {
            s->Release();
            return hr;
        }
        *out = s;
        return S_OK;
    }

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (!ppv)
            return E_POINTER;
        if (iid == IID_IUnknown || iid == IID_IMFMediaEventGenerator || iid == IID_IMFMediaSource || iid == IID_IMFMediaSourceEx)
            *ppv = static_cast<IMFMediaSourceEx *>(this);
        else if (iid == IID_IMFGetService)
            *ppv = static_cast<IMFGetService *>(this);
        else if (iid == IID_IKsControl_)
            *ppv = static_cast<IKsControl *>(this);
        else {
            *ppv = NULL;
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&ref_); }
    STDMETHODIMP_(ULONG) Release() override
    {
        LONG r = InterlockedDecrement(&ref_);
        if (!r)
            delete this;
        return r;
    }

    // IMFMediaEventGenerator : la file est prise sous le verrou (Shutdown la
    // libère depuis un autre fil), puis utilisée hors du verrou.
    IMFMediaEventQueue *queue()
    {
        EnterCriticalSection(&lock_);
        IMFMediaEventQueue *q = queue_;
        if (q)
            q->AddRef();
        LeaveCriticalSection(&lock_);
        return q;
    }
    STDMETHODIMP BeginGetEvent(IMFAsyncCallback *cb, IUnknown *state) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->BeginGetEvent(cb, state);
        q->Release();
        return hr;
    }
    STDMETHODIMP EndGetEvent(IMFAsyncResult *r, IMFMediaEvent **e) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->EndGetEvent(r, e);
        q->Release();
        return hr;
    }
    STDMETHODIMP GetEvent(DWORD flags, IMFMediaEvent **e) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->GetEvent(flags, e);
        q->Release();
        return hr;
    }
    STDMETHODIMP QueueEvent(MediaEventType t, REFGUID ext, HRESULT status, const PROPVARIANT *v) override
    {
        IMFMediaEventQueue *q = queue();
        if (!q)
            return MF_E_SHUTDOWN;
        HRESULT hr = q->QueueEventParamVar(t, ext, status, v);
        q->Release();
        return hr;
    }

    // IMFMediaSource
    STDMETHODIMP GetCharacteristics(DWORD *out) override
    {
        if (!out)
            return E_POINTER;
        if (!queue_)
            return MF_E_SHUTDOWN;
        *out = MFMEDIASOURCE_IS_LIVE;
        return S_OK;
    }
    STDMETHODIMP CreatePresentationDescriptor(IMFPresentationDescriptor **out) override
    {
        if (!out)
            return E_POINTER;
        if (!pd_)
            return MF_E_SHUTDOWN;
        return pd_->Clone(out);
    }
    STDMETHODIMP Start(IMFPresentationDescriptor *pd, const GUID *format, const PROPVARIANT *start) override
    {
        if (!pd || !start)
            return E_INVALIDARG;
        if (format && *format != GUID_NULL)
            return MF_E_UNSUPPORTED_TIME_FORMAT;
        EnterCriticalSection(&lock_);
        if (!queue_) {
            LeaveCriticalSection(&lock_);
            return MF_E_SHUTDOWN;
        }
        BOOL selected = FALSE;
        IMFStreamDescriptor *sd = NULL;
        HRESULT hr = pd->GetStreamDescriptorByIndex(0, &selected, &sd);
        if (SUCCEEDED(hr) && selected) {
            // Le format choisi par l'app, recopié dans notre descripteur.
            IMFMediaTypeHandler *in = NULL, *mine = NULL;
            IMFMediaType *type = NULL;
            if (SUCCEEDED(sd->GetMediaTypeHandler(&in)) && SUCCEEDED(in->GetCurrentMediaType(&type))) {
                IMFStreamDescriptor *own = NULL;
                stream_->GetStreamDescriptor(&own);
                if (own && SUCCEEDED(own->GetMediaTypeHandler(&mine))) {
                    mine->SetCurrentMediaType(type);
                    mine->Release();
                }
                if (own)
                    own->Release();
                type->Release();
            }
            if (in)
                in->Release();
            hr = queue_->QueueEventParamUnk(started_once_ ? MEUpdatedStream : MENewStream, GUID_NULL, S_OK,
                                            static_cast<IMFMediaStream *>(stream_));
            started_once_ = true;
            if (SUCCEEDED(hr))
                hr = stream_->Start();
        }
        if (sd)
            sd->Release();
        if (SUCCEEDED(hr)) {
            PROPVARIANT t;
            PropVariantInit(&t);
            t.vt = VT_I8;
            t.hVal.QuadPart = MFGetSystemTime();
            hr = queue_->QueueEventParamVar(MESourceStarted, GUID_NULL, S_OK, &t);
        }
        LeaveCriticalSection(&lock_);
        return hr;
    }
    STDMETHODIMP Stop() override
    {
        EnterCriticalSection(&lock_);
        HRESULT hr = queue_ ? S_OK : MF_E_SHUTDOWN;
        if (SUCCEEDED(hr) && stream_->running())
            hr = stream_->Stop();
        if (SUCCEEDED(hr))
            hr = queue_->QueueEventParamVar(MESourceStopped, GUID_NULL, S_OK, NULL);
        LeaveCriticalSection(&lock_);
        return hr;
    }
    STDMETHODIMP Pause() override { return MF_E_INVALID_STATE_TRANSITION; }
    STDMETHODIMP Shutdown() override
    {
        EnterCriticalSection(&lock_);
        if (stream_) {
            stream_->Shutdown();
            stream_->Release();
            stream_ = NULL;
        }
        if (queue_) {
            queue_->Shutdown();
            queue_->Release();
            queue_ = NULL;
        }
        if (pd_) {
            pd_->Release();
            pd_ = NULL;
        }
        if (attributes_) {
            attributes_->Release();
            attributes_ = NULL;
        }
        LeaveCriticalSection(&lock_);
        return S_OK;
    }

    // IMFMediaSourceEx
    STDMETHODIMP GetSourceAttributes(IMFAttributes **out) override
    {
        if (!out)
            return E_POINTER;
        if (!attributes_)
            return MF_E_SHUTDOWN;
        *out = attributes_;
        attributes_->AddRef();
        return S_OK;
    }
    STDMETHODIMP GetStreamAttributes(DWORD id, IMFAttributes **out) override
    {
        if (!out)
            return E_POINTER;
        if (id != 0)
            return MF_E_INVALIDSTREAMNUMBER;
        if (!stream_)
            return MF_E_SHUTDOWN;
        IMFStreamDescriptor *sd = NULL;
        HRESULT hr = stream_->GetStreamDescriptor(&sd);
        if (SUCCEEDED(hr)) {
            hr = sd->QueryInterface(IID_IMFAttributes, (void **)out);
            sd->Release();
        }
        return hr;
    }
    STDMETHODIMP SetD3DManager(IUnknown *) override { return S_OK; }

    // IMFGetService
    STDMETHODIMP GetService(REFGUID, REFIID, LPVOID *ppv) override
    {
        if (ppv)
            *ppv = NULL;
        return MF_E_UNSUPPORTED_SERVICE;
    }

    // IKsControl : aucune propriété de caméra (exposition, mise au point…).
    STDMETHODIMP KsProperty(PKSPROPERTY, ULONG, LPVOID, ULONG, ULONG *returned) override
    {
        if (returned)
            *returned = 0;
        return HRESULT_FROM_WIN32(ERROR_SET_NOT_FOUND);
    }
    STDMETHODIMP KsMethod(PKSMETHOD, ULONG, LPVOID, ULONG, ULONG *returned) override
    {
        if (returned)
            *returned = 0;
        return HRESULT_FROM_WIN32(ERROR_SET_NOT_FOUND);
    }
    STDMETHODIMP KsEvent(PKSEVENT, ULONG, LPVOID, ULONG, ULONG *returned) override
    {
        if (returned)
            *returned = 0;
        return HRESULT_FROM_WIN32(ERROR_SET_NOT_FOUND);
    }

    HRESULT stream_source(IMFMediaSource **out)
    {
        return QueryInterface(IID_IMFMediaSource, (void **)out);
    }

private:
    Source() : ref_(1)
    {
        InitializeCriticalSection(&lock_);
        InterlockedIncrement(&g_objects);
    }
    ~Source()
    {
        Shutdown();
        DeleteCriticalSection(&lock_);
        InterlockedDecrement(&g_objects);
    }

    HRESULT Init()
    {
        HRESULT hr = MFCreateEventQueue(&queue_);
        if (SUCCEEDED(hr))
            hr = MFCreateAttributes(&attributes_, 4);
        IMFMediaType *types[2] = { NULL, NULL };
        for (int i = 0; SUCCEEDED(hr) && i < 2; i++)
            hr = make_type(SIZES[i][0], SIZES[i][1], &types[i]);
        IMFStreamDescriptor *sd = NULL;
        if (SUCCEEDED(hr))
            hr = MFCreateStreamDescriptor(0, 2, types, &sd);
        if (SUCCEEDED(hr)) {
            IMFMediaTypeHandler *handler = NULL;
            hr = sd->GetMediaTypeHandler(&handler);
            if (SUCCEEDED(hr)) {
                hr = handler->SetCurrentMediaType(types[0]);
                handler->Release();
            }
            sd->SetGUID(MF_DEVICESTREAM_STREAM_CATEGORY_, PINNAME_VIDEO_CAPTURE_);
            sd->SetUINT32(MF_DEVICESTREAM_STREAM_ID_, 0);
            sd->SetUINT32(MF_DEVICESTREAM_FRAMESERVER_SHARED_, 1);
            sd->SetUINT32(MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES_, MFFrameSourceTypes_Color_);
        }
        if (SUCCEEDED(hr))
            hr = MFCreatePresentationDescriptor(1, &sd, &pd_);
        if (SUCCEEDED(hr))
            hr = pd_->SelectStream(0);
        if (SUCCEEDED(hr)) {
            stream_ = new (std::nothrow) Stream(this, sd);
            if (!stream_)
                hr = E_OUTOFMEMORY;
        }
        if (sd)
            sd->Release();
        for (auto t : types)
            if (t)
                t->Release();
        return hr;
    }

    LONG ref_;
    CRITICAL_SECTION lock_;
    IMFMediaEventQueue *queue_ = NULL;
    IMFAttributes *attributes_ = NULL;
    IMFPresentationDescriptor *pd_ = NULL;
    Stream *stream_ = NULL;
    bool started_once_ = false;
};

STDMETHODIMP Stream::GetMediaSource(IMFMediaSource **out)
{
    if (!out)
        return E_POINTER;
    EnterCriticalSection(&lock_);
    HRESULT hr = source_ ? source_->stream_source(out) : MF_E_SHUTDOWN;
    LeaveCriticalSection(&lock_);
    return hr;
}

// L'objet que crée Frame Server (CoCreateInstance du CLSID inscrit) : un
// IMFActivate qui fait la source à la demande.
class Activate final : public IMFActivate {
public:
    Activate() : ref_(1)
    {
        MFCreateAttributes(&attributes_, 1);
        InterlockedIncrement(&g_objects);
    }

    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (!ppv)
            return E_POINTER;
        if (iid == IID_IUnknown || iid == IID_IMFAttributes || iid == IID_IMFActivate)
            *ppv = static_cast<IMFActivate *>(this);
        else {
            *ppv = NULL;
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&ref_); }
    STDMETHODIMP_(ULONG) Release() override
    {
        LONG r = InterlockedDecrement(&ref_);
        if (!r)
            delete this;
        return r;
    }

    STDMETHODIMP ActivateObject(REFIID iid, void **ppv) override
    {
        if (!ppv)
            return E_POINTER;
        if (!source_) {
            HRESULT hr = Source::Create(&source_);
            if (FAILED(hr))
                return hr;
        }
        return source_->QueryInterface(iid, ppv);
    }
    STDMETHODIMP ShutdownObject() override { return S_OK; }
    STDMETHODIMP DetachObject() override
    {
        if (source_) {
            source_->Release();
            source_ = NULL;
        }
        return S_OK;
    }

    // IMFAttributes, confiés à un magasin de Media Foundation.
#define FWD(sig, call) STDMETHODIMP sig override { return attributes_->call; }
    FWD(GetItem(REFGUID k, PROPVARIANT *v), GetItem(k, v))
    FWD(GetItemType(REFGUID k, MF_ATTRIBUTE_TYPE *t), GetItemType(k, t))
    FWD(CompareItem(REFGUID k, REFPROPVARIANT v, BOOL *r), CompareItem(k, v, r))
    FWD(Compare(IMFAttributes *a, MF_ATTRIBUTES_MATCH_TYPE m, BOOL *r), Compare(a, m, r))
    FWD(GetUINT32(REFGUID k, UINT32 *v), GetUINT32(k, v))
    FWD(GetUINT64(REFGUID k, UINT64 *v), GetUINT64(k, v))
    FWD(GetDouble(REFGUID k, double *v), GetDouble(k, v))
    FWD(GetGUID(REFGUID k, GUID *v), GetGUID(k, v))
    FWD(GetStringLength(REFGUID k, UINT32 *n), GetStringLength(k, n))
    FWD(GetString(REFGUID k, LPWSTR s, UINT32 n, UINT32 *l), GetString(k, s, n, l))
    FWD(GetAllocatedString(REFGUID k, LPWSTR *s, UINT32 *l), GetAllocatedString(k, s, l))
    FWD(GetBlobSize(REFGUID k, UINT32 *n), GetBlobSize(k, n))
    FWD(GetBlob(REFGUID k, UINT8 *b, UINT32 n, UINT32 *l), GetBlob(k, b, n, l))
    FWD(GetAllocatedBlob(REFGUID k, UINT8 **b, UINT32 *l), GetAllocatedBlob(k, b, l))
    FWD(GetUnknown(REFGUID k, REFIID i, LPVOID *p), GetUnknown(k, i, p))
    FWD(SetItem(REFGUID k, REFPROPVARIANT v), SetItem(k, v))
    FWD(DeleteItem(REFGUID k), DeleteItem(k))
    FWD(DeleteAllItems(), DeleteAllItems())
    FWD(SetUINT32(REFGUID k, UINT32 v), SetUINT32(k, v))
    FWD(SetUINT64(REFGUID k, UINT64 v), SetUINT64(k, v))
    FWD(SetDouble(REFGUID k, double v), SetDouble(k, v))
    FWD(SetGUID(REFGUID k, REFGUID v), SetGUID(k, v))
    FWD(SetString(REFGUID k, LPCWSTR v), SetString(k, v))
    FWD(SetBlob(REFGUID k, const UINT8 *b, UINT32 n), SetBlob(k, b, n))
    FWD(SetUnknown(REFGUID k, IUnknown *u), SetUnknown(k, u))
    FWD(LockStore(), LockStore())
    FWD(UnlockStore(), UnlockStore())
    FWD(GetCount(UINT32 *n), GetCount(n))
    FWD(GetItemByIndex(UINT32 i, GUID *k, PROPVARIANT *v), GetItemByIndex(i, k, v))
    FWD(CopyAllItems(IMFAttributes *d), CopyAllItems(d))
#undef FWD

private:
    ~Activate()
    {
        if (source_)
            source_->Release();
        if (attributes_)
            attributes_->Release();
        InterlockedDecrement(&g_objects);
    }
    LONG ref_;
    IMFAttributes *attributes_ = NULL;
    Source *source_ = NULL;
};

class Factory final : public IClassFactory {
public:
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (!ppv)
            return E_POINTER;
        if (iid == IID_IUnknown || iid == IID_IClassFactory) {
            *ppv = static_cast<IClassFactory *>(this);
            AddRef();
            return S_OK;
        }
        *ppv = NULL;
        return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return 2; }
    STDMETHODIMP_(ULONG) Release() override { return 1; }
    STDMETHODIMP CreateInstance(IUnknown *outer, REFIID iid, void **ppv) override
    {
        if (outer)
            return CLASS_E_NOAGGREGATION;
        Activate *a = new (std::nothrow) Activate();
        if (!a)
            return E_OUTOFMEMORY;
        HRESULT hr = a->QueryInterface(iid, ppv);
        a->Release();
        return hr;
    }
    STDMETHODIMP LockServer(BOOL lock) override
    {
        if (lock)
            InterlockedIncrement(&g_locks);
        else
            InterlockedDecrement(&g_locks);
        return S_OK;
    }
};

Factory g_factory;

} // namespace

extern "C" BOOL WINAPI DllMain(HINSTANCE instance, DWORD reason, LPVOID)
{
    if (reason == DLL_PROCESS_ATTACH) {
        g_module = instance;
        DisableThreadLibraryCalls(instance);
    }
    return TRUE;
}

extern "C" HRESULT STDAPICALLTYPE DllGetClassObject(REFCLSID clsid, REFIID iid, void **ppv)
{
    if (clsid != SOURCE_CLSID)
        return CLASS_E_CLASSNOTAVAILABLE;
    return g_factory.QueryInterface(iid, ppv);
}

extern "C" HRESULT STDAPICALLTYPE DllCanUnloadNow()
{
    return g_objects == 0 && g_locks == 0 ? S_OK : S_FALSE;
}
