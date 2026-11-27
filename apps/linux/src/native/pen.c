// Le stylet d'une tablette graphique (docs/link-ui.md, « Stylet » ;
// docs/stylet.md). SDL2 ne voit qu'une souris : la pression, l'inclinaison,
// la gomme et le bouton du stylet sont lus ici, à côté de SDL.
//
// - X11 : XInput2 sur une connexion à part (libX11 et libXi chargées à
//   l'exécution, comme SDL), écoutée par un fil qui réveille la boucle de
//   l'app (événement SDL). Les appareils dont un axe s'appelle « Abs
//   Pressure » sont des stylets (pilotes wacom et libinput) ; « eraser »
//   dans le nom, la gomme. XInput2 ne dit pas quand le stylet s'éloigne :
//   un stylet en survol qui ne bouge plus depuis 500 ms est tenu pour parti.
//   SDL reçoit toujours le stylet en souris (le pointeur du serveur X).
// - Wayland : `zwp_tablet_v2` sur la connexion de SDL, dans une file à nous
//   traitée à chaque tour de la boucle, comme le HDR (wayland_hdr.c). Le
//   compositeur n'envoie alors plus le stylet en souris à l'app : l'app
//   rejoue elle-même ses survols et ses clics pour son interface.
//
// Une seule instance pour l'app ; chaque fenêtre de SDL est suivie
// (`kn_pen_watch`) et chaque événement porte l'identifiant SDL de la sienne.

#include <ctype.h>
#include <dlfcn.h>
#include <poll.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include <wayland-client.h>
#include <SDL.h>
#include <SDL_syswm.h>

#include "wayland/tablet-v2-client.h"

#if __has_include(<X11/extensions/XInput2.h>)
#define KN_PEN_X11 1
#include <X11/Xlib.h>
#include <X11/extensions/XInput2.h>
#endif

typedef struct {
    uint32_t window_id;
    float x, y;
    float pressure;
    float tilt_x, tilt_y;
    float rotation;
    uint8_t in_range, contact, eraser, barrel, has_tilt, has_rotation;
} kn_pen_ev;

#define KN_PEN_QUEUE 256
#define KN_PEN_WINDOWS 8

typedef struct kn_pen kn_pen;

typedef struct {
    SDL_Window *window;
    uint32_t id;
    struct wl_surface *surface;
    unsigned long xwindow;
} kn_pen_win;

// L'état d'un outil : ce qu'on envoie à chaque image du protocole.
typedef struct {
    kn_pen_ev ev;
    int has_pressure;
} kn_pen_tool;

struct kn_pen {
    pthread_mutex_t lock;
    kn_pen_ev queue[KN_PEN_QUEUE];
    int head, count;
    uint32_t wake_type;
    kn_pen_win wins[KN_PEN_WINDOWS];
    int nwins;
    // Wayland.
    struct wl_display *dpy;
    struct wl_event_queue *wq;
    struct wl_display *wrapper;
    struct wl_registry *registry;
    struct zwp_tablet_manager_v2 *manager;
    struct wl_seat *seat;
    struct zwp_tablet_seat_v2 *tablet_seat;
#ifdef KN_PEN_X11
    // X11.
    void *libx11, *libxi;
    Display *xdpy;
    int xi_opcode;
    pthread_t thread;
    int thread_on;
    int stop_pipe[2];
#endif
};

static void push(kn_pen *p, const kn_pen_ev *ev) {
    pthread_mutex_lock(&p->lock);
    if (p->count == KN_PEN_QUEUE) {
        // Plein : le plus ancien déplacement part ; les états se suivent.
        p->head = (p->head + 1) % KN_PEN_QUEUE;
        p->count--;
    }
    p->queue[(p->head + p->count) % KN_PEN_QUEUE] = *ev;
    p->count++;
    uint32_t wake = p->wake_type;
    pthread_mutex_unlock(&p->lock);
    if (wake) {
        SDL_Event e;
        memset(&e, 0, sizeof e);
        e.type = wake;
        SDL_PushEvent(&e);
    }
}

static uint32_t window_of_surface(kn_pen *p, struct wl_surface *s) {
    for (int i = 0; i < p->nwins; i++)
        if (p->wins[i].surface == s) return p->wins[i].id;
    return 0;
}

// ---- Wayland ------------------------------------------------------------

static void tool_type(void *d, struct zwp_tablet_tool_v2 *t, uint32_t type) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.eraser = type == ZWP_TABLET_TOOL_V2_TYPE_ERASER;
}
static void tool_serial(void *d, struct zwp_tablet_tool_v2 *t, uint32_t a, uint32_t b) { (void)d; (void)t; (void)a; (void)b; }
static void tool_hwid(void *d, struct zwp_tablet_tool_v2 *t, uint32_t a, uint32_t b) { (void)d; (void)t; (void)a; (void)b; }
static void tool_capability(void *d, struct zwp_tablet_tool_v2 *t, uint32_t cap) {
    (void)t;
    kn_pen_tool *tool = d;
    if (cap == ZWP_TABLET_TOOL_V2_CAPABILITY_TILT) tool->ev.has_tilt = 1;
    if (cap == ZWP_TABLET_TOOL_V2_CAPABILITY_ROTATION) tool->ev.has_rotation = 1;
    if (cap == ZWP_TABLET_TOOL_V2_CAPABILITY_PRESSURE) tool->has_pressure = 1;
}
static void tool_done(void *d, struct zwp_tablet_tool_v2 *t) { (void)d; (void)t; }
static void tool_removed(void *d, struct zwp_tablet_tool_v2 *t) {
    zwp_tablet_tool_v2_destroy(t);
    free(d);
}
static kn_pen *s_wl_pen; // l'instance de l'app (une seule).
static void tool_proximity_in(void *d, struct zwp_tablet_tool_v2 *t, uint32_t serial, struct zwp_tablet_v2 *tablet, struct wl_surface *surface) {
    (void)t; (void)serial; (void)tablet;
    kn_pen_tool *tool = d;
    tool->ev.window_id = s_wl_pen ? window_of_surface(s_wl_pen, surface) : 0;
    tool->ev.in_range = tool->ev.window_id != 0;
}
static void tool_proximity_out(void *d, struct zwp_tablet_tool_v2 *t) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.in_range = 0;
    tool->ev.contact = 0;
    tool->ev.barrel = 0;
}
static void tool_down(void *d, struct zwp_tablet_tool_v2 *t, uint32_t serial) {
    (void)t; (void)serial;
    kn_pen_tool *tool = d;
    tool->ev.contact = 1;
    // Sans capteur de pression, un stylet posé appuie à fond.
    if (!tool->has_pressure) tool->ev.pressure = 1.0f;
}
static void tool_up(void *d, struct zwp_tablet_tool_v2 *t) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.contact = 0;
}
static void tool_motion(void *d, struct zwp_tablet_tool_v2 *t, wl_fixed_t x, wl_fixed_t y) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.x = (float)wl_fixed_to_double(x);
    tool->ev.y = (float)wl_fixed_to_double(y);
}
static void tool_pressure(void *d, struct zwp_tablet_tool_v2 *t, uint32_t pressure) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.pressure = (float)pressure / 65535.0f;
}
static void tool_distance(void *d, struct zwp_tablet_tool_v2 *t, uint32_t v) { (void)d; (void)t; (void)v; }
static void tool_tilt(void *d, struct zwp_tablet_tool_v2 *t, wl_fixed_t x, wl_fixed_t y) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.tilt_x = (float)wl_fixed_to_double(x);
    tool->ev.tilt_y = (float)wl_fixed_to_double(y);
    tool->ev.has_tilt = 1;
}
static void tool_rotation(void *d, struct zwp_tablet_tool_v2 *t, wl_fixed_t deg) {
    (void)t;
    kn_pen_tool *tool = d;
    tool->ev.rotation = (float)wl_fixed_to_double(deg);
    tool->ev.has_rotation = 1;
}
static void tool_slider(void *d, struct zwp_tablet_tool_v2 *t, int32_t v) { (void)d; (void)t; (void)v; }
static void tool_wheel(void *d, struct zwp_tablet_tool_v2 *t, wl_fixed_t deg, int32_t clicks) { (void)d; (void)t; (void)deg; (void)clicks; }
static void tool_button(void *d, struct zwp_tablet_tool_v2 *t, uint32_t serial, uint32_t button, uint32_t state) {
    (void)t; (void)serial;
    kn_pen_tool *tool = d;
    // BTN_STYLUS (0x14b) : le bouton du stylet ; BTN_STYLUS2 (0x14c) aussi.
    if (button == 0x14b || button == 0x14c)
        tool->ev.barrel = state == ZWP_TABLET_TOOL_V2_BUTTON_STATE_PRESSED;
}
static void tool_frame(void *d, struct zwp_tablet_tool_v2 *t, uint32_t time) {
    (void)t; (void)time;
    kn_pen_tool *tool = d;
    if (!s_wl_pen || !tool->ev.window_id) return;
    push(s_wl_pen, &tool->ev);
    if (!tool->ev.in_range) tool->ev.window_id = 0;
}

static const struct zwp_tablet_tool_v2_listener s_tool_listener = {
    .type = tool_type,
    .hardware_serial = tool_serial,
    .hardware_id_wacom = tool_hwid,
    .capability = tool_capability,
    .done = tool_done,
    .removed = tool_removed,
    .proximity_in = tool_proximity_in,
    .proximity_out = tool_proximity_out,
    .down = tool_down,
    .up = tool_up,
    .motion = tool_motion,
    .pressure = tool_pressure,
    .distance = tool_distance,
    .tilt = tool_tilt,
    .rotation = tool_rotation,
    .slider = tool_slider,
    .wheel = tool_wheel,
    .button = tool_button,
    .frame = tool_frame,
};

static void tablet_name(void *d, struct zwp_tablet_v2 *t, const char *name) { (void)d; (void)t; (void)name; }
static void tablet_id(void *d, struct zwp_tablet_v2 *t, uint32_t vid, uint32_t pid) { (void)d; (void)t; (void)vid; (void)pid; }
static void tablet_path(void *d, struct zwp_tablet_v2 *t, const char *path) { (void)d; (void)t; (void)path; }
static void tablet_done(void *d, struct zwp_tablet_v2 *t) { (void)d; (void)t; }
static void tablet_removed(void *d, struct zwp_tablet_v2 *t) {
    (void)d;
    zwp_tablet_v2_destroy(t);
}
static const struct zwp_tablet_v2_listener s_tablet_listener = {
    .name = tablet_name,
    .id = tablet_id,
    .path = tablet_path,
    .done = tablet_done,
    .removed = tablet_removed,
};

static void seat_tablet_added(void *d, struct zwp_tablet_seat_v2 *s, struct zwp_tablet_v2 *tablet) {
    (void)d; (void)s;
    // La tablette elle-même ne sert pas, mais elle reste en vie jusqu'à son
    // retrait : l'approche du stylet la cite, et un objet détruit cité par un
    // événement coupe la connexion (celle de SDL).
    zwp_tablet_v2_add_listener(tablet, &s_tablet_listener, NULL);
}
static void seat_tool_added(void *d, struct zwp_tablet_seat_v2 *s, struct zwp_tablet_tool_v2 *t) {
    (void)d; (void)s;
    kn_pen_tool *tool = calloc(1, sizeof *tool);
    if (!tool) {
        zwp_tablet_tool_v2_destroy(t);
        return;
    }
    zwp_tablet_tool_v2_add_listener(t, &s_tool_listener, tool);
}
static void seat_pad_added(void *d, struct zwp_tablet_seat_v2 *s, struct zwp_tablet_pad_v2 *pad) {
    (void)d; (void)s;
    // Les boutons de la tablette restent à l'appareil.
    zwp_tablet_pad_v2_destroy(pad);
}
static const struct zwp_tablet_seat_v2_listener s_seat_listener = {
    .tablet_added = seat_tablet_added,
    .tool_added = seat_tool_added,
    .pad_added = seat_pad_added,
};

static void reg_global(void *d, struct wl_registry *r, uint32_t name, const char *iface, uint32_t version) {
    (void)version;
    kn_pen *p = d;
    if (!strcmp(iface, zwp_tablet_manager_v2_interface.name) && !p->manager)
        p->manager = wl_registry_bind(r, name, &zwp_tablet_manager_v2_interface, 1);
    else if (!strcmp(iface, wl_seat_interface.name) && !p->seat)
        p->seat = wl_registry_bind(r, name, &wl_seat_interface, 1);
}
static void reg_remove(void *d, struct wl_registry *r, uint32_t name) { (void)d; (void)r; (void)name; }
static const struct wl_registry_listener s_registry_listener = { reg_global, reg_remove };

static int wl_open(kn_pen *p, struct wl_display *dpy) {
    p->dpy = dpy;
    p->wq = wl_display_create_queue(dpy);
    p->wrapper = wl_proxy_create_wrapper(dpy);
    wl_proxy_set_queue((struct wl_proxy *)p->wrapper, p->wq);
    p->registry = wl_display_get_registry(p->wrapper);
    wl_registry_add_listener(p->registry, &s_registry_listener, p);
    if (wl_display_roundtrip_queue(dpy, p->wq) < 0) return 0;
    if (!p->manager || !p->seat) return 0;
    s_wl_pen = p;
    p->tablet_seat = zwp_tablet_manager_v2_get_tablet_seat(p->manager, p->seat);
    zwp_tablet_seat_v2_add_listener(p->tablet_seat, &s_seat_listener, p);
    wl_display_roundtrip_queue(dpy, p->wq);
    return 1;
}

// ---- X11 ------------------------------------------------------------------

#ifdef KN_PEN_X11
static struct {
    Display *(*OpenDisplay)(const char *);
    int (*CloseDisplay)(Display *);
    int (*conn_number)(Display *);
    int (*Pending)(Display *);
    int (*NextEvent)(Display *, XEvent *);
    Bool (*GetEventData)(Display *, XGenericEventCookie *);
    void (*FreeEventData)(Display *, XGenericEventCookie *);
    Bool (*QueryExtension)(Display *, const char *, int *, int *, int *);
    char *(*GetAtomName)(Display *, Atom);
    int (*Free)(void *);
    int (*Flush)(Display *);
    Window (*default_root)(Display *);
    Status (*XIQueryVersion)(Display *, int *, int *);
    XIDeviceInfo *(*XIQueryDevice)(Display *, int, int *);
    void (*XIFreeDeviceInfo)(XIDeviceInfo *);
    Status (*XISelectEvents)(Display *, Window, XIEventMask *, int);
} X;

#define KN_X_DEVICES 16
typedef struct {
    int id;
    int eraser;
    int pressure, tilt_x, tilt_y, rotation; // numéros d'axes (-1 : absent)
    double p_min, p_max, tx_min, tx_max, ty_min, ty_max, r_min, r_max;
} kn_x_device;

static kn_x_device s_xdev[KN_X_DEVICES];
static int s_nxdev;

// Xlib en plusieurs fils : la connexion du stylet sert au fil X (XPending, XNextEvent) et au fil principal
// (XISelectEvents, XFlush, à chaque fenêtre ouverte). XInitThreads doit précéder tout appel de Xlib dans le
// processus : kn_x_init_threads est appelée au lancement, avant SDL_Init, puis ici par sûreté (libX11 1.8 et
// plus le fait seul ; un appel de plus ne fait rien).
int kn_x_init_threads(void) {
    static int done = -1;
    if (done >= 0) return done;
    done = 0;
    void *x11 = dlopen("libX11.so.6", RTLD_NOW | RTLD_LOCAL);
    if (!x11) return 0;
    int (*init)(void) = (int (*)(void))dlsym(x11, "XInitThreads");
    done = init && init() != 0;
    return done;
}

static int x_load(void) {
    static int loaded = -1;
    if (loaded >= 0) return loaded;
    kn_x_init_threads();
    void *x11 = dlopen("libX11.so.6", RTLD_NOW | RTLD_LOCAL);
    void *xi = dlopen("libXi.so.6", RTLD_NOW | RTLD_LOCAL);
    loaded = 0;
    if (!x11 || !xi) return 0;
#define SYM(lib, field, name) if (!(*(void **)&X.field = dlsym(lib, name))) return 0
    SYM(x11, OpenDisplay, "XOpenDisplay");
    SYM(x11, CloseDisplay, "XCloseDisplay");
    SYM(x11, conn_number, "XConnectionNumber");
    SYM(x11, Pending, "XPending");
    SYM(x11, NextEvent, "XNextEvent");
    SYM(x11, GetEventData, "XGetEventData");
    SYM(x11, FreeEventData, "XFreeEventData");
    SYM(x11, QueryExtension, "XQueryExtension");
    SYM(x11, GetAtomName, "XGetAtomName");
    SYM(x11, Free, "XFree");
    SYM(x11, Flush, "XFlush");
    SYM(x11, default_root, "XDefaultRootWindow");
    SYM(xi, XIQueryVersion, "XIQueryVersion");
    SYM(xi, XIQueryDevice, "XIQueryDevice");
    SYM(xi, XIFreeDeviceInfo, "XIFreeDeviceInfo");
    SYM(xi, XISelectEvents, "XISelectEvents");
#undef SYM
    loaded = 1;
    return 1;
}

static int has_word(const char *s, const char *word) {
    size_t n = strlen(word);
    for (; *s; s++) {
        size_t i = 0;
        while (i < n && s[i] && tolower((unsigned char)s[i]) == word[i]) i++;
        if (i == n) return 1;
    }
    return 0;
}

// Les stylets parmi les appareils d'entrée (à l'ouverture et à chaque
// changement : tablette branchée ou débranchée).
static void x_scan(kn_pen *p) {
    int n = 0;
    XIDeviceInfo *info = X.XIQueryDevice(p->xdpy, XIAllDevices, &n);
    s_nxdev = 0;
    for (int i = 0; info && i < n && s_nxdev < KN_X_DEVICES; i++) {
        if (info[i].use != XISlavePointer || !info[i].enabled) continue;
        kn_x_device d = { .id = info[i].deviceid, .pressure = -1, .tilt_x = -1, .tilt_y = -1, .rotation = -1 };
        d.eraser = has_word(info[i].name, "eraser");
        for (int c = 0; c < info[i].num_classes; c++) {
            if (info[i].classes[c]->type != XIValuatorClass) continue;
            XIValuatorClassInfo *v = (XIValuatorClassInfo *)info[i].classes[c];
            if (!v->label) continue;
            char *label = X.GetAtomName(p->xdpy, v->label);
            if (!label) continue;
            if (!strcmp(label, "Abs Pressure")) { d.pressure = v->number; d.p_min = v->min; d.p_max = v->max; }
            else if (!strcmp(label, "Abs Tilt X")) { d.tilt_x = v->number; d.tx_min = v->min; d.tx_max = v->max; }
            else if (!strcmp(label, "Abs Tilt Y")) { d.tilt_y = v->number; d.ty_min = v->min; d.ty_max = v->max; }
            else if (!strcmp(label, "Abs Wheel") && has_word(info[i].name, "art")) { d.rotation = v->number; d.r_min = v->min; d.r_max = v->max; }
            X.Free(label);
        }
        if (d.pressure >= 0 && d.p_max > d.p_min) s_xdev[s_nxdev++] = d;
    }
    if (info) X.XIFreeDeviceInfo(info);
}

static void x_select(kn_pen *p, unsigned long xwindow) {
    unsigned char bits[XIMaskLen(XI_LASTEVENT)];
    memset(bits, 0, sizeof bits);
    XISetMask(bits, XI_Motion);
    XISetMask(bits, XI_ButtonPress);
    XISetMask(bits, XI_ButtonRelease);
    XISetMask(bits, XI_Leave);
    XIEventMask mask = { .deviceid = XIAllDevices, .mask_len = sizeof bits, .mask = bits };
    X.XISelectEvents(p->xdpy, xwindow, &mask, 1);
    X.Flush(p->xdpy);
}

static double now_s(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec / 1e9;
}

static double axis(const XIValuatorState *v, int number, int *found) {
    *found = 0;
    if (number < 0 || number >= v->mask_len * 8 || !XIMaskIsSet(v->mask, number)) return 0;
    const double *val = v->values;
    for (int i = 0; i < number; i++)
        if (XIMaskIsSet(v->mask, i)) val++;
    *found = 1;
    return *val;
}

static double scale(double v, double lo, double hi, double to_lo, double to_hi) {
    if (hi <= lo) return 0;
    double f = (v - lo) / (hi - lo);
    if (f < 0) f = 0;
    if (f > 1) f = 1;
    return to_lo + f * (to_hi - to_lo);
}

static uint32_t window_of_x(kn_pen *p, Window w) {
    pthread_mutex_lock(&p->lock);
    uint32_t id = 0;
    for (int i = 0; i < p->nwins; i++)
        if (p->wins[i].xwindow == w) id = p->wins[i].id;
    pthread_mutex_unlock(&p->lock);
    return id;
}

static void *x_thread(void *arg) {
    kn_pen *p = arg;
    // L'état du stylet qui sert (un seul à la fois).
    kn_pen_ev cur;
    memset(&cur, 0, sizeof cur);
    int cur_dev = -1;
    double last = 0;
    struct pollfd fds[2] = {
        { .fd = X.conn_number(p->xdpy), .events = POLLIN },
        { .fd = p->stop_pipe[0], .events = POLLIN },
    };
    for (;;) {
        while (X.Pending(p->xdpy)) {
            XEvent xe;
            X.NextEvent(p->xdpy, &xe);
            XGenericEventCookie *cookie = &xe.xcookie;
            if (xe.type != GenericEvent || cookie->extension != p->xi_opcode || !X.GetEventData(p->xdpy, cookie)) continue;
            if (cookie->evtype == XI_HierarchyChanged) {
                x_scan(p);
            } else if (cookie->evtype == XI_Leave) {
                XILeaveEvent *le = cookie->data;
                if (cur.in_range && !cur.contact && le->sourceid == cur_dev) {
                    cur.in_range = 0;
                    cur.barrel = 0;
                    push(p, &cur);
                    cur_dev = -1;
                }
            } else {
                XIDeviceEvent *de = cookie->data;
                kn_x_device *dev = NULL;
                for (int i = 0; i < s_nxdev; i++)
                    if (s_xdev[i].id == de->sourceid) dev = &s_xdev[i];
                uint32_t win = dev ? window_of_x(p, de->event) : 0;
                if (dev && win) {
                    if (cur_dev >= 0 && cur_dev != de->sourceid && cur.in_range) {
                        // Pointe et gomme : l'autre bout part d'abord.
                        kn_pen_ev gone = cur;
                        gone.in_range = gone.contact = gone.barrel = 0;
                        push(p, &gone);
                    }
                    cur_dev = de->sourceid;
                    cur.window_id = win;
                    cur.x = (float)de->event_x;
                    cur.y = (float)de->event_y;
                    cur.in_range = 1;
                    cur.eraser = (uint8_t)dev->eraser;
                    int f;
                    double pr = axis(&de->valuators, dev->pressure, &f);
                    if (f) cur.pressure = (float)scale(pr, dev->p_min, dev->p_max, 0, 1);
                    double tx = axis(&de->valuators, dev->tilt_x, &f);
                    if (f) { cur.tilt_x = (float)scale(tx, dev->tx_min, dev->tx_max, -64, 63); cur.has_tilt = 1; }
                    double ty = axis(&de->valuators, dev->tilt_y, &f);
                    if (f) { cur.tilt_y = (float)scale(ty, dev->ty_min, dev->ty_max, -64, 63); cur.has_tilt = 1; }
                    double ro = axis(&de->valuators, dev->rotation, &f);
                    if (f) { cur.rotation = (float)scale(ro, dev->r_min, dev->r_max, 0, 360); cur.has_rotation = 1; }
                    if (cookie->evtype == XI_ButtonPress || cookie->evtype == XI_ButtonRelease) {
                        int down = cookie->evtype == XI_ButtonPress;
                        if (de->detail == 1) cur.contact = (uint8_t)down;
                        else if (de->detail == 2 || de->detail == 3) cur.barrel = (uint8_t)down;
                    }
                    if (!cur.contact) cur.pressure = 0;
                    last = now_s();
                    push(p, &cur);
                }
            }
            X.FreeEventData(p->xdpy, cookie);
        }
        // XInput2 ne dit pas quand le stylet s'éloigne : survol immobile
        // depuis 500 ms, il est tenu pour parti.
        int timeout = -1;
        if (cur.in_range && !cur.contact) {
            double left = 0.5 - (now_s() - last);
            if (left <= 0) {
                cur.in_range = 0;
                cur.barrel = 0;
                push(p, &cur);
                cur_dev = -1;
            } else {
                timeout = (int)(left * 1000) + 1;
            }
        }
        if (poll(fds, 2, timeout) < 0) continue;
        if (fds[1].revents) break;
    }
    return NULL;
}

static int x_open(kn_pen *p) {
    if (!x_load()) return 0;
    p->xdpy = X.OpenDisplay(NULL);
    if (!p->xdpy) return 0;
    int ev, err;
    if (!X.QueryExtension(p->xdpy, "XInputExtension", &p->xi_opcode, &ev, &err)) return 0;
    int major = 2, minor = 2;
    if (X.XIQueryVersion(p->xdpy, &major, &minor) != Success) return 0;
    x_scan(p);
    // Tablette branchée ou débranchée.
    unsigned char bits[XIMaskLen(XI_LASTEVENT)];
    memset(bits, 0, sizeof bits);
    XISetMask(bits, XI_HierarchyChanged);
    XIEventMask mask = { .deviceid = XIAllDevices, .mask_len = sizeof bits, .mask = bits };
    X.XISelectEvents(p->xdpy, X.default_root(p->xdpy), &mask, 1);
    X.Flush(p->xdpy);
    if (pipe(p->stop_pipe) < 0) return 0;
    if (pthread_create(&p->thread, NULL, x_thread, p) != 0) return 0;
    p->thread_on = 1;
    return 1;
}
#endif

// ---- API ------------------------------------------------------------------

void kn_pen_close(kn_pen *p);

// Le stylet de l'app, d'après la fenêtre `window` (X11 ou Wayland). NULL :
// ni l'un ni l'autre (KMS, Raspberry Pi), ou le système ne le permet pas.
kn_pen *kn_pen_open(SDL_Window *window) {
    SDL_SysWMinfo info;
    SDL_VERSION(&info.version);
    if (!SDL_GetWindowWMInfo(window, &info)) return NULL;
    kn_pen *p = calloc(1, sizeof *p);
    if (!p) return NULL;
    pthread_mutex_init(&p->lock, NULL);
#ifdef KN_PEN_X11
    p->stop_pipe[0] = p->stop_pipe[1] = -1;
#endif
    int ok = 0;
    if (info.subsystem == SDL_SYSWM_WAYLAND) ok = wl_open(p, info.info.wl.display);
#ifdef KN_PEN_X11
    else if (info.subsystem == SDL_SYSWM_X11) ok = x_open(p);
#endif
    if (!ok) {
        kn_pen_close(p);
        return NULL;
    }
    return p;
}

// Le type de l'événement SDL qui réveille la boucle de l'app (X11).
void kn_pen_set_wake(kn_pen *p, uint32_t type) {
    pthread_mutex_lock(&p->lock);
    p->wake_type = type;
    pthread_mutex_unlock(&p->lock);
}

// Suit (ou ne suit plus, `on` à 0) une fenêtre de SDL.
void kn_pen_watch(kn_pen *p, SDL_Window *window, int on) {
    SDL_SysWMinfo info;
    SDL_VERSION(&info.version);
    if (!SDL_GetWindowWMInfo(window, &info)) return;
    pthread_mutex_lock(&p->lock);
    uint32_t id = SDL_GetWindowID(window);
    int at = -1;
    for (int i = 0; i < p->nwins; i++)
        if (p->wins[i].id == id) at = i;
    if (!on) {
        if (at >= 0) p->wins[at] = p->wins[--p->nwins];
        pthread_mutex_unlock(&p->lock);
        return;
    }
    if (at < 0 && p->nwins < KN_PEN_WINDOWS) at = p->nwins++;
    if (at >= 0) {
        kn_pen_win *w = &p->wins[at];
        memset(w, 0, sizeof *w);
        w->window = window;
        w->id = id;
        if (info.subsystem == SDL_SYSWM_WAYLAND) w->surface = info.info.wl.surface;
#ifdef KN_PEN_X11
        if (info.subsystem == SDL_SYSWM_X11) w->xwindow = info.info.x11.window;
#endif
    }
    pthread_mutex_unlock(&p->lock);
#ifdef KN_PEN_X11
    if (at >= 0 && p->xdpy && info.subsystem == SDL_SYSWM_X11) x_select(p, info.info.x11.window);
#endif
}

// L'événement suivant du stylet (1), ou plus rien (0).
int kn_pen_poll(kn_pen *p, kn_pen_ev *out) {
    if (p->wq) wl_display_dispatch_queue_pending(p->dpy, p->wq);
    pthread_mutex_lock(&p->lock);
    int got = p->count > 0;
    if (got) {
        *out = p->queue[p->head];
        p->head = (p->head + 1) % KN_PEN_QUEUE;
        p->count--;
    }
    pthread_mutex_unlock(&p->lock);
    return got;
}

void kn_pen_close(kn_pen *p) {
    if (!p) return;
#ifdef KN_PEN_X11
    if (p->thread_on) {
        if (write(p->stop_pipe[1], "x", 1) < 0) {}
        pthread_join(p->thread, NULL);
    }
    if (p->stop_pipe[0] >= 0) close(p->stop_pipe[0]);
    if (p->stop_pipe[1] >= 0) close(p->stop_pipe[1]);
    if (p->xdpy) X.CloseDisplay(p->xdpy);
#endif
    if (s_wl_pen == p) s_wl_pen = NULL;
    if (p->tablet_seat) zwp_tablet_seat_v2_destroy(p->tablet_seat);
    if (p->manager) zwp_tablet_manager_v2_destroy(p->manager);
    if (p->seat) wl_seat_destroy(p->seat);
    if (p->registry) wl_registry_destroy(p->registry);
    if (p->wrapper) wl_proxy_wrapper_destroy(p->wrapper);
    if (p->wq) wl_event_queue_destroy(p->wq);
    pthread_mutex_destroy(&p->lock);
    free(p);
}
