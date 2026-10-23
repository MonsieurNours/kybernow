/* Crochet sur StarfishMediaAPIs::Load, le lecteur média de LG que le
 * décodeur du téléviseur (NDL DirectMedia, chargé par dlopen dans ce
 * processus) pilote par des documents JSON. Le symbole, exporté par le
 * binaire, passe devant celui de libplayerAPIs pour les bibliothèques
 * chargées ensuite : on voit le document, et on peut le réécrire (son
 * Dolby) avant de le remettre au vrai Load. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

typedef void (*starfish_cb)(int type, long long num, const char *str, void *data);
typedef int (*starfish_load)(void *self, const char *payload, starfish_cb cb, void *data);
typedef void (*starfish_cb3)(int type, long long num, const char *str);
typedef int (*starfish_load3)(void *self, const char *payload, starfish_cb3 cb);

/* Côté Rust : journal, réécriture (chaîne malloc, ou NULL pour garder), et
 * les événements du lecteur (rappel du chargement). */
extern void kybernow_starfish_seen(const char *payload);
extern char *kybernow_starfish_rewrite(const char *payload);
extern void kybernow_starfish_event(int type, long long num, const char *str);
extern long long kybernow_starfish_pts_scale(void);

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
    char *rewritten = kybernow_starfish_rewrite(payload);
    int r = real(self, rewritten ? rewritten : payload, cb, data);
    free(rewritten);
    return r;
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
    char *rewritten = kybernow_starfish_rewrite(payload);
    g_cb3 = cb;
    int r = real(self, rewritten ? rewritten : payload, trampoline3);
    free(rewritten);
    return r;
}

/* Feed(const char *payload) rend une std::string : sur l'ABI ARM, l'adresse
 * du résultat vient en premier (r0), puis l'objet, puis le document. Le
 * document ({"bufferAddr":"…","bufferSize":N,"pts":T,"esData":E}) est
 * transmis tel quel, sauf l'horodatage, mis à l'échelle de la chaîne chargée. */
typedef void *(*starfish_feed)(void *ret, void *self, const char *payload);

void *_ZN17StarfishMediaAPIs4FeedB5cxx11EPKc(void *ret, void *self, const char *payload) {
    static starfish_feed real;
    if (!real) {
        real = (starfish_feed)dlsym(RTLD_NEXT, "_ZN17StarfishMediaAPIs4FeedB5cxx11EPKc");
        if (!real) {
            return ret;
        }
    }
    long long scale = kybernow_starfish_pts_scale();
    const char *key = payload ? strstr(payload, "\"pts\":") : NULL;
    if (scale == 1 || !key) {
        return real(ret, self, payload);
    }
    char buf[256];
    const char *num = key + 6;
    char *end;
    long long pts = strtoll(num, &end, 10);
    size_t head = (size_t)(num - payload);
    if (head + 32 + strlen(end) >= sizeof buf) {
        return real(ret, self, payload);
    }
    memcpy(buf, payload, head);
    int n = snprintf(buf + head, sizeof buf - head, "%lld%s", pts * scale, end);
    if (n < 0) {
        return real(ret, self, payload);
    }
    return real(ret, self, buf);
}
