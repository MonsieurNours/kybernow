/*
 * kybernow-camera, Station Windows (docs/webcam.md, « Station Windows ») :
 * l'image NV12 dans la mémoire partagée `Global\KyberNowCameraFrame`
 * (windows/kn_shared.h), lue par la caméra de Windows 11 ou le filtre de
 * Windows 10. L'outil tourne en SYSTEM (lancé par kycontroller, comme
 * kyavserver) : il peut créer un objet global.
 */
#include "camera.h"
#include "windows/kn_shared.h"

#include <windows.h>
#include <sddl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* windows/install.cpp */
int kn_windows_install(int install);
int kn_windows_installed(const char **reason);

struct win_out {
    HANDLE mapping;
    struct kn_shared *view;
    int width, height;
};

static int win_open(struct kn_output *out, int w, int h)
{
    struct win_out *o = calloc(1, sizeof *o);
    SECURITY_ATTRIBUTES sa = { sizeof sa, NULL, FALSE };
    PSECURITY_DESCRIPTOR sd = NULL;
    if (ConvertStringSecurityDescriptorToSecurityDescriptorW(KN_SHARED_SDDL, SDDL_REVISION_1, &sd, NULL))
        sa.lpSecurityDescriptor = sd;
    o->mapping = CreateFileMappingW(INVALID_HANDLE_VALUE, sd ? &sa : NULL, PAGE_READWRITE, 0,
                                    (DWORD)sizeof(struct kn_shared), KN_SHARED_NAME);
    if (sd)
        LocalFree(sd);
    if (!o->mapping) {
        kn_log("shared memory not created (%lu)", GetLastError());
        free(o);
        return -1;
    }
    o->view = (struct kn_shared *)MapViewOfFile(o->mapping, FILE_MAP_WRITE, 0, 0, sizeof(struct kn_shared));
    if (!o->view) {
        CloseHandle(o->mapping);
        free(o);
        return -1;
    }
    o->width = w;
    o->height = h;
    o->view->seq = 0;
    o->view->width = (uint32_t)w;
    o->view->height = (uint32_t)h;
    o->view->writer_pid = GetCurrentProcessId();
    o->view->version = KN_SHARED_VERSION;
    MemoryBarrier();
    o->view->magic = KN_SHARED_MAGIC;
    out->priv = o;
    return 0;
}

static int win_write(struct kn_output *out, const uint8_t *data, size_t size, int64_t pts)
{
    struct win_out *o = out->priv;
    if (size > KN_SHARED_FRAME_MAX)
        return -1;
    InterlockedIncrement((volatile LONG *)&o->view->seq); /* impair : en cours */
    MemoryBarrier();
    memcpy(o->view->frame, data, size);
    o->view->width = (uint32_t)o->width;
    o->view->height = (uint32_t)o->height;
    o->view->pts_us = pts;
    o->view->written_ms = GetTickCount64();
    MemoryBarrier();
    InterlockedIncrement((volatile LONG *)&o->view->seq); /* pair : stable */
    return 0;
}

static void win_close(struct kn_output *out)
{
    struct win_out *o = out->priv;
    if (!o)
        return;
    /* Plus d'écrivain : les lecteurs passent au gris tout de suite. */
    o->view->written_ms = 0;
    UnmapViewOfFile(o->view);
    CloseHandle(o->mapping);
    free(o);
    out->priv = NULL;
}

static struct kn_output win_output = {
    .name = "Windows virtual camera",
    .format = KN_NV12,
    .open = win_open,
    .write = win_write,
    .close = win_close,
};

struct kn_output *kn_output_open(int width, int height, const char **reason)
{
    if (kn_windows_installed(reason) != 0)
        return NULL;
    if (win_output.open(&win_output, width, height) != 0) {
        *reason = "shared memory";
        return NULL;
    }
    return &win_output;
}

int kn_output_probe(const char **reason)
{
    return kn_windows_installed(reason);
}

int kn_output_install(int install)
{
    return kn_windows_install(install);
}
