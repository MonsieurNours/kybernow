// Décodage PyroWave de l'app Android (libkybernow_pyrowave.so) : un device
// Vulkan partagé par l'app, le décodeur de PyroWave (chemin par fragments sur
// les GPU qui le préfèrent) et l'affichage direct sur la surface de la
// SurfaceView par une swapchain Vulkan. Appelé par apps/android/jni/src/pyrowave.rs.
// docs/pyrowave-client-android.md.
#ifndef KN_PYROWAVE_H
#define KN_PYROWAVE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(KN_PYROWAVE_BUILD)
#define KN_PW_API __attribute__((visibility("default")))
#else
#define KN_PW_API
#endif

struct ANativeWindow;
typedef struct kn_pw_session kn_pw_session;

// Sonde : device Vulkan 1.3 avec ce qu'exige le décodeur, décodeur 128 × 128,
// image de test décodée et relue. 1 si PyroWave marche ; sinon 0 et la raison
// (anglaise, pour le journal) dans `report`. Avec 1, `report` dit le GPU et le
// chemin (« Mali-G57, fragment »). Le device reste ouvert pour les sessions.
KN_PW_API int kn_pw_probe(char *report, size_t report_size);

// Une session d'affichage sur cette surface (la swapchain est créée par
// kn_pw_configure). Nul si impossible (raison dans `error`).
KN_PW_API kn_pw_session *kn_pw_open(struct ANativeWindow *window, char *error, size_t error_size);

// Format de l'image : définition, 4:4:4, HDR10 (BT.2020 PQ) ou SDR BT.709,
// plage pleine ou réduite (paquet de configuration KNPW). Recrée le décodeur,
// les plans et, si le HDR change, la swapchain. `max_nits`, `max_fall`,
// `min_nits` : métadonnées HDR10 de l'écran (0 : rien à donner). 1 si prêt.
KN_PW_API int kn_pw_configure(kn_pw_session *session, uint32_t width, uint32_t height, int chroma444, int hdr, int full_range,
                    float max_nits, float max_fall, float min_nits);

enum
{
	KN_PW_SHOWN = 0,      // décodée et présentée (GPU terminé au retour)
	KN_PW_INCOMPLETE = 1, // image incomplète ou illisible : sautée
	KN_PW_BUSY = 2,       // pas d'image de swapchain libre à temps : sautée
	KN_PW_LOST = -1,      // surface perdue ou device en erreur : session à refaire
};

// Une image entière (paquets PyroWave bout à bout).
KN_PW_API int kn_pw_frame(kn_pw_session *session, const uint8_t *data, size_t size);

// « PyroWave, Vulkan, <GPU>, fragment|compute, HDR10 » pour les statistiques.
KN_PW_API const char *kn_pw_describe(kn_pw_session *session);

// La surface affichée en HDR10 (swapchain HDR10_ST2084).
KN_PW_API int kn_pw_hdr_output(kn_pw_session *session);

KN_PW_API void kn_pw_close(kn_pw_session *session);

#ifdef __cplusplus
}
#endif

#endif
