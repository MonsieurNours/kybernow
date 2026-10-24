#!/usr/bin/env python3
"""Pose une vraie capture d'app sur l'écran vert d'une photo générée :

    composite.py SCENE.png CAPTURE.png SORTIE.png [--glare 0.06] [--debug]

L'écran vert (chroma) est détecté, ses quatre coins trouvés, la capture y est posée en
perspective. Ce qui passe devant l'écran (une tête, une main) reste devant : seul le vert
est remplacé. Les franges vertes autour de l'écran sont neutralisées.
"""
import argparse
import cv2
import numpy as np


def green_mask(img):
    hsv = cv2.cvtColor(img, cv2.COLOR_BGR2HSV)
    h, s, v = cv2.split(hsv)
    m = ((h >= 35) & (h <= 90) & (s >= 70) & (v >= 70)).astype(np.uint8) * 255
    b, g, r = cv2.split(img.astype(np.int16))
    m &= (((g - np.maximum(r, b)) > 35).astype(np.uint8) * 255)
    k = np.ones((3, 3), np.uint8)
    return cv2.morphologyEx(m, cv2.MORPH_OPEN, k)


def quad_of(mask):
    cnts, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    c = max(cnts, key=cv2.contourArea)
    hull = cv2.convexHull(c)
    peri = cv2.arcLength(hull, True)
    for eps in (0.01, 0.015, 0.02, 0.03, 0.04, 0.05):
        ap = cv2.approxPolyDP(hull, eps * peri, True)
        if len(ap) == 4:
            pts = ap.reshape(4, 2).astype(np.float32)
            break
    else:
        p = hull.reshape(-1, 2).astype(np.float32)
        s, d = p.sum(1), p[:, 0] - p[:, 1]
        pts = np.array([p[s.argmin()], p[d.argmax()], p[s.argmax()], p[d.argmin()]], np.float32)
    # Ordre : haut gauche, haut droite, bas droite, bas gauche.
    s, d = pts.sum(1), pts[:, 0] - pts[:, 1]
    tl, br = pts[s.argmin()], pts[s.argmax()]
    tr, bl = pts[d.argmax()], pts[d.argmin()]
    q = np.array([tl, tr, br, bl], np.float32)
    # Un pixel vers l'extérieur, pour couvrir le bord adouci du vert.
    c0 = q.mean(0)
    grow = float(__import__('os').environ.get('GROW', '3'))
    q = c0 + (q - c0) * (1 + grow / max(np.linalg.norm(q - c0, axis=1).min(), 1))
    return q, c


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scene"); ap.add_argument("capture"); ap.add_argument("out")
    ap.add_argument("--glare", type=float, default=0.06)
    ap.add_argument("--dim", type=float, default=0.94, help="luminosité de l'écran (1 = telle quelle)")
    ap.add_argument("--relight", type=float, default=0.9, help="rayon des reflets recolorés, en largeurs d'écran (0 = aucun)")
    ap.add_argument("--debug", action="store_true")
    a = ap.parse_args()

    scene = cv2.imread(a.scene, cv2.IMREAD_COLOR)
    cap = cv2.imread(a.capture, cv2.IMREAD_COLOR)
    H, W = scene.shape[:2]
    gm = green_mask(scene)
    q, cnt = quad_of(gm)

    # La capture, réduite d'abord à la taille de l'écran (réduction propre), puis en perspective.
    tw = int(max(np.linalg.norm(q[1] - q[0]), np.linalg.norm(q[2] - q[3])) * 1.5)
    th = int(max(np.linalg.norm(q[3] - q[0]), np.linalg.norm(q[2] - q[1])) * 1.5)
    small = cv2.resize(cap, (max(tw, 8), max(th, 8)), interpolation=cv2.INTER_AREA)
    src = np.array([[0, 0], [small.shape[1], 0], [small.shape[1], small.shape[0]], [0, small.shape[0]]], np.float32)
    M = cv2.getPerspectiveTransform(src, q)
    warped = cv2.warpPerspective(small, M, (W, H), flags=cv2.INTER_CUBIC, borderMode=cv2.BORDER_CONSTANT)
    warped = cv2.GaussianBlur(warped, (0, 0), 0.45)

    # Écran : un peu moins éclatant que le fichier, un reflet en diagonale très léger.
    f = warped.astype(np.float32) * a.dim + 3
    if a.glare > 0:
        yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
        x0, y0 = q[:, 0].min(), q[:, 1].min()
        x1, y1 = q[:, 0].max(), q[:, 1].max()
        t = ((xx - x0) / max(x1 - x0, 1) + (yy - y0) / max(y1 - y0, 1)) / 2  # 0 en haut à gauche, 1 en bas à droite
        glare = np.clip(1 - np.abs(t - 0.28) / 0.22, 0, 1) * a.glare * 255
        f += glare[..., None]
    warped = np.clip(f, 0, 255).astype(np.uint8)

    # Masque : le quadrilatère, moins ce qui passe devant (ce qui n'est pas vert à l'intérieur).
    quad = np.zeros((H, W), np.uint8)
    cv2.fillConvexPoly(quad, q.astype(np.int32), 255)
    k = max(3, int(W * 0.012) | 1)
    closed = cv2.morphologyEx(gm, cv2.MORPH_CLOSE, np.ones((k, k), np.uint8))
    closed = cv2.dilate(closed, np.ones((3, 3), np.uint8))
    alpha = cv2.bitwise_and(quad, closed).astype(np.float32) / 255
    alpha = cv2.GaussianBlur(alpha, (0, 0), 0.8)[..., None]
    out = (warped.astype(np.float32) * alpha + scene.astype(np.float32) * (1 - alpha))

    # Franges vertes autour de l'écran (et reflets verts tout près) : le vert ramené au niveau du rouge et du bleu.
    band = cv2.dilate(quad, np.ones((max(5, int(W * 0.02)),) * 2, np.uint8))
    sel = (band > 0)
    b, g, r = out[..., 0], out[..., 1], out[..., 2]
    spill = sel & (g > np.maximum(r, b) + 3) & (alpha[..., 0] < 0.98)
    # Ramenées au gris de leur luminance (un cadre de télé est neutre).
    lum = 0.30 * r + 0.50 * np.maximum(r, b) + 0.20 * b
    for ch in (b, g, r):
        ch[spill] = lum[spill]

    # Reflets de l'écran vert dans la pièce (table basse, visages, mains) : la lueur verte devient
    # celle de l'image posée (sa couleur moyenne), dans un large rayon autour de l'écran. Les
    # plantes (vert-jaune, plus saturées et plus sombres) sont épargnées.
    if a.relight > 0:
        sw = max(np.linalg.norm(q[1] - q[0]), np.linalg.norm(q[2] - q[3]))
        big = cv2.dilate(quad, np.ones((int(2 * sw * a.relight) | 1,) * 2, np.uint8))
        inside = alpha[..., 0] > 0.5
        avg = warped[inside].reshape(-1, 3).mean(0) if inside.any() else np.array([90, 60, 60.0])
        tint = avg / max(avg.max(), 1)
        hsv = cv2.cvtColor(np.clip(out, 0, 255).astype(np.uint8), cv2.COLOR_BGR2HSV)
        hh, ss, vv = hsv[..., 0], hsv[..., 1], hsv[..., 2]
        b, g, r = out[..., 0], out[..., 1], out[..., 2]
        e = g - np.maximum(r, b)
        # Lueur de l'écran : vert franc (H ≥ 50), ou vert pâle sur une peau ou un objet clair
        # (H ≥ 33, peu saturé, lumineux). Les feuillages (vert-jaune saturé et sombre) restent.
        glow = (hh >= 50) | ((hh >= 33) & (ss <= 110) & (vv >= 80))
        sel2 = (big > 0) & ~inside & (e > 4) & (hh <= 95) & (vv > 20) & glow
        ee = e[sel2]
        g[sel2] -= ee
        b[sel2] += ee * tint[0] * 0.9
        g[sel2] += ee * tint[1] * 0.9
        r[sel2] += ee * tint[2] * 0.9
    out = np.clip(out, 0, 255).astype(np.uint8)
    cv2.imwrite(a.out, out)
    if a.debug:
        dbg = scene.copy()
        cv2.polylines(dbg, [q.astype(np.int32)], True, (0, 0, 255), 2)
        cv2.imwrite(a.out.replace(".png", "-debug.png"), dbg)
    print(a.out, "coins", q.round(1).tolist())


if __name__ == "__main__":
    main()
