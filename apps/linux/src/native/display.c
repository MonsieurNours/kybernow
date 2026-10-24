// Ce que disent les écrans branchés, par DRM (libdrm, lecture seule, sans
// être maître de l'affichage) : la propriété « vrr_capable » de chaque
// connecteur branché, celle qu'écrit le pilote d'après l'EDID (FreeSync,
// Adaptive-Sync, HDMI VRR). Le compositeur (gamescope, KWin, Mutter) décide
// d'activer la fréquence variable ; l'app, elle, montre chaque image dès
// qu'elle est décodée quand l'écran la permet.

#include <stdio.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>

#include <xf86drm.h>
#include <xf86drmMode.h>

static const char *connector_type(uint32_t t) {
    switch (t) {
    case DRM_MODE_CONNECTOR_DisplayPort: return "DP";
    case DRM_MODE_CONNECTOR_HDMIA: return "HDMI-A";
    case DRM_MODE_CONNECTOR_HDMIB: return "HDMI-B";
    case DRM_MODE_CONNECTOR_eDP: return "eDP";
    case DRM_MODE_CONNECTOR_DSI: return "DSI";
    case DRM_MODE_CONNECTOR_VIRTUAL: return "Virtual";
    case DRM_MODE_CONNECTOR_DVID: return "DVI-D";
    default: return "Unknown";
    }
}

// Les modes qu'annonce le premier écran branché : la fréquence la plus haute
// en 1920 × 1080 (`hz_1080p`) et le plus grand mode (`w`, `h`, `hz` : le plus
// de pixels, puis la fréquence la plus haute). Rend le nombre de modes lus,
// 0 si aucun écran, -1 si DRM n'est pas lisible.
int kn_modes_probe(int *hz_1080p, int *w, int *h, int *hz) {
    int seen_card = 0, modes = 0;
    if (hz_1080p) *hz_1080p = 0;
    if (w) *w = 0;
    if (h) *h = 0;
    if (hz) *hz = 0;
    for (int card = 0; card < 8 && !modes; card++) {
        char path[32];
        snprintf(path, sizeof path, "/dev/dri/card%d", card);
        int fd = open(path, O_RDWR | O_CLOEXEC);
        if (fd < 0) fd = open(path, O_RDONLY | O_CLOEXEC);
        if (fd < 0) continue;
        drmModeRes *res = drmModeGetResources(fd);
        if (!res) { close(fd); continue; }
        seen_card = 1;
        for (int i = 0; i < res->count_connectors && !modes; i++) {
            drmModeConnector *c = drmModeGetConnector(fd, res->connectors[i]);
            if (!c) continue;
            if (c->connection == DRM_MODE_CONNECTED) {
                for (int m = 0; m < c->count_modes; m++) {
                    drmModeModeInfo *mi = &c->modes[m];
                    modes++;
                    if (mi->hdisplay == 1920 && mi->vdisplay == 1080 && hz_1080p && (int)mi->vrefresh > *hz_1080p) *hz_1080p = mi->vrefresh;
                    if (w && h && hz) {
                        long px = (long)mi->hdisplay * mi->vdisplay, best = (long)*w * *h;
                        if (px > best || (px == best && (int)mi->vrefresh > *hz)) { *w = mi->hdisplay; *h = mi->vdisplay; *hz = mi->vrefresh; }
                    }
                }
            }
            drmModeFreeConnector(c);
        }
        drmModeFreeResources(res);
        close(fd);
    }
    return seen_card ? modes : -1;
}

// Rend 1 si un écran branché annonce le VRR (son connecteur dans `name`),
// 0 si aucun, -1 si DRM n'est pas lisible (pas de /dev/dri, machine virtuelle).
// `connected` : nombre d'écrans branchés vus.
int kn_vrr_probe(char *name, int len, int *connected) {
    int seen_card = 0, result = 0;
    if (connected) *connected = 0;
    for (int card = 0; card < 8; card++) {
        char path[32];
        snprintf(path, sizeof path, "/dev/dri/card%d", card);
        int fd = open(path, O_RDWR | O_CLOEXEC);
        if (fd < 0) fd = open(path, O_RDONLY | O_CLOEXEC);
        if (fd < 0) continue;
        drmModeRes *res = drmModeGetResources(fd);
        if (!res) { close(fd); continue; }
        seen_card = 1;
        for (int i = 0; i < res->count_connectors; i++) {
            drmModeConnector *c = drmModeGetConnector(fd, res->connectors[i]);
            if (!c) continue;
            if (c->connection == DRM_MODE_CONNECTED) {
                if (connected) (*connected)++;
                drmModeObjectProperties *props = drmModeObjectGetProperties(fd, c->connector_id, DRM_MODE_OBJECT_CONNECTOR);
                for (uint32_t p = 0; props && p < props->count_props; p++) {
                    drmModePropertyRes *prop = drmModeGetProperty(fd, props->props[p]);
                    if (!prop) continue;
                    if (strcmp(prop->name, "vrr_capable") == 0 && props->prop_values[p] == 1 && !result) {
                        result = 1;
                        if (name && len > 0) snprintf(name, len, "%s-%u", connector_type(c->connector_type), c->connector_type_id);
                    }
                    drmModeFreeProperty(prop);
                }
                if (props) drmModeFreeObjectProperties(props);
            }
            drmModeFreeConnector(c);
        }
        drmModeFreeResources(res);
        close(fd);
    }
    return seen_card ? result : -1;
}
