// Les courbes du HDR en Metal (docs/hdr.md), recopiées des règles du cœur
// (`rules::hdr` : PQ de SMPTE ST 2084, primaires BT.2020 vers BT.709,
// courbe vers le SDR de la Station, genou à 0,75 du blanc et pente 3,6923077)
// pour que le même flux rende pareil sur toutes les apps. Un shader ne peut
// pas appeler le cœur : tests/hdr compare ces fonctions, exécutées sur le
// GPU, aux siennes (mêmes valeurs à 1e-4 sur une grille). À part du rendu
// (`VideoRenderer`) pour que l'essai les compile seules.

enum HDRShader {
    /// Les fonctions, à mettre en tête d'une bibliothèque Metal (après
    /// `#include <metal_stdlib>` et `using namespace metal;`).
    static let functions = """
    // PQ (SMPTE ST 2084) → lumière linéaire, 1 = 10 000 nits (`pq_to_nits` du cœur).
    float3 kn_pq_eotf(float3 e) {
        const float m1 = 0.1593017578125, m2 = 78.84375;
        const float c1 = 0.8359375, c2 = 18.8515625, c3 = 18.6875;
        float3 p = pow(clamp(e, 0.0, 1.0), 1.0 / m2);
        return pow(max(p - c1, 0.0) / (c2 - c3 * p), 1.0 / m1);
    }

    // Primaires BT.2020 vers BT.709, lumière linéaire (`BT2020_TO_709` du cœur).
    float3 kn_bt2020_to_709(float3 l) {
        return float3(dot(float3( 1.660491, -0.587641, -0.072850), l),
                      dot(float3(-0.124550,  1.132900, -0.008349), l),
                      dot(float3(-0.018151, -0.100579,  1.118730), l));
    }

    // La courbe vers le SDR de la Station (`sdr_curve` du cœur), sur la plus
    // grande composante en unités du blanc SDR : rien sous le genou (0,75),
    // puis une compression de pente 1 au genou qui ramène 4 fois le blanc à 1.
    float kn_sdr_curve(float m) {
        if (m <= 0.75) { return m; }
        float x = m - 0.75;
        return min(0.75 + x / (1.0 + 3.6923077 * x), 1.0);
    }

    // Image PQ BT.2020 ramenée en SDR BT.709 : blanc SDR à 203 nits, la
    // courbe de la Station sur la plus grande composante, puis la courbe sRGB.
    float3 kn_pq_to_sdr(float3 pq) {
        float3 c = max(kn_bt2020_to_709(kn_pq_eotf(pq) * (10000.0 / 203.0)), 0.0);
        float m = max(max(c.r, c.g), c.b);
        if (m > 0.75) { c *= kn_sdr_curve(m) / m; }
        c = saturate(c);
        return select(1.055 * pow(c, 1.0 / 2.4) - 0.055, 12.92 * c, c <= 0.0031308);
    }

    // Image PQ BT.2020 en lumière linéaire Display P3 étendue (couche
    // `linear`, propre à l'EDR d'Apple : le système n'a pas de PQ à cet
    // endroit) : le blanc du bureau vaut 1,0 ; au-dessus de 80 % de la marge
    // EDR du moment, compression douce jusqu'à la marge, la crête du flux
    // (MaxCLL ou écran de mastering, 10 000 nits sans métadonnées) au bout.
    float3 kn_pq_to_edr(float3 pq, float white, float headroom, float peak) {
        float3 l = kn_pq_eotf(pq) * (10000.0 / white);
        float3 c = float3(dot(float3( 1.3436, -0.2822, -0.0614), l),
                          dot(float3(-0.0653,  1.0758, -0.0105), l),
                          dot(float3( 0.0028, -0.0196,  1.0168), l));
        c = max(c, 0.0);
        float h = max(headroom, 1.0);
        float k = h > 1.01 ? 0.8 * h : 0.75;
        float top = max(peak, white) / white;
        float m = max(max(c.r, c.g), c.b);
        if (m > k && top > h) {
            float a = (m - k) / (h - k);
            float b = (top - k) / (h - k);
            float y = k + (h - k) * a * (1.0 + a / (b * b)) / (1.0 + a);
            c *= y / m;
        }
        return c;
    }

    """
}
