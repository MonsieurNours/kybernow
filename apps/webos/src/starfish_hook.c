/* Crochet sur StarfishMediaAPIs::Load, le lecteur média de LG que le
 * décodeur du téléviseur (NDL DirectMedia, chargé par dlopen dans ce
 * processus) pilote par des documents JSON. Le symbole, exporté par le
 * binaire, passe devant celui de libplayerAPIs pour les bibliothèques
 * chargées ensuite : le document est journalisé, puis remis tel quel au
 * vrai Load. Feed n'est plus intercepté (WEBOS-24) : il ne changeait rien,
 * et son ABI std::string était devinée. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

typedef void (*starfish_cb)(int type, long long num, const char *str, void *data);
typedef int (*starfish_load)(void *self, const char *payload, starfish_cb cb, void *data);
typedef void (*starfish_cb3)(int type, long long num, const char *str);
typedef int (*starfish_load3)(void *self, const char *payload, starfish_cb3 cb);

/* Côté Rust : journal du document et des événements du lecteur (rappel du
 * chargement). */
extern void kybernow_starfish_seen(const char *payload);
extern void kybernow_starfish_event(int type, long long num, const char *str);

/* Le rappel du décodeur, un seul lecteur à la fois : vu avant d'être remis. */
static starfish_cb3 g_cb3;
static void trampoline3(int type, long long num, const char *str) {
    kybernow_starfish_event(type, num, str);
    if (g_cb3) {
        g_cb3(type, num, str);
    }
}

int _ZN17StarfishMediaAPIs4LoadEPKcPFvixS1_PvES2_(void *self, const char *payload, starfish_cb cb, void *data) {
    static starfish_load real;
    if (!real) {
        real = (starfish_load)dlsym(RTLD_NEXT, "_ZN17StarfishMediaAPIs4LoadEPKcPFvixS1_PvES2_");
        if (!real) {
            return 0;
        }
    }
    kybernow_starfish_seen(payload);
    return real(self, payload, cb, data);
}

/* La variante sans données, celle qu'appelle le décodeur du téléviseur. */
int _ZN17StarfishMediaAPIs4LoadEPKcPFvixS1_E(void *self, const char *payload, starfish_cb3 cb) {
    static starfish_load3 real;
    if (!real) {
        real = (starfish_load3)dlsym(RTLD_NEXT, "_ZN17StarfishMediaAPIs4LoadEPKcPFvixS1_E");
        if (!real) {
            return 0;
        }
    }
    kybernow_starfish_seen(payload);
    g_cb3 = cb;
    return real(self, payload, trampoline3);
}
