// KyberNowCameraFilter.dll : la caméra « KyberNow Camera » sous Windows 10
// (docs/webcam.md, « Station Windows »), où la caméra virtuelle de Windows 11
// n'existe pas. Un filtre source DirectShow, inscrit dans la catégorie des
// caméras (CLSID_VideoInputDeviceCategory) par `kybernow-camera --install` :
// Zoom, Teams, Discord, OBS, Chrome et Edge le proposent ; les apps qui ne
// lisent que Media Foundation (Caméra de Windows) ne le voient pas.
//
// Une broche de sortie YUY2, 1280 × 720 ou 1920 × 1080 à 30 images par
// seconde ; l'image vient de la mémoire partagée de kybernow-camera.exe
// (kn_shared.h, NV12, convertie ici), sinon du gris foncé. Écrit d'après la
// documentation de DirectShow (IBaseFilter, IPin, IAMStreamConfig,
// IKsPropertySet), sans les classes de base de l'exemple de Microsoft.
#include <windows.h>
#include <dshow.h>
#include <dvdmedia.h>
#include <new>
#include <string.h>
#include <wchar.h>

#include "kn_shared.h"

namespace {

const GUID FILTER_CLSID = KN_FILTER_CLSID_INIT;
const wchar_t *FILTER_NAME = KN_CAMERA_NAME_W;
const wchar_t *PIN_NAME = L"Capture";
const REFERENCE_TIME FRAME_100NS = 10000000 / 30;
const int SIZES[2][2] = { { 1280, 720 }, { 1920, 1080 } };

LONG g_objects = 0;
LONG g_locks = 0;
HMODULE g_module = NULL;

void free_type_content(AM_MEDIA_TYPE &mt)
{
    if (mt.cbFormat && mt.pbFormat)
        CoTaskMemFree(mt.pbFormat);
    mt.cbFormat = 0;
    mt.pbFormat = NULL;
    if (mt.pUnk) {
        mt.pUnk->Release();
        mt.pUnk = NULL;
    }
}

HRESULT copy_type(AM_MEDIA_TYPE &dst, const AM_MEDIA_TYPE &src)
{
    dst = src;
    if (src.cbFormat) {
        dst.pbFormat = (BYTE *)CoTaskMemAlloc(src.cbFormat);
        if (!dst.pbFormat) {
            dst.cbFormat = 0;
            return E_OUTOFMEMORY;
        }
        memcpy(dst.pbFormat, src.pbFormat, src.cbFormat);
    }
    if (dst.pUnk)
        dst.pUnk->AddRef();
    return S_OK;
}

// Le format YUY2 d'une des deux tailles.
void make_type(int index, AM_MEDIA_TYPE &mt)
{
    int w = SIZES[index][0], h = SIZES[index][1];
    memset(&mt, 0, sizeof mt);
    mt.majortype = MEDIATYPE_Video;
    mt.subtype = MEDIASUBTYPE_YUY2;
    mt.bFixedSizeSamples = TRUE;
    mt.bTemporalCompression = FALSE;
    mt.lSampleSize = (ULONG)(w * h * 2);
    mt.formattype = FORMAT_VideoInfo;
    mt.cbFormat = sizeof(VIDEOINFOHEADER);
    mt.pbFormat = (BYTE *)CoTaskMemAlloc(sizeof(VIDEOINFOHEADER));
    VIDEOINFOHEADER *vih = (VIDEOINFOHEADER *)mt.pbFormat;
    memset(vih, 0, sizeof *vih);
    vih->AvgTimePerFrame = FRAME_100NS;
    vih->dwBitRate = (DWORD)(w * h * 2 * 8 * 30);
    vih->bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    vih->bmiHeader.biWidth = w;
    vih->bmiHeader.biHeight = h;
    vih->bmiHeader.biPlanes = 1;
    vih->bmiHeader.biBitCount = 16;
    vih->bmiHeader.biCompression = MAKEFOURCC('Y', 'U', 'Y', '2');
    vih->bmiHeader.biSizeImage = (DWORD)(w * h * 2);
}

// La taille d'un format, s'il est l'un des nôtres ; -1 sinon.
int type_index(const AM_MEDIA_TYPE *mt)
{
    if (!mt || mt->majortype != MEDIATYPE_Video || mt->subtype != MEDIASUBTYPE_YUY2)
        return -1;
    if (mt->formattype != FORMAT_VideoInfo || mt->cbFormat < sizeof(VIDEOINFOHEADER) || !mt->pbFormat)
        return -1;
    const VIDEOINFOHEADER *vih = (const VIDEOINFOHEADER *)mt->pbFormat;
    for (int i = 0; i < 2; i++)
        if (vih->bmiHeader.biWidth == SIZES[i][0] && abs(vih->bmiHeader.biHeight) == SIZES[i][1])
            return i;
    return -1;
}

// NV12 vers YUY2 (deux pixels : Y0 U Y1 V).
void nv12_to_yuy2(const uint8_t *nv12, uint8_t *yuy2, int w, int h)
{
    const uint8_t *uv = nv12 + (size_t)w * h;
    for (int y = 0; y < h; y++) {
        const uint8_t *row = nv12 + (size_t)y * w;
        const uint8_t *c = uv + (size_t)(y / 2) * w;
        uint8_t *out = yuy2 + (size_t)y * w * 2;
        for (int x = 0; x < w; x += 2) {
            out[0] = row[x];
            out[1] = c[x];
            out[2] = row[x + 1];
            out[3] = c[x + 1];
            out += 4;
        }
    }
}

class Filter;

class EnumTypes final : public IEnumMediaTypes {
public:
    explicit EnumTypes(ULONG pos = 0) : ref_(1), pos_(pos) {}
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (iid == IID_IUnknown || iid == IID_IEnumMediaTypes) {
            *ppv = this;
            AddRef();
            return S_OK;
        }
        *ppv = NULL;
        return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&ref_); }
    STDMETHODIMP_(ULONG) Release() override
    {
        LONG r = InterlockedDecrement(&ref_);
        if (!r)
            delete this;
        return r;
    }
    STDMETHODIMP Next(ULONG n, AM_MEDIA_TYPE **types, ULONG *fetched) override
    {
        ULONG got = 0;
        while (got < n && pos_ < 2) {
            AM_MEDIA_TYPE *mt = (AM_MEDIA_TYPE *)CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE));
            if (!mt)
                break;
            make_type((int)pos_++, *mt);
            types[got++] = mt;
        }
        if (fetched)
            *fetched = got;
        return got == n ? S_OK : S_FALSE;
    }
    STDMETHODIMP Skip(ULONG n) override
    {
        pos_ += n;
        return pos_ <= 2 ? S_OK : S_FALSE;
    }
    STDMETHODIMP Reset() override
    {
        pos_ = 0;
        return S_OK;
    }
    STDMETHODIMP Clone(IEnumMediaTypes **out) override
    {
        *out = new (std::nothrow) EnumTypes(pos_);
        return *out ? S_OK : E_OUTOFMEMORY;
    }

private:
    LONG ref_;
    ULONG pos_;
};

class Pin final : public IPin, public IAMStreamConfig, public IKsPropertySet {
public:
    explicit Pin(Filter *filter) : filter_(filter)
    {
        InitializeCriticalSection(&lock_);
        make_type(0, preferred_);
        memset(&current_, 0, sizeof current_);
    }
    ~Pin()
    {
        Disconnect();
        free_type_content(preferred_);
        // Tampon de conversion et vue de la mémoire partagée : une instance
        // par graphe (Zoom ou Teams le recréent à chaque changement de caméra).
        free(nv12_);
        if (view_)
            UnmapViewOfFile(view_);
        if (mapping_)
            CloseHandle(mapping_);
        DeleteCriticalSection(&lock_);
    }

    // IUnknown : le compte du filtre.
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    // IPin
    STDMETHODIMP Connect(IPin *receiver, const AM_MEDIA_TYPE *mt) override
    {
        if (!receiver)
            return E_POINTER;
        EnterCriticalSection(&lock_);
        if (peer_) {
            LeaveCriticalSection(&lock_);
            return VFW_E_ALREADY_CONNECTED;
        }
        HRESULT hr = VFW_E_NO_ACCEPTABLE_TYPES;
        // Le format demandé, sinon celui choisi par SetFormat, sinon les deux tailles.
        AM_MEDIA_TYPE tries[3];
        int n = 0;
        if (mt && type_index(mt) >= 0)
            copy_type(tries[n++], *mt);
        copy_type(tries[n++], preferred_);
        int other = type_index(&preferred_) == 0 ? 1 : 0;
        make_type(other, tries[n++]);
        for (int i = 0; i < n && FAILED(hr); i++) {
            if (mt && mt->majortype != GUID_NULL && mt->majortype != MEDIATYPE_Video)
                break;
            if (SUCCEEDED(receiver->ReceiveConnection(this, &tries[i]))) {
                hr = attach(receiver, tries[i]);
                if (FAILED(hr))
                    receiver->Disconnect();
            }
        }
        for (int i = 0; i < n; i++)
            free_type_content(tries[i]);
        LeaveCriticalSection(&lock_);
        return hr;
    }
    STDMETHODIMP ReceiveConnection(IPin *, const AM_MEDIA_TYPE *) override { return E_UNEXPECTED; }
    STDMETHODIMP Disconnect() override
    {
        EnterCriticalSection(&lock_);
        if (!peer_) {
            LeaveCriticalSection(&lock_);
            return S_FALSE;
        }
        if (allocator_) {
            allocator_->Decommit();
            allocator_->Release();
            allocator_ = NULL;
        }
        if (input_) {
            input_->Release();
            input_ = NULL;
        }
        peer_->Release();
        peer_ = NULL;
        free_type_content(current_);
        memset(&current_, 0, sizeof current_);
        LeaveCriticalSection(&lock_);
        return S_OK;
    }
    STDMETHODIMP ConnectedTo(IPin **out) override
    {
        if (!out)
            return E_POINTER;
        EnterCriticalSection(&lock_);
        *out = peer_;
        if (peer_)
            peer_->AddRef();
        LeaveCriticalSection(&lock_);
        return *out ? S_OK : VFW_E_NOT_CONNECTED;
    }
    STDMETHODIMP ConnectionMediaType(AM_MEDIA_TYPE *mt) override
    {
        if (!mt)
            return E_POINTER;
        EnterCriticalSection(&lock_);
        HRESULT hr = peer_ ? copy_type(*mt, current_) : VFW_E_NOT_CONNECTED;
        LeaveCriticalSection(&lock_);
        return hr;
    }
    STDMETHODIMP QueryPinInfo(PIN_INFO *info) override;
    STDMETHODIMP QueryDirection(PIN_DIRECTION *dir) override
    {
        *dir = PINDIR_OUTPUT;
        return S_OK;
    }
    STDMETHODIMP QueryId(LPWSTR *id) override
    {
        *id = (LPWSTR)CoTaskMemAlloc((wcslen(PIN_NAME) + 1) * sizeof(wchar_t));
        if (!*id)
            return E_OUTOFMEMORY;
        wcscpy(*id, PIN_NAME);
        return S_OK;
    }
    STDMETHODIMP QueryAccept(const AM_MEDIA_TYPE *mt) override { return type_index(mt) >= 0 ? S_OK : S_FALSE; }
    STDMETHODIMP EnumMediaTypes(IEnumMediaTypes **out) override
    {
        *out = new (std::nothrow) EnumTypes();
        return *out ? S_OK : E_OUTOFMEMORY;
    }
    STDMETHODIMP QueryInternalConnections(IPin **, ULONG *) override { return E_NOTIMPL; }
    STDMETHODIMP EndOfStream() override { return E_UNEXPECTED; }
    STDMETHODIMP BeginFlush() override { return E_UNEXPECTED; }
    STDMETHODIMP EndFlush() override { return E_UNEXPECTED; }
    STDMETHODIMP NewSegment(REFERENCE_TIME, REFERENCE_TIME, double) override { return S_OK; }

    // IAMStreamConfig
    STDMETHODIMP SetFormat(AM_MEDIA_TYPE *mt) override
    {
        if (type_index(mt) < 0)
            return VFW_E_INVALIDMEDIATYPE;
        EnterCriticalSection(&lock_);
        free_type_content(preferred_);
        copy_type(preferred_, *mt);
        LeaveCriticalSection(&lock_);
        return S_OK;
    }
    STDMETHODIMP GetFormat(AM_MEDIA_TYPE **out) override
    {
        AM_MEDIA_TYPE *mt = (AM_MEDIA_TYPE *)CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE));
        if (!mt)
            return E_OUTOFMEMORY;
        EnterCriticalSection(&lock_);
        copy_type(*mt, peer_ ? current_ : preferred_);
        LeaveCriticalSection(&lock_);
        *out = mt;
        return S_OK;
    }
    STDMETHODIMP GetNumberOfCapabilities(int *count, int *size) override
    {
        *count = 2;
        *size = sizeof(VIDEO_STREAM_CONFIG_CAPS);
        return S_OK;
    }
    STDMETHODIMP GetStreamCaps(int index, AM_MEDIA_TYPE **out, BYTE *caps) override
    {
        if (index < 0 || index > 1)
            return S_FALSE;
        AM_MEDIA_TYPE *mt = (AM_MEDIA_TYPE *)CoTaskMemAlloc(sizeof(AM_MEDIA_TYPE));
        if (!mt)
            return E_OUTOFMEMORY;
        make_type(index, *mt);
        *out = mt;
        VIDEO_STREAM_CONFIG_CAPS *c = (VIDEO_STREAM_CONFIG_CAPS *)caps;
        memset(c, 0, sizeof *c);
        c->guid = FORMAT_VideoInfo;
        SIZE s = { SIZES[index][0], SIZES[index][1] };
        c->InputSize = c->MinCroppingSize = c->MaxCroppingSize = c->MinOutputSize = c->MaxOutputSize = s;
        c->MinFrameInterval = c->MaxFrameInterval = FRAME_100NS;
        c->MinBitsPerSecond = c->MaxBitsPerSecond = s.cx * s.cy * 2 * 8 * 30;
        return S_OK;
    }

    // IKsPropertySet : la broche est une broche de capture.
    STDMETHODIMP Set(REFGUID, DWORD, LPVOID, DWORD, LPVOID, DWORD) override { return E_NOTIMPL; }
    STDMETHODIMP Get(REFGUID set, DWORD id, LPVOID, DWORD, LPVOID data, DWORD size, DWORD *returned) override
    {
        if (set != AMPROPSETID_Pin)
            return E_PROP_SET_UNSUPPORTED;
        if (id != AMPROPERTY_PIN_CATEGORY)
            return E_PROP_ID_UNSUPPORTED;
        if (returned)
            *returned = sizeof(GUID);
        if (!data)
            return S_OK;
        if (size < sizeof(GUID))
            return E_UNEXPECTED;
        *(GUID *)data = PIN_CATEGORY_CAPTURE;
        return S_OK;
    }
    STDMETHODIMP QuerySupported(REFGUID set, DWORD id, DWORD *support) override
    {
        if (set != AMPROPSETID_Pin)
            return E_PROP_SET_UNSUPPORTED;
        if (id != AMPROPERTY_PIN_CATEGORY)
            return E_PROP_ID_UNSUPPORTED;
        if (support)
            *support = KSPROPERTY_SUPPORT_GET;
        return S_OK;
    }

    // Du filtre.
    HRESULT Commit()
    {
        EnterCriticalSection(&lock_);
        HRESULT hr = allocator_ ? allocator_->Commit() : S_OK;
        LeaveCriticalSection(&lock_);
        return hr;
    }
    void Decommit()
    {
        EnterCriticalSection(&lock_);
        if (allocator_)
            allocator_->Decommit();
        LeaveCriticalSection(&lock_);
    }
    // Une image à `start` (temps du flux) : 0 si partie.
    HRESULT Deliver(REFERENCE_TIME start)
    {
        EnterCriticalSection(&lock_);
        IMemAllocator *alloc = allocator_;
        IMemInputPin *input = input_;
        int index = type_index(&current_);
        if (alloc)
            alloc->AddRef();
        if (input)
            input->AddRef();
        LeaveCriticalSection(&lock_);
        HRESULT hr = S_FALSE;
        if (alloc && input && index >= 0) {
            int w = SIZES[index][0], h = SIZES[index][1];
            IMediaSample *sample = NULL;
            hr = alloc->GetBuffer(&sample, NULL, NULL, 0);
            if (SUCCEEDED(hr)) {
                BYTE *data = NULL;
                long size = (long)(w * h * 2);
                if (!nv12_)
                    nv12_ = (uint8_t *)malloc(KN_SHARED_FRAME_MAX);
                if (!nv12_) {
                    hr = E_OUTOFMEMORY;
                } else if (SUCCEEDED(sample->GetPointer(&data)) && sample->GetSize() >= size) {
                    kn_shared_read(&mapping_, &view_, nv12_, w, h);
                    nv12_to_yuy2(nv12_, data, w, h);
                    sample->SetActualDataLength(size);
                    REFERENCE_TIME end = start + FRAME_100NS;
                    sample->SetTime(&start, &end);
                    sample->SetSyncPoint(TRUE);
                    hr = input->Receive(sample);
                }
                sample->Release();
            }
        }
        if (alloc)
            alloc->Release();
        if (input)
            input->Release();
        return hr;
    }

private:
    HRESULT attach(IPin *receiver, const AM_MEDIA_TYPE &mt)
    {
        IMemInputPin *input = NULL;
        HRESULT hr = receiver->QueryInterface(IID_IMemInputPin, (void **)&input);
        if (FAILED(hr))
            return hr;
        int index = type_index(&mt);
        ALLOCATOR_PROPERTIES want = {}, got = {};
        input->GetAllocatorRequirements(&want);
        want.cBuffers = want.cBuffers > 2 ? want.cBuffers : 2;
        want.cbBuffer = SIZES[index][0] * SIZES[index][1] * 2;
        want.cbAlign = want.cbAlign > 0 ? want.cbAlign : 1;
        IMemAllocator *alloc = NULL;
        hr = input->GetAllocator(&alloc);
        if (FAILED(hr) || !alloc)
            hr = CoCreateInstance(CLSID_MemoryAllocator, NULL, CLSCTX_INPROC_SERVER, IID_IMemAllocator, (void **)&alloc);
        if (SUCCEEDED(hr))
            hr = alloc->SetProperties(&want, &got);
        if (SUCCEEDED(hr) && got.cbBuffer < want.cbBuffer)
            hr = E_FAIL;
        if (SUCCEEDED(hr))
            hr = input->NotifyAllocator(alloc, FALSE);
        if (FAILED(hr)) {
            if (alloc)
                alloc->Release();
            input->Release();
            return hr;
        }
        peer_ = receiver;
        peer_->AddRef();
        input_ = input;
        allocator_ = alloc;
        copy_type(current_, mt);
        return S_OK;
    }

    Filter *filter_;
    CRITICAL_SECTION lock_;
    IPin *peer_ = NULL;
    IMemInputPin *input_ = NULL;
    IMemAllocator *allocator_ = NULL;
    AM_MEDIA_TYPE current_;
    AM_MEDIA_TYPE preferred_;
    HANDLE mapping_ = NULL;
    struct kn_shared *view_ = NULL;
    uint8_t *nv12_ = NULL;
};

class EnumPins final : public IEnumPins {
public:
    EnumPins(IPin *pin, ULONG pos) : ref_(1), pin_(pin), pos_(pos) { pin_->AddRef(); }
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (iid == IID_IUnknown || iid == IID_IEnumPins) {
            *ppv = this;
            AddRef();
            return S_OK;
        }
        *ppv = NULL;
        return E_NOINTERFACE;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&ref_); }
    STDMETHODIMP_(ULONG) Release() override
    {
        LONG r = InterlockedDecrement(&ref_);
        if (!r) {
            pin_->Release();
            delete this;
        }
        return r;
    }
    STDMETHODIMP Next(ULONG n, IPin **pins, ULONG *fetched) override
    {
        ULONG got = 0;
        if (n > 0 && pos_ == 0) {
            pins[0] = pin_;
            pin_->AddRef();
            pos_ = 1;
            got = 1;
        }
        if (fetched)
            *fetched = got;
        return got == n ? S_OK : S_FALSE;
    }
    STDMETHODIMP Skip(ULONG n) override
    {
        pos_ += n;
        return pos_ <= 1 ? S_OK : S_FALSE;
    }
    STDMETHODIMP Reset() override
    {
        pos_ = 0;
        return S_OK;
    }
    STDMETHODIMP Clone(IEnumPins **out) override
    {
        *out = new (std::nothrow) EnumPins(pin_, pos_);
        return *out ? S_OK : E_OUTOFMEMORY;
    }

private:
    LONG ref_;
    IPin *pin_;
    ULONG pos_;
};

class Filter final : public IBaseFilter {
public:
    Filter() : ref_(1), pin_(this)
    {
        InitializeCriticalSection(&lock_);
        stop_ = CreateEventW(NULL, TRUE, FALSE, NULL);
        InterlockedIncrement(&g_objects);
    }

    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (!ppv)
            return E_POINTER;
        if (iid == IID_IUnknown || iid == IID_IPersist || iid == IID_IMediaFilter || iid == IID_IBaseFilter)
            *ppv = static_cast<IBaseFilter *>(this);
        else if (iid == IID_IAMStreamConfig)
            *ppv = static_cast<IAMStreamConfig *>(&pin_);
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

    // IPersist
    STDMETHODIMP GetClassID(CLSID *clsid) override
    {
        *clsid = FILTER_CLSID;
        return S_OK;
    }

    // IMediaFilter
    STDMETHODIMP Stop() override
    {
        // L'allocateur d'abord : un fil bloqué dans GetBuffer en sort tout de suite.
        pin_.Decommit();
        stop_thread(5000);
        EnterCriticalSection(&lock_);
        state_ = State_Stopped;
        LeaveCriticalSection(&lock_);
        return S_OK;
    }
    STDMETHODIMP Pause() override
    {
        EnterCriticalSection(&lock_);
        FILTER_STATE was = state_;
        state_ = State_Paused;
        LeaveCriticalSection(&lock_);
        if (was == State_Stopped)
            pin_.Commit();
        if (was == State_Running)
            stop_thread(5000);
        return S_OK;
    }
    STDMETHODIMP Run(REFERENCE_TIME start) override
    {
        EnterCriticalSection(&lock_);
        FILTER_STATE was = state_;
        start_ = start;
        state_ = State_Running;
        LeaveCriticalSection(&lock_);
        if (was == State_Stopped)
            pin_.Commit();
        start_thread();
        return S_OK;
    }
    STDMETHODIMP GetState(DWORD, FILTER_STATE *state) override
    {
        EnterCriticalSection(&lock_);
        *state = state_;
        LeaveCriticalSection(&lock_);
        // Une source en direct n'a rien à montrer en pause.
        return *state == State_Paused ? VFW_S_CANT_CUE : S_OK;
    }
    STDMETHODIMP SetSyncSource(IReferenceClock *clock) override
    {
        EnterCriticalSection(&lock_);
        if (clock_)
            clock_->Release();
        clock_ = clock;
        if (clock_)
            clock_->AddRef();
        LeaveCriticalSection(&lock_);
        return S_OK;
    }
    STDMETHODIMP GetSyncSource(IReferenceClock **clock) override
    {
        EnterCriticalSection(&lock_);
        *clock = clock_;
        if (clock_)
            clock_->AddRef();
        LeaveCriticalSection(&lock_);
        return S_OK;
    }

    // IBaseFilter
    STDMETHODIMP EnumPins(IEnumPins **out) override
    {
        *out = new (std::nothrow) ::EnumPins(static_cast<IPin *>(&pin_), 0);
        return *out ? S_OK : E_OUTOFMEMORY;
    }
    STDMETHODIMP FindPin(LPCWSTR id, IPin **out) override
    {
        if (wcscmp(id, PIN_NAME) != 0) {
            *out = NULL;
            return VFW_E_NOT_FOUND;
        }
        *out = &pin_;
        pin_.AddRef();
        return S_OK;
    }
    STDMETHODIMP QueryFilterInfo(FILTER_INFO *info) override
    {
        wcsncpy(info->achName, name_[0] ? name_ : FILTER_NAME, MAX_FILTER_NAME - 1);
        info->achName[MAX_FILTER_NAME - 1] = 0;
        info->pGraph = graph_;
        if (graph_)
            graph_->AddRef();
        return S_OK;
    }
    STDMETHODIMP JoinFilterGraph(IFilterGraph *graph, LPCWSTR name) override
    {
        // Le graphe nous tient : pas de référence en retour.
        graph_ = graph;
        if (name) {
            wcsncpy(name_, name, MAX_FILTER_NAME - 1);
            name_[MAX_FILTER_NAME - 1] = 0;
        } else {
            name_[0] = 0;
        }
        return S_OK;
    }
    STDMETHODIMP QueryVendorInfo(LPWSTR *) override { return E_NOTIMPL; }

    void pin_info(PIN_INFO *info)
    {
        info->pFilter = this;
        AddRef();
        info->dir = PINDIR_OUTPUT;
        wcsncpy(info->achName, PIN_NAME, MAX_PIN_NAME - 1);
        info->achName[MAX_PIN_NAME - 1] = 0;
    }

private:
    ~Filter()
    {
        // Jamais détruit sous un fil qui tourne encore.
        pin_.Decommit();
        stop_thread(INFINITE);
        if (clock_)
            clock_->Release();
        CloseHandle(stop_);
        DeleteCriticalSection(&lock_);
        InterlockedDecrement(&g_objects);
    }

    void start_thread()
    {
        // Un fil gardé après un arrêt trop lent, qui a fini depuis : refermé.
        if (thread_ && WaitForSingleObject(thread_, 0) == WAIT_OBJECT_0) {
            CloseHandle(thread_);
            thread_ = NULL;
        }
        if (thread_)
            return;
        ResetEvent(stop_);
        thread_ = CreateThread(NULL, 0, &Filter::thread_main, this, 0, NULL);
    }
    // Arrête le fil ; s'il ne s'arrête pas à temps, il est gardé (attendu
    // de nouveau, sans limite, avant la destruction du filtre).
    bool stop_thread(DWORD timeout)
    {
        if (!thread_)
            return true;
        SetEvent(stop_);
        if (WaitForSingleObject(thread_, timeout) != WAIT_OBJECT_0)
            return false;
        CloseHandle(thread_);
        thread_ = NULL;
        return true;
    }
    static DWORD WINAPI thread_main(LPVOID self)
    {
        static_cast<Filter *>(self)->run();
        return 0;
    }
    // Une image toutes les 1/30 s, horodatée sur l'horloge du graphe.
    void run()
    {
        CoInitializeEx(NULL, COINIT_MULTITHREADED);
        REFERENCE_TIME next = -1;
        for (;;) {
            REFERENCE_TIME now = 0, start;
            EnterCriticalSection(&lock_);
            IReferenceClock *clock = clock_;
            if (clock)
                clock->AddRef();
            start = start_;
            LeaveCriticalSection(&lock_);
            if (clock) {
                clock->GetTime(&now);
                clock->Release();
                now -= start;
            } else {
                now = (REFERENCE_TIME)GetTickCount64() * 10000;
            }
            if (next < 0 || now - next > 3 * FRAME_100NS)
                next = now > 0 ? now : 0;
            if (pin_.Deliver(next) == S_FALSE) {
                // Pas encore reliée : on attend.
            }
            next += FRAME_100NS;
            DWORD wait = 0;
            if (clock && next > now)
                wait = (DWORD)((next - now) / 10000);
            else if (!clock)
                wait = 33;
            if (WaitForSingleObject(stop_, wait) == WAIT_OBJECT_0)
                break;
        }
        CoUninitialize();
    }

    LONG ref_;
    CRITICAL_SECTION lock_;
    Pin pin_;
    FILTER_STATE state_ = State_Stopped;
    IReferenceClock *clock_ = NULL;
    IFilterGraph *graph_ = NULL;
    REFERENCE_TIME start_ = 0;
    HANDLE thread_ = NULL;
    HANDLE stop_;
    wchar_t name_[MAX_FILTER_NAME] = { 0 };

    friend class Pin;
};

STDMETHODIMP Pin::QueryInterface(REFIID iid, void **ppv)
{
    if (!ppv)
        return E_POINTER;
    if (iid == IID_IUnknown || iid == IID_IPin)
        *ppv = static_cast<IPin *>(this);
    else if (iid == IID_IAMStreamConfig)
        *ppv = static_cast<IAMStreamConfig *>(this);
    else if (iid == IID_IKsPropertySet)
        *ppv = static_cast<IKsPropertySet *>(this);
    else {
        *ppv = NULL;
        return E_NOINTERFACE;
    }
    AddRef();
    return S_OK;
}
STDMETHODIMP_(ULONG) Pin::AddRef() { return filter_->AddRef(); }
STDMETHODIMP_(ULONG) Pin::Release() { return filter_->Release(); }
STDMETHODIMP Pin::QueryPinInfo(PIN_INFO *info)
{
    filter_->pin_info(info);
    return S_OK;
}

class Factory final : public IClassFactory {
public:
    STDMETHODIMP QueryInterface(REFIID iid, void **ppv) override
    {
        if (iid == IID_IUnknown || iid == IID_IClassFactory) {
            *ppv = static_cast<IClassFactory *>(this);
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
        Filter *f = new (std::nothrow) Filter();
        if (!f)
            return E_OUTOFMEMORY;
        HRESULT hr = f->QueryInterface(iid, ppv);
        f->Release();
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

HRESULT set_string(const wchar_t *path, const wchar_t *name, const wchar_t *value)
{
    HKEY key;
    LONG r = RegCreateKeyExW(HKEY_LOCAL_MACHINE, path, 0, NULL, 0, KEY_WRITE, NULL, &key, NULL);
    if (r != ERROR_SUCCESS)
        return HRESULT_FROM_WIN32(r);
    r = RegSetValueExW(key, name, 0, REG_SZ, (const BYTE *)value, (DWORD)((wcslen(value) + 1) * sizeof(wchar_t)));
    RegCloseKey(key);
    return HRESULT_FROM_WIN32(r);
}

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
    if (clsid != FILTER_CLSID)
        return CLASS_E_CLASSNOTAVAILABLE;
    return g_factory.QueryInterface(iid, ppv);
}

extern "C" HRESULT STDAPICALLTYPE DllCanUnloadNow()
{
    return g_objects == 0 && g_locks == 0 ? S_OK : S_FALSE;
}

extern "C" HRESULT STDAPICALLTYPE DllRegisterServer()
{
    wchar_t dll[MAX_PATH];
    if (!GetModuleFileNameW(g_module, dll, MAX_PATH))
        return E_FAIL;
    wchar_t key[160];
    swprintf(key, 160, L"SOFTWARE\\Classes\\CLSID\\%ls", KN_FILTER_CLSID_STR);
    HRESULT hr = set_string(key, NULL, FILTER_NAME);
    wcscat(key, L"\\InprocServer32");
    if (SUCCEEDED(hr))
        hr = set_string(key, NULL, dll);
    if (SUCCEEDED(hr))
        hr = set_string(key, L"ThreadingModel", L"Both");
    if (FAILED(hr))
        return hr;
    HRESULT co = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);
    IFilterMapper2 *mapper = NULL;
    hr = CoCreateInstance(CLSID_FilterMapper2, NULL, CLSCTX_INPROC_SERVER, IID_IFilterMapper2, (void **)&mapper);
    if (SUCCEEDED(hr)) {
        REGPINTYPES type = { &MEDIATYPE_Video, &MEDIASUBTYPE_YUY2 };
        REGFILTERPINS2 pin = {};
        pin.dwFlags = REG_PINFLAG_B_OUTPUT;
        pin.cInstances = 1;
        pin.nMediaTypes = 1;
        pin.lpMediaType = &type;
        pin.clsPinCategory = &PIN_CATEGORY_CAPTURE;
        REGFILTER2 filter = {};
        filter.dwVersion = 2;
        filter.dwMerit = MERIT_DO_NOT_USE;
        filter.cPins2 = 1;
        filter.rgPins2 = &pin;
        hr = mapper->RegisterFilter(FILTER_CLSID, FILTER_NAME, NULL, &CLSID_VideoInputDeviceCategory, FILTER_NAME, &filter);
        mapper->Release();
    }
    if (SUCCEEDED(co))
        CoUninitialize();
    return hr;
}

extern "C" HRESULT STDAPICALLTYPE DllUnregisterServer()
{
    HRESULT co = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);
    IFilterMapper2 *mapper = NULL;
    if (SUCCEEDED(CoCreateInstance(CLSID_FilterMapper2, NULL, CLSCTX_INPROC_SERVER, IID_IFilterMapper2, (void **)&mapper))) {
        mapper->UnregisterFilter(&CLSID_VideoInputDeviceCategory, FILTER_NAME, FILTER_CLSID);
        mapper->Release();
    }
    if (SUCCEEDED(co))
        CoUninitialize();
    wchar_t key[160];
    swprintf(key, 160, L"SOFTWARE\\Classes\\CLSID\\%ls", KN_FILTER_CLSID_STR);
    RegDeleteTreeW(HKEY_LOCAL_MACHINE, key);
    return S_OK;
}
