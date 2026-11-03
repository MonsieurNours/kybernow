// Le rendu d'une image HDR (HEVC Main 10, AV1 10 bits : PQ, BT.2020) par
// OpenGL ES 2, dans le contexte courant : les plans 10 bits partent tels quels
// à la carte graphique (deux octets par valeur, en textures LUMINANCE_ALPHA ou
// RGBA, recomposés par le shader), et le shader fait Y'CbCr BT.2020 non
// constant (plage limitée) → R'G'B' PQ, puis selon l'écran :
//   0  PQ tel quel (écran HDR qui tient la crête du flux) ;
//   1  EETF de BT.2390 vers la crête d'un écran HDR plus faible, sur la plus
//      grande composante en lumière linéaire (la teinte reste) ;
//   2  vers le SDR : BT.2020 → BT.709, le blanc du bureau de l'ordinateur
//      ramené à 1, genou à 75 % du blanc et compression jusqu'à 4 fois le
//      blanc (la courbe de l'image à part en SDR de la Station), sRGB.
// Mêmes calculs que `shade` de src/hdr.rs (comparés par tests/hdr/run.sh).
//
// Formats : P010 (deux plans, valeurs 10 bits dans les bits de poids fort de
// mots de 16 bits) et I010 / yuv420p10le (trois plans, 10 bits de poids
// faible), lignes serrées (largeur × 2 octets) : `video.rs` les recopie ainsi.
// OpenGL ES 2 n'a pas de texture 16 bits ; deux octets en LUMINANCE_ALPHA se
// filtrent linéairement comme le mot qu'ils forment.

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <GLES2/gl2.h>

#include "hdr_gl.h"

static const char *VS =
    "attribute vec2 a_pos;\n"
    "varying vec2 v_uv;\n"
    "void main() {\n"
    "    v_uv = vec2(a_pos.x * 0.5 + 0.5, 0.5 - a_pos.y * 0.5);\n"
    "    gl_Position = vec4(a_pos, 0.0, 1.0);\n"
    "}\n";

static const char *FS =
    "#ifdef GL_FRAGMENT_PRECISION_HIGH\n"
    "precision highp float;\n"
    "#else\n"
    "precision mediump float;\n"
    "#endif\n"
    "varying vec2 v_uv;\n"
    "uniform sampler2D t_y;\n"
    "uniform sampler2D t_c;\n"
    "uniform sampler2D t_v;\n"
    "uniform float u_planar;\n"
    "uniform float u_unit;\n"
    "uniform int u_mode;\n"
    "uniform vec3 u_eetf;\n"
    "uniform float u_white;\n"
    "const float M1 = 0.1593017578125;\n"
    "const float M2 = 78.84375;\n"
    "const float C1 = 0.8359375;\n"
    "const float C2 = 18.8515625;\n"
    "const float C3 = 18.6875;\n"
    "float eotf1(float e) {\n"
    "    float p = pow(clamp(e, 0.0, 1.0), 1.0 / M2);\n"
    "    return 10000.0 * pow(max(p - C1, 0.0) / (C2 - C3 * p), 1.0 / M1);\n"
    "}\n"
    "float oetf1(float n) {\n"
    "    if (n <= 0.0) return 0.0;\n"
    "    float l = pow(clamp(n / 10000.0, 0.0, 1.0), M1);\n"
    "    return pow((C1 + C2 * l) / (1.0 + C3 * l), M2);\n"
    "}\n"
    "vec3 eotf(vec3 e) { return vec3(eotf1(e.r), eotf1(e.g), eotf1(e.b)); }\n"
    "vec3 oetf(vec3 n) { return vec3(oetf1(n.r), oetf1(n.g), oetf1(n.b)); }\n"
    // u_eetf : crête du flux, noir et crête de l'écran, en PQ.
    "float eetf(float e) {\n"
    "    float hi = u_eetf.x;\n"
    "    float e1 = clamp(e / hi, 0.0, 1.0);\n"
    "    float maxl = min(u_eetf.z / hi, 1.0);\n"
    "    float minl = clamp(u_eetf.y / hi, 0.0, 1.0);\n"
    "    float ks = max(1.5 * maxl - 0.5, 0.0);\n"
    "    float e2 = e1;\n"
    "    if (e1 >= ks && ks < 1.0) {\n"
    "        float t = (e1 - ks) / (1.0 - ks);\n"
    "        float t2 = t * t;\n"
    "        float t3 = t2 * t;\n"
    "        e2 = (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * maxl;\n"
    "    }\n"
    "    float b = 1.0 - e2;\n"
    "    return (e2 + minl * b * b * b * b) * hi;\n"
    "}\n"
    // Deux octets (poids faible, poids fort) : le mot de 16 bits.
    "float word(float lo, float hi) { return hi * 65280.0 + lo * 255.0; }\n"
    "vec3 srgb(vec3 v) {\n"
    "    vec3 lo = v * 12.92;\n"
    "    vec3 hi = 1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055;\n"
    "    return mix(lo, hi, step(vec3(0.0031308), v));\n"
    "}\n"
    "void main() {\n"
    "    vec4 ty = texture2D(t_y, v_uv);\n"
    "    vec4 tc = texture2D(t_c, v_uv);\n"
    "    float y = word(ty.r, ty.a) / u_unit;\n"
    "    float cb;\n"
    "    float cr;\n"
    "    if (u_planar > 0.5) {\n"
    "        vec4 tv = texture2D(t_v, v_uv);\n"
    "        cb = word(tc.r, tc.a);\n"
    "        cr = word(tv.r, tv.a);\n"
    "    } else {\n"
    "        cb = word(tc.r, tc.g) / u_unit;\n"
    "        cr = word(tc.b, tc.a) / u_unit;\n"
    "    }\n"
    "    float Y = (y - 64.0) / 876.0;\n"
    "    float U = (cb - 512.0) / 896.0;\n"
    "    float V = (cr - 512.0) / 896.0;\n"
    "    vec3 e = clamp(vec3(Y + 1.4746 * V, Y - 0.16455 * U - 0.57135 * V, Y + 1.8814 * U), 0.0, 1.0);\n"
    "    if (u_mode == 0) { gl_FragColor = vec4(e, 1.0); return; }\n"
    "    vec3 n = eotf(e);\n"
    "    if (u_mode == 1) {\n"
    "        float m = max(n.r, max(n.g, n.b));\n"
    "        if (m > 0.0) n *= eotf1(eetf(oetf1(m))) / m;\n"
    "        gl_FragColor = vec4(oetf(n), 1.0);\n"
    "        return;\n"
    "    }\n"
    "    vec3 s = vec3(dot(vec3(1.660491, -0.587641, -0.072850), n),\n"
    "                  dot(vec3(-0.124550, 1.132900, -0.008349), n),\n"
    "                  dot(vec3(-0.018151, -0.100579, 1.118730), n));\n"
    "    s = max(s, 0.0) / max(u_white, 1.0);\n"
    "    float m = max(s.r, max(s.g, s.b));\n"
    "    if (m > 0.75) {\n"
    "        float x = m - 0.75;\n"
    "        s *= (0.75 + x / (1.0 + 3.6923077 * x)) / m;\n"
    "    }\n"
    "    gl_FragColor = vec4(srgb(clamp(s, 0.0, 1.0)), 1.0);\n"
    "}\n";

static GLuint compile(GLenum kind, const char *src, char *err, int errlen) {
    GLuint s = glCreateShader(kind);
    glShaderSource(s, 1, &src, NULL);
    glCompileShader(s);
    GLint ok = 0;
    glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
    if (!ok) {
        char log[512] = {0};
        glGetShaderInfoLog(s, sizeof log, NULL, log);
        if (err && errlen > 0) snprintf(err, errlen, "shader : %s", log);
        glDeleteShader(s);
        return 0;
    }
    return s;
}

int kn_hdr_gl_init(kn_hdr_gl *g, char *err, int errlen) {
    memset(g, 0, sizeof *g);
    GLuint vs = compile(GL_VERTEX_SHADER, VS, err, errlen);
    if (!vs) return -1;
    GLuint fs = compile(GL_FRAGMENT_SHADER, FS, err, errlen);
    if (!fs) { glDeleteShader(vs); return -1; }
    g->program = glCreateProgram();
    glAttachShader(g->program, vs);
    glAttachShader(g->program, fs);
    glBindAttribLocation(g->program, 0, "a_pos");
    glLinkProgram(g->program);
    glDeleteShader(vs);
    glDeleteShader(fs);
    GLint ok = 0;
    glGetProgramiv(g->program, GL_LINK_STATUS, &ok);
    if (!ok) {
        char log[512] = {0};
        glGetProgramInfoLog(g->program, sizeof log, NULL, log);
        if (err && errlen > 0) snprintf(err, errlen, "programme : %s", log);
        glDeleteProgram(g->program);
        g->program = 0;
        return -1;
    }
    g->u_planar = glGetUniformLocation(g->program, "u_planar");
    g->u_unit = glGetUniformLocation(g->program, "u_unit");
    g->u_mode = glGetUniformLocation(g->program, "u_mode");
    g->u_eetf = glGetUniformLocation(g->program, "u_eetf");
    g->u_white = glGetUniformLocation(g->program, "u_white");
    glUseProgram(g->program);
    glUniform1i(glGetUniformLocation(g->program, "t_y"), 0);
    glUniform1i(glGetUniformLocation(g->program, "t_c"), 1);
    glUniform1i(glGetUniformLocation(g->program, "t_v"), 2);
    glGenTextures(3, g->tex);
    for (int i = 0; i < 3; i++) {
        glBindTexture(GL_TEXTURE_2D, g->tex[i]);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    }
    static const float quad[8] = { -1, -1, 1, -1, -1, 1, 1, 1 };
    glGenBuffers(1, &g->vbo);
    glBindBuffer(GL_ARRAY_BUFFER, g->vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof quad, quad, GL_STATIC_DRAW);
    return 0;
}

static void plane(GLuint tex, int unit, GLenum fmt, int w, int h, const uint8_t *data, int *alloc) {
    glActiveTexture(GL_TEXTURE0 + unit);
    glBindTexture(GL_TEXTURE_2D, tex);
    if (alloc[0] != w || alloc[1] != h || alloc[2] != (int)fmt) {
        glTexImage2D(GL_TEXTURE_2D, 0, fmt, w, h, 0, fmt, GL_UNSIGNED_BYTE, data);
        alloc[0] = w; alloc[1] = h; alloc[2] = (int)fmt;
    } else {
        glTexSubImage2D(GL_TEXTURE_2D, 0, 0, 0, w, h, fmt, GL_UNSIGNED_BYTE, data);
    }
}

int kn_hdr_gl_upload(kn_hdr_gl *g, int format, int width, int height, const uint8_t *const planes[3]) {
    if (!g->program || width <= 0 || height <= 0) return -1;
    int cw = (width + 1) / 2, ch = (height + 1) / 2;
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    plane(g->tex[0], 0, GL_LUMINANCE_ALPHA, width, height, planes[0], g->alloc[0]);
    if (format == KN_HDR_P010) {
        plane(g->tex[1], 1, GL_RGBA, cw, ch, planes[1], g->alloc[1]);
    } else if (format == KN_HDR_I010) {
        plane(g->tex[1], 1, GL_LUMINANCE_ALPHA, cw, ch, planes[1], g->alloc[1]);
        plane(g->tex[2], 2, GL_LUMINANCE_ALPHA, cw, ch, planes[2], g->alloc[2]);
    } else {
        return -1;
    }
    g->format = format;
    g->width = width;
    g->height = height;
    return 0;
}

void kn_hdr_gl_draw(kn_hdr_gl *g, const kn_hdr_tone *t, int vw, int vh) {
    if (!g->program || !g->format) return;
    glViewport(0, 0, vw, vh);
    glDisable(GL_BLEND);
    glDisable(GL_DEPTH_TEST);
    glDisable(GL_SCISSOR_TEST);
    glUseProgram(g->program);
    for (int i = 0; i < 3; i++) {
        glActiveTexture(GL_TEXTURE0 + i);
        glBindTexture(GL_TEXTURE_2D, g->tex[i]);
    }
    glUniform1f(g->u_planar, g->format == KN_HDR_I010 ? 1.0f : 0.0f);
    glUniform1f(g->u_unit, g->format == KN_HDR_P010 ? 64.0f : 1.0f);
    glUniform1i(g->u_mode, t->mode);
    glUniform3f(g->u_eetf, t->src_hi > 0 ? t->src_hi : 1.0f, t->dst_lo, t->dst_hi);
    glUniform1f(g->u_white, t->white);
    glBindBuffer(GL_ARRAY_BUFFER, g->vbo);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, 0);
    glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
}

void kn_hdr_gl_free(kn_hdr_gl *g) {
    if (g->program) glDeleteProgram(g->program);
    glDeleteTextures(3, g->tex);
    if (g->vbo) glDeleteBuffers(1, &g->vbo);
    memset(g, 0, sizeof *g);
}
