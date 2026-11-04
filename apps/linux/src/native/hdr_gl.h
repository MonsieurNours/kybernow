// Rendu d'une image HDR par OpenGL ES 2 (hdr_gl.c), dans le contexte courant.
#ifndef KN_HDR_GL_H
#define KN_HDR_GL_H

#include <stdint.h>

#define KN_HDR_P010 4
#define KN_HDR_I010 5

// Les paramètres de `Tone` (src/hdr.rs) : mode 0 PQ tel quel, 1 EETF, 2 SDR ;
// bornes de l'EETF en PQ ; blanc SDR en cd/m².
typedef struct kn_hdr_tone {
    int mode;
    float src_hi, dst_lo, dst_hi, white;
} kn_hdr_tone;

typedef struct kn_hdr_gl {
    unsigned program, vbo, tex[3];
    int u_planar, u_unit, u_mode, u_eetf, u_white;
    int alloc[3][3];
    int format, width, height;
} kn_hdr_gl;

int kn_hdr_gl_init(kn_hdr_gl *g, char *err, int errlen);
// Plans serrés (largeur × 2 octets par ligne ; chroma à la moitié).
int kn_hdr_gl_upload(kn_hdr_gl *g, int format, int width, int height, const uint8_t *const planes[3]);
// Dessine l'image dans le cadre (0, 0, vw, vh) du tampon courant.
void kn_hdr_gl_draw(kn_hdr_gl *g, const kn_hdr_tone *t, int vw, int vh);
void kn_hdr_gl_free(kn_hdr_gl *g);

#endif
