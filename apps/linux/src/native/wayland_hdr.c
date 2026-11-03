// Le HDR sur un bureau Wayland, par la gestion des couleurs du compositeur
// (`wp_color_management_v1` : KDE Plasma 6.3 et après, GNOME 48 et après) :
//
// - `kn_wl_probe` : avant SDL, sur une connexion à part, ce que sait le
//   compositeur (image paramétrique en PQ et primaires BT.2020) et ce que
//   chaque sortie préfère (PQ : un écran en HDR, ses luminosités) ;
// - `kn_hdr_surface_*` : pour une fenêtre SDL sur Wayland, une sous-surface
//   à nous, sous la sienne, où l'image HDR est dessinée par OpenGL ES 2 en
//   10 bits (hdr_gl.c) avec sa description d'image (PQ, BT.2020, luminosités
//   de l'écran de matriçage, MaxCLL et MaxFALL, blanc de référence) ; la
//   fenêtre SDL laisse à cet endroit un trou transparent, et le compositeur
//   compose l'interface SDR par-dessus. Le retour de la surface
//   (`get_surface_feedback` sur la surface de SDL) dit si l'écran où se
//   trouve la fenêtre est HDR, et quand ça change.
//
// Tout se passe sur le fil principal, sur la connexion de SDL, dans une file
// d'événements à nous (SDL lit la connexion ; nous traitons notre file à
// chaque tour). Une erreur de protocole couperait la connexion de SDL :
// chaque requête est d'abord vérifiée d'après ce que le compositeur annonce.
// Le contexte EGL de SDL est remis comme il était après chaque dessin.

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <wayland-client.h>
#include <wayland-egl.h>
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES2/gl2.h>
#include <SDL.h>
#include <SDL_syswm.h>

#include "wayland/color-management-v1-client.h"
#include "wayland/viewporter-client.h"
#include "hdr_gl.h"

#define KN_WL_MAX_OUTPUTS 8

// Une description d'image (`WlDescription` de src/hdr.rs) ; 0 : absente.
typedef struct kn_wl_desc {
    uint32_t tf, min_lum, max_lum, reference_lum, target_min, target_max, max_cll, max_fall;
} kn_wl_desc;

typedef struct kn_wl_output_info {
    int32_t x, y;
    kn_wl_desc desc;
} kn_wl_output_info;

// Les métadonnées du flux, pour la description de l'image (cd/m² ; 0 : absentes).
typedef struct kn_hdr_meta {
    float mastering_min, mastering_max, max_cll, max_fall, sdr_white;
} kn_hdr_meta;

// ---- Ce que le compositeur sait faire -------------------------------------

typedef struct kn_cm_caps {
    uint32_t features, tfs, primaries, intents;
    int done;
} kn_cm_caps;

static void cm_intent(void *d, struct wp_color_manager_v1 *m, uint32_t v) { (void)m; ((kn_cm_caps *)d)->intents |= 1u << (v & 31); }
static void cm_feature(void *d, struct wp_color_manager_v1 *m, uint32_t v) { (void)m; ((kn_cm_caps *)d)->features |= 1u << (v & 31); }
static void cm_tf(void *d, struct wp_color_manager_v1 *m, uint32_t v) { (void)m; ((kn_cm_caps *)d)->tfs |= 1u << (v & 31); }
static void cm_primaries(void *d, struct wp_color_manager_v1 *m, uint32_t v) { (void)m; ((kn_cm_caps *)d)->primaries |= 1u << (v & 31); }
static void cm_done(void *d, struct wp_color_manager_v1 *m) { (void)m; ((kn_cm_caps *)d)->done = 1; }
static const struct wp_color_manager_v1_listener cm_listener = { cm_intent, cm_feature, cm_tf, cm_primaries, cm_done };

// Image paramétrique, PQ et BT.2020 : la voie du HDR.
static int cm_hdr_ok(const kn_cm_caps *c) {
    return (c->features & (1u << WP_COLOR_MANAGER_V1_FEATURE_PARAMETRIC)) && (c->tfs & (1u << WP_COLOR_MANAGER_V1_TRANSFER_FUNCTION_ST2084_PQ)) &&
           (c->primaries & (1u << WP_COLOR_MANAGER_V1_PRIMARIES_BT2020));
}

// ---- Description d'une image (préférée d'une surface, d'une sortie) -------

typedef struct kn_desc_wait {
    kn_wl_desc desc;
    int ready, failed, done;
    // Lire la description (préférée d'une surface, d'une sortie) ; une
    // description créée par nous ne le permet pas (erreur de protocole).
    int want_info;
    struct wp_image_description_info_v1 *info;
} kn_desc_wait;

static void info_done(void *d, struct wp_image_description_info_v1 *i) {
    kn_desc_wait *w = d;
    w->done = 1;
    wp_image_description_info_v1_destroy(i);
    if (w->info == i) w->info = NULL;
}
static void info_icc(void *d, struct wp_image_description_info_v1 *i, int32_t fd, uint32_t size) { (void)d; (void)i; (void)size; close(fd); }
static void info_primaries(void *d, struct wp_image_description_info_v1 *i, int32_t a, int32_t b, int32_t c, int32_t e, int32_t f, int32_t g, int32_t h, int32_t k) {
    (void)d; (void)i; (void)a; (void)b; (void)c; (void)e; (void)f; (void)g; (void)h; (void)k;
}
static void info_primaries_named(void *d, struct wp_image_description_info_v1 *i, uint32_t p) { (void)d; (void)i; (void)p; }
static void info_tf_power(void *d, struct wp_image_description_info_v1 *i, uint32_t e) { (void)d; (void)i; (void)e; }
static void info_tf_named(void *d, struct wp_image_description_info_v1 *i, uint32_t tf) { (void)i; ((kn_desc_wait *)d)->desc.tf = tf; }
static void info_luminances(void *d, struct wp_image_description_info_v1 *i, uint32_t min, uint32_t max, uint32_t ref) {
    (void)i;
    kn_desc_wait *w = d;
    w->desc.min_lum = min; w->desc.max_lum = max; w->desc.reference_lum = ref;
}
static void info_target_luminance(void *d, struct wp_image_description_info_v1 *i, uint32_t min, uint32_t max) {
    (void)i;
    kn_desc_wait *w = d;
    w->desc.target_min = min; w->desc.target_max = max;
}
static void info_cll(void *d, struct wp_image_description_info_v1 *i, uint32_t v) { (void)i; ((kn_desc_wait *)d)->desc.max_cll = v; }
static void info_fall(void *d, struct wp_image_description_info_v1 *i, uint32_t v) { (void)i; ((kn_desc_wait *)d)->desc.max_fall = v; }
static const struct wp_image_description_info_v1_listener info_listener = {
    info_done, info_icc, info_primaries, info_primaries_named, info_tf_power, info_tf_named, info_luminances, info_primaries, info_target_luminance, info_cll, info_fall,
};

static void desc_failed(void *d, struct wp_image_description_v1 *i, uint32_t cause, const char *msg) { (void)i; (void)cause; (void)msg; ((kn_desc_wait *)d)->failed = 1; }
static void desc_ready(void *d, struct wp_image_description_v1 *i, uint32_t identity) {
    (void)identity;
    kn_desc_wait *w = d;
    w->ready = 1;
    if (!w->want_info || w->info) return;
    memset(&w->desc, 0, sizeof w->desc);
    w->done = 0;
    w->info = wp_image_description_v1_get_information(i);
    wp_image_description_info_v1_add_listener(w->info, &info_listener, w);
}
static const struct wp_image_description_v1_listener desc_listener = { desc_failed, desc_ready };

// ---- Sonde, avant SDL ------------------------------------------------------

typedef struct kn_probe_output {
    struct wl_output *output;
    int32_t x, y;
} kn_probe_output;

typedef struct kn_probe {
    struct wl_registry *registry;
    struct wp_color_manager_v1 *cm;
    kn_cm_caps caps;
    kn_probe_output outputs[KN_WL_MAX_OUTPUTS];
    int n_outputs;
} kn_probe;

static void out_geometry(void *d, struct wl_output *o, int32_t x, int32_t y, int32_t pw, int32_t ph, int32_t sub, const char *make, const char *model, int32_t tr) {
    (void)pw; (void)ph; (void)sub; (void)make; (void)model; (void)tr;
    kn_probe *p = d;
    for (int i = 0; i < p->n_outputs; i++) {
        if (p->outputs[i].output == o) { p->outputs[i].x = x; p->outputs[i].y = y; }
    }
}
static void out_mode(void *d, struct wl_output *o, uint32_t f, int32_t w, int32_t h, int32_t r) { (void)d; (void)o; (void)f; (void)w; (void)h; (void)r; }
static void out_done(void *d, struct wl_output *o) { (void)d; (void)o; }
static void out_scale(void *d, struct wl_output *o, int32_t s) { (void)d; (void)o; (void)s; }
static void out_name(void *d, struct wl_output *o, const char *n) { (void)d; (void)o; (void)n; }
static void out_description(void *d, struct wl_output *o, const char *n) { (void)d; (void)o; (void)n; }
static const struct wl_output_listener out_listener = { out_geometry, out_mode, out_done, out_scale, out_name, out_description };

static void probe_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
    kn_probe *p = d;
    if (strcmp(iface, wp_color_manager_v1_interface.name) == 0 && !p->cm) {
        p->cm = wl_registry_bind(r, name, &wp_color_manager_v1_interface, 1);
        wp_color_manager_v1_add_listener(p->cm, &cm_listener, &p->caps);
    } else if (strcmp(iface, wl_output_interface.name) == 0 && p->n_outputs < KN_WL_MAX_OUTPUTS) {
        kn_probe_output *o = &p->outputs[p->n_outputs++];
        o->output = wl_registry_bind(r, name, &wl_output_interface, version < 2 ? version : 2);
        wl_output_add_listener(o->output, &out_listener, p);
    }
}
static void probe_global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener probe_registry_listener = { probe_global, probe_global_remove };

// Rend le nombre de sorties décrites dans `outs` (au plus `max`), -1 sans
// Wayland. `supported` : la gestion des couleurs décrit une image PQ BT.2020.
int kn_wl_probe(int *supported, kn_wl_output_info *outs, int max) {
    *supported = 0;
    if (!getenv("WAYLAND_DISPLAY") && !getenv("WAYLAND_SOCKET")) return -1;
    struct wl_display *dpy = wl_display_connect(NULL);
    if (!dpy) return -1;
    kn_probe p;
    memset(&p, 0, sizeof p);
    p.registry = wl_display_get_registry(dpy);
    wl_registry_add_listener(p.registry, &probe_registry_listener, &p);
    int n = 0;
    if (wl_display_roundtrip(dpy) < 0 || wl_display_roundtrip(dpy) < 0) goto out;
    *supported = p.cm && cm_hdr_ok(&p.caps);
    for (int i = 0; i < p.n_outputs && n < max; i++) {
        kn_wl_output_info *o = &outs[n++];
        memset(o, 0, sizeof *o);
        o->x = p.outputs[i].x;
        o->y = p.outputs[i].y;
        if (!p.cm) continue;
        kn_desc_wait w;
        memset(&w, 0, sizeof w);
        w.want_info = 1;
        struct wp_color_management_output_v1 *cmo = wp_color_manager_v1_get_output(p.cm, p.outputs[i].output);
        struct wp_image_description_v1 *desc = wp_color_management_output_v1_get_image_description(cmo);
        wp_image_description_v1_add_listener(desc, &desc_listener, &w);
        for (int k = 0; k < 4 && !w.failed && !(w.ready && w.done); k++) {
            if (wl_display_roundtrip(dpy) < 0) break;
        }
        if (w.info) wp_image_description_info_v1_destroy(w.info);
        if (w.ready && w.done) o->desc = w.desc;
        wp_image_description_v1_destroy(desc);
        wp_color_management_output_v1_destroy(cmo);
    }
out:
    for (int i = 0; i < p.n_outputs; i++) wl_output_destroy(p.outputs[i].output);
    if (p.cm) wp_color_manager_v1_destroy(p.cm);
    wl_registry_destroy(p.registry);
    wl_display_disconnect(dpy);
    return n;
}

// ---- La sous-surface d'une fenêtre SDL -------------------------------------

typedef struct kn_hdr_surface {
    struct wl_display *dpy;
    struct wl_event_queue *queue;
    struct wl_display *wrapper;
    struct wl_registry *registry;
    struct wl_compositor *compositor;
    struct wl_subcompositor *subcompositor;
    struct wp_viewporter *viewporter;
    struct wp_color_manager_v1 *cm;
    kn_cm_caps caps;
    struct wl_surface *parent, *surface;
    struct wl_subsurface *sub;
    struct wp_viewport *viewport;
    struct wp_color_management_surface_v1 *cms;
    struct wp_color_management_surface_feedback_v1 *feedback;
    // Préférée : la demande en cours, et la dernière complète.
    struct wp_image_description_v1 *pref_obj;
    kn_desc_wait pref;
    kn_wl_desc preferred;
    int have_preferred, changed;
    // Notre description : celle en préparation et celle posée.
    struct wp_image_description_v1 *desc_pending, *desc_set;
    kn_desc_wait desc_wait;
    int desc_key_mode;
    kn_hdr_meta desc_key, pending_key;
    int desc_applied;
    // EGL et rendu.
    EGLDisplay edpy;
    EGLConfig config;
    EGLContext ctx;
    EGLSurface esurf;
    struct wl_egl_window *ewin;
    int ew, eh, bits, egl_failed;
    kn_hdr_gl gl;
    int gl_ok;
    // Où l'image est posée (coordonnées de la fenêtre), et si elle se voit.
    int x, y, w, h, mapped;
    char err[256];
} kn_hdr_surface;

static void s_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
    kn_hdr_surface *s = d;
    if (strcmp(iface, wl_compositor_interface.name) == 0 && !s->compositor) {
        s->compositor = wl_registry_bind(r, name, &wl_compositor_interface, version < 4 ? version : 4);
    } else if (strcmp(iface, wl_subcompositor_interface.name) == 0 && !s->subcompositor) {
        s->subcompositor = wl_registry_bind(r, name, &wl_subcompositor_interface, 1);
    } else if (strcmp(iface, wp_viewporter_interface.name) == 0 && !s->viewporter) {
        s->viewporter = wl_registry_bind(r, name, &wp_viewporter_interface, 1);
    } else if (strcmp(iface, wp_color_manager_v1_interface.name) == 0 && !s->cm) {
        s->cm = wl_registry_bind(r, name, &wp_color_manager_v1_interface, 1);
        wp_color_manager_v1_add_listener(s->cm, &cm_listener, &s->caps);
    }
}
static void s_global_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener s_registry_listener = { s_global, s_global_remove };

static void ask_preferred(kn_hdr_surface *s) {
    if (s->pref.info) { wp_image_description_info_v1_destroy(s->pref.info); s->pref.info = NULL; }
    if (s->pref_obj) wp_image_description_v1_destroy(s->pref_obj);
    memset(&s->pref, 0, sizeof s->pref);
    s->pref.want_info = 1;
    s->pref_obj = wp_color_management_surface_feedback_v1_get_preferred(s->feedback);
    wp_image_description_v1_add_listener(s->pref_obj, &desc_listener, &s->pref);
}

static void fb_changed(void *d, struct wp_color_management_surface_feedback_v1 *f, uint32_t identity) {
    (void)f; (void)identity;
    ask_preferred(d);
}
static const struct wp_color_management_surface_feedback_v1_listener fb_listener = { fb_changed };

// La préférée arrivée au complet : gardée, et signalée si elle a changé.
static void collect_preferred(kn_hdr_surface *s) {
    if (s->pref_obj && s->pref.ready && s->pref.done) {
        if (!s->have_preferred || memcmp(&s->preferred, &s->pref.desc, sizeof s->preferred) != 0) s->changed = 1;
        s->preferred = s->pref.desc;
        s->have_preferred = 1;
        wp_image_description_v1_destroy(s->pref_obj);
        s->pref_obj = NULL;
    } else if (s->pref_obj && s->pref.failed) {
        wp_image_description_v1_destroy(s->pref_obj);
        s->pref_obj = NULL;
    }
}

void kn_hdr_surface_free(kn_hdr_surface *s);

// Une sous-surface pour la fenêtre `window` de SDL (sur Wayland). NULL si la
// fenêtre n'est pas sur Wayland ou que le compositeur ne décrit pas une image
// PQ BT.2020 (`err` dit pourquoi). `require_cm` à 0 (essai, `--hdr-test`) : sans
// gestion des couleurs aussi, l'écran pris pour un écran SDR.
kn_hdr_surface *kn_hdr_surface_new(SDL_Window *window, int require_cm, char *err, int errlen) {
    SDL_SysWMinfo info;
    SDL_VERSION(&info.version);
    if (!SDL_GetWindowWMInfo(window, &info) || info.subsystem != SDL_SYSWM_WAYLAND) {
        snprintf(err, errlen, "fenêtre hors de Wayland");
        return NULL;
    }
    kn_hdr_surface *s = calloc(1, sizeof *s);
    if (!s) return NULL;
    s->dpy = info.info.wl.display;
    s->parent = info.info.wl.surface;
    s->queue = wl_display_create_queue(s->dpy);
    s->wrapper = wl_proxy_create_wrapper(s->dpy);
    wl_proxy_set_queue((struct wl_proxy *)s->wrapper, s->queue);
    s->registry = wl_display_get_registry(s->wrapper);
    wl_registry_add_listener(s->registry, &s_registry_listener, s);
    if (wl_display_roundtrip_queue(s->dpy, s->queue) < 0 || wl_display_roundtrip_queue(s->dpy, s->queue) < 0) {
        snprintf(err, errlen, "connexion Wayland");
        kn_hdr_surface_free(s);
        return NULL;
    }
    if (!require_cm && s->cm && !cm_hdr_ok(&s->caps)) {
        wp_color_manager_v1_destroy(s->cm);
        s->cm = NULL;
    }
    if (!s->compositor || !s->subcompositor || !s->viewporter || (require_cm && (!s->cm || !cm_hdr_ok(&s->caps)))) {
        snprintf(err, errlen, "%s", !s->cm ? "pas de gestion des couleurs" : !cm_hdr_ok(&s->caps) ? "pas d'image PQ BT.2020" : "pas de sous-surface ou de viewporter");
        kn_hdr_surface_free(s);
        return NULL;
    }
    s->surface = wl_compositor_create_surface(s->compositor);
    s->sub = wl_subcompositor_get_subsurface(s->subcompositor, s->surface, s->parent);
    wl_subsurface_place_below(s->sub, s->parent);
    wl_subsurface_set_desync(s->sub);
    // Le pointeur et le clavier restent à la fenêtre de SDL.
    struct wl_region *none = wl_compositor_create_region(s->compositor);
    wl_surface_set_input_region(s->surface, none);
    wl_region_destroy(none);
    s->viewport = wp_viewporter_get_viewport(s->viewporter, s->surface);
    if (s->cm) {
        s->cms = wp_color_manager_v1_get_surface(s->cm, s->surface);
        s->feedback = wp_color_manager_v1_get_surface_feedback(s->cm, s->parent);
        wp_color_management_surface_feedback_v1_add_listener(s->feedback, &fb_listener, s);
        ask_preferred(s);
    }
    for (int k = 0; k < 4 && !s->have_preferred && s->pref_obj; k++) {
        if (wl_display_roundtrip_queue(s->dpy, s->queue) < 0) break;
        collect_preferred(s);
    }
    s->changed = 1;
    s->desc_key_mode = -1;
    return s;
}

// Traite ce qui est arrivé pour nous. 1 : la description préférée de l'écran
// de la fenêtre a changé (copiée dans `out`).
int kn_hdr_surface_poll(kn_hdr_surface *s, kn_wl_desc *out) {
    wl_display_dispatch_queue_pending(s->dpy, s->queue);
    collect_preferred(s);
    if (s->desc_pending && (s->desc_wait.ready || s->desc_wait.failed)) {
        // Une description refusée : l'image reste sans description (sRGB pour le compositeur).
        if (s->desc_wait.ready) {
            if (s->desc_set) wp_image_description_v1_destroy(s->desc_set);
            s->desc_set = s->desc_pending;
            s->desc_key = s->pending_key;
            s->desc_applied = 0;
        } else {
            wp_image_description_v1_destroy(s->desc_pending);
        }
        s->desc_pending = NULL;
    }
    wl_display_flush(s->dpy);
    if (out) *out = s->preferred;
    int c = s->changed;
    s->changed = 0;
    return c;
}

// Demande une description PQ BT.2020 avec ces métadonnées (prête plus tard).
static void make_description(kn_hdr_surface *s, const kn_hdr_meta *m) {
    if (s->desc_pending) wp_image_description_v1_destroy(s->desc_pending);
    struct wp_image_description_creator_params_v1 *c = wp_color_manager_v1_create_parametric_creator(s->cm);
    wp_image_description_creator_params_v1_set_tf_named(c, WP_COLOR_MANAGER_V1_TRANSFER_FUNCTION_ST2084_PQ);
    wp_image_description_creator_params_v1_set_primaries_named(c, WP_COLOR_MANAGER_V1_PRIMARIES_BT2020);
    // Le blanc du bureau de l'ordinateur : ancré au blanc de référence du compositeur.
    if ((s->caps.features & (1u << WP_COLOR_MANAGER_V1_FEATURE_SET_LUMINANCES)) && m->sdr_white >= 1.0f)
        wp_image_description_creator_params_v1_set_luminances(c, 0, 10000, (uint32_t)(m->sdr_white + 0.5f));
    float lo = m->mastering_min > 0 ? m->mastering_min : 0, hi = m->mastering_max;
    int mastering = (s->caps.features & (1u << WP_COLOR_MANAGER_V1_FEATURE_SET_MASTERING_DISPLAY_PRIMARIES)) && hi > lo + 1.0f;
    if (mastering) wp_image_description_creator_params_v1_set_mastering_luminance(c, (uint32_t)(lo * 10000.0f + 0.5f), (uint32_t)(hi + 0.5f));
    // MaxCLL et MaxFALL : au-dessus du noir, au plus la crête de matriçage (10 000 sans elle), MaxFALL ≤ MaxCLL.
    float top = mastering ? hi : 10000.0f;
    float cll = m->max_cll > top ? top : m->max_cll;
    float fall = m->max_fall > cll && cll > 0 ? cll : m->max_fall > top ? top : m->max_fall;
    if (cll >= lo + 1.0f) wp_image_description_creator_params_v1_set_max_cll(c, (uint32_t)cll);
    if (fall >= lo + 1.0f && (cll < lo + 1.0f || (uint32_t)fall <= (uint32_t)cll)) wp_image_description_creator_params_v1_set_max_fall(c, (uint32_t)fall);
    memset(&s->desc_wait, 0, sizeof s->desc_wait);
    s->desc_pending = wp_image_description_creator_params_v1_create(c);
    wp_image_description_v1_add_listener(s->desc_pending, &desc_listener, &s->desc_wait);
    s->pending_key = *m;
}

static int egl_init(kn_hdr_surface *s, int w, int h) {
    PFNEGLGETPLATFORMDISPLAYEXTPROC get_platform = (PFNEGLGETPLATFORMDISPLAYEXTPROC)eglGetProcAddress("eglGetPlatformDisplayEXT");
    // Le même EGLDisplay que celui de SDL (même connexion) : jamais terminé ici.
    s->edpy = get_platform ? get_platform(EGL_PLATFORM_WAYLAND_KHR, s->dpy, NULL) : eglGetDisplay((EGLNativeDisplayType)s->dpy);
    if (s->edpy == EGL_NO_DISPLAY || !eglInitialize(s->edpy, NULL, NULL)) { snprintf(s->err, sizeof s->err, "EGL indisponible"); return -1; }
    EGLint attrs[] = { EGL_SURFACE_TYPE, EGL_WINDOW_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT, EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_NONE };
    EGLConfig configs[64];
    EGLint n = 0;
    if (!eglChooseConfig(s->edpy, attrs, configs, 64, &n) || n <= 0) { snprintf(s->err, sizeof s->err, "aucune configuration EGL"); return -1; }
    // 10 bits par composante d'abord (le PQ en 8 bits se voit en paliers), sinon 8.
    int best = -1, best_bits = 0;
    for (int i = 0; i < n; i++) {
        EGLint r = 0, g = 0, b = 0, type = EGL_COLOR_COMPONENT_TYPE_FIXED_EXT;
        eglGetConfigAttrib(s->edpy, configs[i], EGL_RED_SIZE, &r);
        eglGetConfigAttrib(s->edpy, configs[i], EGL_GREEN_SIZE, &g);
        eglGetConfigAttrib(s->edpy, configs[i], EGL_BLUE_SIZE, &b);
        eglGetConfigAttrib(s->edpy, configs[i], EGL_COLOR_COMPONENT_TYPE_EXT, &type);
        if (type != EGL_COLOR_COMPONENT_TYPE_FIXED_EXT || r != g || g != b) continue;
        if ((r == 10 && best_bits != 10) || (r == 8 && best_bits == 0)) { best = i; best_bits = r; }
    }
    if (best < 0) { snprintf(s->err, sizeof s->err, "aucune configuration EGL en 8 ou 10 bits"); return -1; }
    s->config = configs[best];
    s->bits = best_bits;
    EGLenum api = eglQueryAPI();
    eglBindAPI(EGL_OPENGL_ES_API);
    EGLint cattrs[] = { EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE };
    s->ctx = eglCreateContext(s->edpy, s->config, EGL_NO_CONTEXT, cattrs);
    eglBindAPI(api);
    if (s->ctx == EGL_NO_CONTEXT) { snprintf(s->err, sizeof s->err, "contexte OpenGL ES impossible"); return -1; }
    s->ewin = wl_egl_window_create(s->surface, w, h);
    s->esurf = eglCreateWindowSurface(s->edpy, s->config, (EGLNativeWindowType)s->ewin, NULL);
    if (s->esurf == EGL_NO_SURFACE) { snprintf(s->err, sizeof s->err, "surface EGL impossible"); return -1; }
    s->ew = w;
    s->eh = h;
    return 0;
}

// Les bits par composante de la sortie (0 : pas encore ouverte).
int kn_hdr_surface_bits(kn_hdr_surface *s) { return s->bits; }
const char *kn_hdr_surface_error(kn_hdr_surface *s) { return s->err; }

// Dessine l'image `planes` (P010 ou I010, lignes serrées) sur la sous-surface,
// posée en (x, y, w, h) dans la fenêtre (coordonnées de la fenêtre), avec ce
// rendu et ces métadonnées. 0 : fait ; < 0 : impossible (EGL, shader : `err`).
int kn_hdr_surface_present(kn_hdr_surface *s, int format, int width, int height, const uint8_t *const planes[3], const kn_hdr_tone *t, const kn_hdr_meta *m, int x, int y, int w, int h) {
    if (s->egl_failed) return -1;
    EGLDisplay pdpy = eglGetCurrentDisplay();
    EGLContext pctx = eglGetCurrentContext();
    EGLSurface pdraw = eglGetCurrentSurface(EGL_DRAW), pread = eglGetCurrentSurface(EGL_READ);
    EGLenum papi = eglQueryAPI();
    int r = 0;
    if (!s->ctx && egl_init(s, width, height) < 0) { s->egl_failed = 1; r = -1; goto restore; }
    if (s->ew != width || s->eh != height) {
        wl_egl_window_resize(s->ewin, width, height, 0, 0);
        s->ew = width;
        s->eh = height;
    }
    eglBindAPI(EGL_OPENGL_ES_API);
    if (!eglMakeCurrent(s->edpy, s->esurf, s->esurf, s->ctx)) { snprintf(s->err, sizeof s->err, "eglMakeCurrent"); r = -1; goto restore; }
    if (!s->gl_ok) {
        if (kn_hdr_gl_init(&s->gl, s->err, sizeof s->err) < 0) { s->egl_failed = 1; r = -1; goto restore; }
        s->gl_ok = 1;
        // Sans attendre la synchro verticale : la boucle de l'app ne bloque pas ici.
        eglSwapInterval(s->edpy, 0);
    }
    if (kn_hdr_gl_upload(&s->gl, format, width, height, planes) < 0) { r = -1; goto restore; }
    kn_hdr_gl_draw(&s->gl, t, width, height);
    // La description : PQ BT.2020 pour une image HDR (telle quelle ou ramenée à l'écran), aucune vers le SDR (sRGB).
    if (!s->cms) {
        // Sans gestion des couleurs (essai) : rien à décrire, l'image est prise pour du sRGB.
    } else if (t->mode == 2) {
        if (s->desc_key_mode != 2) { wp_color_management_surface_v1_unset_image_description(s->cms); s->desc_key_mode = 2; }
    } else {
        int same = s->desc_set && memcmp(&s->desc_key, m, sizeof *m) == 0;
        int asked = s->desc_pending && memcmp(&s->pending_key, m, sizeof *m) == 0;
        if (!same && !asked) make_description(s, m);
        if (s->desc_set && (!s->desc_applied || s->desc_key_mode != t->mode)) {
            wp_color_management_surface_v1_set_image_description(s->cms, s->desc_set, WP_COLOR_MANAGER_V1_RENDER_INTENT_PERCEPTUAL);
            s->desc_applied = 1;
            s->desc_key_mode = t->mode;
        }
    }
    if (x != s->x || y != s->y || !s->mapped) wl_subsurface_set_position(s->sub, x, y);
    if (w != s->w || h != s->h || !s->mapped) wp_viewport_set_destination(s->viewport, w > 0 ? w : 1, h > 0 ? h : 1);
    s->x = x; s->y = y; s->w = w; s->h = h;
    if (!eglSwapBuffers(s->edpy, s->esurf)) { snprintf(s->err, sizeof s->err, "eglSwapBuffers"); r = -1; }
    s->mapped = r == 0;
restore:
    eglBindAPI(papi);
    if (pdpy != EGL_NO_DISPLAY) eglMakeCurrent(pdpy, pdraw, pread, pctx);
    else if (s->edpy) eglMakeCurrent(s->edpy, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    wl_display_flush(s->dpy);
    return r;
}

// Retire l'image (une image SDR revient par SDL, la session est finie).
void kn_hdr_surface_hide(kn_hdr_surface *s) {
    if (!s->mapped) return;
    wl_surface_attach(s->surface, NULL, 0, 0);
    wl_surface_commit(s->surface);
    s->mapped = 0;
    wl_display_flush(s->dpy);
}

void kn_hdr_surface_free(kn_hdr_surface *s) {
    if (!s) return;
    if (s->ctx) {
        EGLDisplay pdpy = eglGetCurrentDisplay();
        EGLContext pctx = eglGetCurrentContext();
        EGLSurface pdraw = eglGetCurrentSurface(EGL_DRAW), pread = eglGetCurrentSurface(EGL_READ);
        EGLenum papi = eglQueryAPI();
        eglBindAPI(EGL_OPENGL_ES_API);
        if (s->esurf != EGL_NO_SURFACE && eglMakeCurrent(s->edpy, s->esurf, s->esurf, s->ctx) && s->gl_ok) kn_hdr_gl_free(&s->gl);
        eglMakeCurrent(s->edpy, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
        if (s->esurf != EGL_NO_SURFACE) eglDestroySurface(s->edpy, s->esurf);
        eglDestroyContext(s->edpy, s->ctx);
        eglBindAPI(papi);
        if (pdpy != EGL_NO_DISPLAY) eglMakeCurrent(pdpy, pdraw, pread, pctx);
    }
    if (s->ewin) wl_egl_window_destroy(s->ewin);
    if (s->pref.info) wp_image_description_info_v1_destroy(s->pref.info);
    if (s->pref_obj) wp_image_description_v1_destroy(s->pref_obj);
    if (s->desc_pending) wp_image_description_v1_destroy(s->desc_pending);
    if (s->desc_set) wp_image_description_v1_destroy(s->desc_set);
    if (s->feedback) wp_color_management_surface_feedback_v1_destroy(s->feedback);
    if (s->cms) wp_color_management_surface_v1_destroy(s->cms);
    if (s->viewport) wp_viewport_destroy(s->viewport);
    if (s->sub) wl_subsurface_destroy(s->sub);
    if (s->surface) wl_surface_destroy(s->surface);
    if (s->cm) wp_color_manager_v1_destroy(s->cm);
    if (s->viewporter) wp_viewporter_destroy(s->viewporter);
    if (s->subcompositor) wl_subcompositor_destroy(s->subcompositor);
    if (s->compositor) wl_compositor_destroy(s->compositor);
    if (s->registry) wl_registry_destroy(s->registry);
    if (s->wrapper) wl_proxy_wrapper_destroy(s->wrapper);
    if (s->dpy) wl_display_flush(s->dpy);
    if (s->queue) wl_event_queue_destroy(s->queue);
    free(s);
}

// ---- Essai hors écran (tests) ---------------------------------------------

// Dessine l'image dans une texture RGBA 8 bits hors écran (EGL sans surface,
// Mesa) et la relit dans `rgba` (largeur × hauteur × 4). 0 : fait ; -1 : pas
// d'EGL utilisable ici (le test est sauté), -2 : erreur du rendu (`err`).
int kn_hdr_offscreen(int format, int width, int height, const uint8_t *const planes[3], const kn_hdr_tone *t, uint8_t *rgba, char *err, int errlen) {
    PFNEGLGETPLATFORMDISPLAYEXTPROC get_platform = (PFNEGLGETPLATFORMDISPLAYEXTPROC)eglGetProcAddress("eglGetPlatformDisplayEXT");
    EGLDisplay d = get_platform ? get_platform(EGL_PLATFORM_SURFACELESS_MESA, EGL_DEFAULT_DISPLAY, NULL) : EGL_NO_DISPLAY;
    if (d == EGL_NO_DISPLAY || !eglInitialize(d, NULL, NULL)) { snprintf(err, errlen, "EGL sans surface indisponible"); return -1; }
    EGLint attrs[] = { EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT, EGL_NONE };
    EGLConfig cfg;
    EGLint n = 0;
    if (!eglChooseConfig(d, attrs, &cfg, 1, &n) || n < 1) {
        EGLint any[] = { EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT, EGL_NONE };
        if (!eglChooseConfig(d, any, &cfg, 1, &n) || n < 1) { snprintf(err, errlen, "aucune configuration EGL"); return -1; }
    }
    eglBindAPI(EGL_OPENGL_ES_API);
    EGLint cattrs[] = { EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE };
    EGLContext ctx = eglCreateContext(d, cfg, EGL_NO_CONTEXT, cattrs);
    if (ctx == EGL_NO_CONTEXT || !eglMakeCurrent(d, EGL_NO_SURFACE, EGL_NO_SURFACE, ctx)) { snprintf(err, errlen, "contexte sans surface impossible"); return -1; }
    int r = 0;
    kn_hdr_gl g;
    GLuint fbo = 0, tex = 0;
    glGenTextures(1, &tex);
    glBindTexture(GL_TEXTURE_2D, tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
    glGenFramebuffers(1, &fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
    if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) { snprintf(err, errlen, "tampon hors écran incomplet"); r = -2; goto out; }
    if (kn_hdr_gl_init(&g, err, errlen) < 0) { r = -2; goto out; }
    if (kn_hdr_gl_upload(&g, format, width, height, planes) < 0) { snprintf(err, errlen, "envoi des plans"); r = -2; kn_hdr_gl_free(&g); goto out; }
    glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    kn_hdr_gl_draw(&g, t, width, height);
    glPixelStorei(GL_PACK_ALIGNMENT, 1);
    glReadPixels(0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, rgba);
    if (glGetError() != GL_NO_ERROR) { snprintf(err, errlen, "relecture"); r = -2; }
    kn_hdr_gl_free(&g);
out:
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    if (fbo) glDeleteFramebuffers(1, &fbo);
    if (tex) glDeleteTextures(1, &tex);
    eglMakeCurrent(d, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    eglDestroyContext(d, ctx);
    return r;
}
