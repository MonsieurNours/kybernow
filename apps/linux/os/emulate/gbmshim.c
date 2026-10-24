/* Emulation only (QEMU virtio-gpu accepts XRGB8888 scanout only): map ARGB8888 → XRGB8888 in GBM. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdint.h>
#include <stddef.h>
#define ARGB 0x34325241u
#define XRGB 0x34325258u
struct gbm_device; struct gbm_surface; struct gbm_bo;
static uint32_t fix(uint32_t f) { return f == ARGB ? XRGB : f; }
struct gbm_surface *gbm_surface_create(struct gbm_device *d, uint32_t w, uint32_t h, uint32_t f, uint32_t fl) {
    static struct gbm_surface *(*real)(struct gbm_device *, uint32_t, uint32_t, uint32_t, uint32_t);
    if (!real) real = dlsym(RTLD_NEXT, "gbm_surface_create");
    return real(d, w, h, fix(f), fl);
}
struct gbm_surface *gbm_surface_create_with_modifiers(struct gbm_device *d, uint32_t w, uint32_t h, uint32_t f, const uint64_t *m, unsigned n) {
    static struct gbm_surface *(*real)(struct gbm_device *, uint32_t, uint32_t, uint32_t, const uint64_t *, unsigned);
    if (!real) real = dlsym(RTLD_NEXT, "gbm_surface_create_with_modifiers");
    return real(d, w, h, fix(f), m, n);
}
struct gbm_bo *gbm_bo_create(struct gbm_device *d, uint32_t w, uint32_t h, uint32_t f, uint32_t fl) {
    static struct gbm_bo *(*real)(struct gbm_device *, uint32_t, uint32_t, uint32_t, uint32_t);
    if (!real) real = dlsym(RTLD_NEXT, "gbm_bo_create");
    return real(d, w, h, fix(f), fl);
}
int gbm_device_is_format_supported(struct gbm_device *d, uint32_t f, uint32_t fl) {
    static int (*real)(struct gbm_device *, uint32_t, uint32_t);
    if (!real) real = dlsym(RTLD_NEXT, "gbm_device_is_format_supported");
    return real(d, fix(f), fl);
}
