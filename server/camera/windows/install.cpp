// kybernow-camera --install / --uninstall, et l'état de la caméra (Station
// Windows, docs/webcam.md). Lancé par l'installateur de la Station (en
// administrateur) :
// - Windows 11 (build 22000 et après) : la source de KyberNowCameraSource.dll
//   inscrite (COM, HKLM), puis la caméra créée par MFCreateVirtualCamera,
//   durée de vie « système » et pour tous les utilisateurs : Windows la garde
//   d'un démarrage à l'autre, Frame Server charge la source quand une app
//   filme. Retirée par Remove() à la désinstallation.
// - Windows 10 : le filtre DirectShow KyberNowCameraFilter.dll inscrit
//   (DllRegisterServer), dans la catégorie des caméras de DirectShow.
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <stdio.h>

#include "kn_shared.h"

extern "C" void kn_log(const char *fmt, ...);

namespace {

// mfvirtualcamera.h (SDK 10.0.22000), absent de MinGW.
enum KnVirtualCameraType { KnSoftwareCameraSource = 0 };
enum KnVirtualCameraLifetime { KnLifetimeSession = 0, KnLifetimeSystem = 1 };
enum KnVirtualCameraAccess { KnAccessCurrentUser = 0, KnAccessAllUsers = 1 };

struct KnVirtualCamera : public IMFAttributes {
    virtual HRESULT STDMETHODCALLTYPE AddDeviceSourceInfo(LPCWSTR info) = 0;
    virtual HRESULT STDMETHODCALLTYPE AddProperty(const void *key, ULONG type, const BYTE *data, ULONG size) = 0;
    virtual HRESULT STDMETHODCALLTYPE AddRegistryEntry(LPCWSTR name, LPCWSTR path, DWORD type, const BYTE *data, ULONG size) = 0;
    virtual HRESULT STDMETHODCALLTYPE Start(IMFAsyncCallback *callback) = 0;
    virtual HRESULT STDMETHODCALLTYPE Stop() = 0;
    virtual HRESULT STDMETHODCALLTYPE Remove() = 0;
    virtual HRESULT STDMETHODCALLTYPE GetMediaSource(IMFMediaSource **source) = 0;
    // SendCameraProperty, CreateSyncEvent, CreateSyncSemaphore, Shutdown : pas appelées.
};

typedef HRESULT(WINAPI *CreateVirtualCameraFn)(KnVirtualCameraType, KnVirtualCameraLifetime, KnVirtualCameraAccess,
                                               LPCWSTR, LPCWSTR, const GUID *, ULONG, KnVirtualCamera **);

const GUID KSCATEGORY_VIDEO_CAMERA_ = { 0xe5323777, 0xf976, 0x4f5b, { 0x9b, 0x55, 0xb9, 0x46, 0x99, 0xc4, 0x6e, 0x44 } };
const wchar_t *MARKER_KEY = L"SOFTWARE\\KyberNow\\Camera";

DWORD windows_build()
{
    typedef LONG(WINAPI * RtlGetVersionFn)(OSVERSIONINFOW *);
    OSVERSIONINFOW v = { sizeof v };
    auto fn = (RtlGetVersionFn)GetProcAddress(GetModuleHandleW(L"ntdll.dll"), "RtlGetVersion");
    if (fn && fn(&v) == 0)
        return v.dwBuildNumber;
    return 0;
}

bool windows11() { return windows_build() >= 22000; }

// Le chemin d'une DLL à côté de cet exécutable.
bool beside(const wchar_t *name, wchar_t *path, DWORD size)
{
    DWORD n = GetModuleFileNameW(NULL, path, size);
    if (!n || n >= size)
        return false;
    wchar_t *slash = wcsrchr(path, L'\\');
    if (!slash)
        return false;
    slash[1] = 0;
    if (wcslen(path) + wcslen(name) + 1 > size)
        return false;
    wcscat(path, name);
    return GetFileAttributesW(path) != INVALID_FILE_ATTRIBUTES;
}

bool set_string(HKEY root, const wchar_t *path, const wchar_t *name, const wchar_t *value)
{
    HKEY key;
    if (RegCreateKeyExW(root, path, 0, NULL, 0, KEY_WRITE, NULL, &key, NULL) != ERROR_SUCCESS)
        return false;
    LONG r = RegSetValueExW(key, name, 0, REG_SZ, (const BYTE *)value, (DWORD)((wcslen(value) + 1) * sizeof(wchar_t)));
    RegCloseKey(key);
    return r == ERROR_SUCCESS;
}

bool key_exists(const wchar_t *path)
{
    HKEY key;
    if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, path, 0, KEY_READ, &key) != ERROR_SUCCESS)
        return false;
    RegCloseKey(key);
    return true;
}

HRESULT virtual_camera(KnVirtualCamera **camera)
{
    HMODULE lib = LoadLibraryW(L"mfsensorgroup.dll");
    if (!lib)
        return HRESULT_FROM_WIN32(GetLastError());
    auto create = (CreateVirtualCameraFn)GetProcAddress(lib, "MFCreateVirtualCamera");
    if (!create)
        return E_NOTIMPL;
    return create(KnSoftwareCameraSource, KnLifetimeSystem, KnAccessAllUsers, KN_CAMERA_NAME_W,
                  KN_SOURCE_CLSID_STR, &KSCATEGORY_VIDEO_CAMERA_, 1, camera);
}

int install_windows11(bool install)
{
    wchar_t clsid_key[128];
    swprintf(clsid_key, 128, L"SOFTWARE\\Classes\\CLSID\\%ls", KN_SOURCE_CLSID_STR);
    wchar_t server_key[160];
    swprintf(server_key, 160, L"%ls\\InprocServer32", clsid_key);
    if (install) {
        wchar_t dll[MAX_PATH];
        if (!beside(L"KyberNowCameraSource.dll", dll, MAX_PATH)) {
            kn_log("KyberNowCameraSource.dll not found next to kybernow-camera.exe");
            return 1;
        }
        if (!set_string(HKEY_LOCAL_MACHINE, clsid_key, NULL, KN_CAMERA_NAME_W)
            || !set_string(HKEY_LOCAL_MACHINE, server_key, NULL, dll)
            || !set_string(HKEY_LOCAL_MACHINE, server_key, L"ThreadingModel", L"Both")) {
            kn_log("media source not registered (run as administrator)");
            return 1;
        }
    }
    // MFPlat chargée à la demande : un Windows N sans le Media Feature Pack
    // n'en a pas, et l'outil doit s'y lancer quand même (pour le dire).
    HMODULE mfplat = LoadLibraryW(L"mfplat.dll");
    typedef HRESULT(WINAPI * StartupFn)(ULONG, DWORD);
    typedef HRESULT(WINAPI * ShutdownFn)();
    auto startup = mfplat ? (StartupFn)GetProcAddress(mfplat, "MFStartup") : NULL;
    auto shutdown = mfplat ? (ShutdownFn)GetProcAddress(mfplat, "MFShutdown") : NULL;
    if (!startup || !shutdown || FAILED(startup(MF_VERSION, MFSTARTUP_LITE))) {
        kn_log("Media Foundation unavailable (Windows N without the Media Feature Pack?)");
        return 1;
    }
    KnVirtualCamera *camera = NULL;
    HRESULT hr = virtual_camera(&camera);
    if (SUCCEEDED(hr)) {
        hr = install ? camera->Start(NULL) : camera->Remove();
        camera->Release();
    }
    shutdown();
    if (!install) {
        RegDeleteTreeW(HKEY_LOCAL_MACHINE, clsid_key);
        RegDeleteTreeW(HKEY_LOCAL_MACHINE, MARKER_KEY);
    }
    if (FAILED(hr)) {
        kn_log("virtual camera %s failed (0x%08lx)", install ? "creation" : "removal", (unsigned long)hr);
        return install ? 1 : 0;
    }
    if (install)
        set_string(HKEY_LOCAL_MACHINE, MARKER_KEY, L"Kind", L"mf");
    kn_log("virtual camera %s (Windows 11)", install ? "created" : "removed");
    return 0;
}

int install_windows10(bool install)
{
    wchar_t dll[MAX_PATH];
    if (!beside(L"KyberNowCameraFilter.dll", dll, MAX_PATH)) {
        kn_log("KyberNowCameraFilter.dll not found next to kybernow-camera.exe");
        return install ? 1 : 0;
    }
    HMODULE lib = LoadLibraryW(dll);
    if (!lib)
        return 1;
    typedef HRESULT(STDAPICALLTYPE * RegFn)();
    auto fn = (RegFn)GetProcAddress(lib, install ? "DllRegisterServer" : "DllUnregisterServer");
    HRESULT hr = fn ? fn() : E_NOTIMPL;
    FreeLibrary(lib);
    if (FAILED(hr)) {
        kn_log("DirectShow filter %s failed (0x%08lx)", install ? "registration" : "removal", (unsigned long)hr);
        return 1;
    }
    if (install)
        set_string(HKEY_LOCAL_MACHINE, MARKER_KEY, L"Kind", L"dshow");
    else
        RegDeleteTreeW(HKEY_LOCAL_MACHINE, MARKER_KEY);
    kn_log("DirectShow filter %s (Windows 10)", install ? "registered" : "removed");
    return 0;
}

} // namespace

// Windows 11 (build 22000 et après) : la mémoire partagée n'est lue que par Frame Server.
extern "C" int kn_windows11(void)
{
    return windows11() ? 1 : 0;
}

extern "C" int kn_windows_install(int install)
{
    return windows11() ? install_windows11(install != 0) : install_windows10(install != 0);
}

extern "C" int kn_windows_installed(const char **reason)
{
    wchar_t key[160];
    if (windows11()) {
        swprintf(key, 160, L"SOFTWARE\\Classes\\CLSID\\%ls\\InprocServer32", KN_SOURCE_CLSID_STR);
        if (key_exists(key) && key_exists(MARKER_KEY))
            return 0;
        *reason = "virtual camera not installed (kybernow-camera --install)";
        return 1;
    }
    swprintf(key, 160, L"SOFTWARE\\Classes\\CLSID\\%ls\\InprocServer32", KN_FILTER_CLSID_STR);
    if (key_exists(key))
        return 0;
    *reason = "DirectShow filter not registered (kybernow-camera --install)";
    return 1;
}
