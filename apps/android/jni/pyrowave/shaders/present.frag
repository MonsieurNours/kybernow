#version 450
// Les trois plans décodés par PyroWave (Y, Cb, Cr ; la chroma 4:2:0 est
// agrandie par l'échantillonneur) vers l'image de la swapchain.
//
// mode 0 : SDR BT.709 vers une swapchain SDR (R'G'B' tel quel).
// mode 1 : HDR10 (BT.2020, PQ) vers une swapchain HDR10_ST2084 (R'G'B' en PQ tel quel).
// mode 2 : HDR10 vers une swapchain SDR : PQ → linéaire, BT.2020 → BT.709,
//          blanc SDR à sdr_white nits, genou doux au-dessus du blanc, gamma 2,2.
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 color;

layout(set = 0, binding = 0) uniform sampler2D plane_y;
layout(set = 0, binding = 1) uniform sampler2D plane_cb;
layout(set = 0, binding = 2) uniform sampler2D plane_cr;

layout(push_constant) uniform Params
{
    // Décalage et échelle de Y, puis de Cb et Cr (plage pleine ou réduite).
    vec4 range;
    // Coefficients de la matrice : Cr → R, Cb → G, Cr → G, Cb → B.
    vec4 coef;
    int mode;
    float sdr_white;
} p;

float pq_to_linear(float e)
{
    const float m1 = 2610.0 / 16384.0;
    const float m2 = 2523.0 / 4096.0 * 128.0;
    const float c1 = 3424.0 / 4096.0;
    const float c2 = 2413.0 / 4096.0 * 32.0;
    const float c3 = 2392.0 / 4096.0 * 32.0;
    float ep = pow(clamp(e, 0.0, 1.0), 1.0 / m2);
    return pow(max(ep - c1, 0.0) / (c2 - c3 * ep), 1.0 / m1); // 1,0 = 10 000 nits
}

void main()
{
    float y = (texture(plane_y, uv).r - p.range.x) * p.range.y;
    float cb = (texture(plane_cb, uv).r - p.range.z) * p.range.w;
    float cr = (texture(plane_cr, uv).r - p.range.z) * p.range.w;
    vec3 rgb = vec3(y + p.coef.x * cr, y - p.coef.y * cb - p.coef.z * cr, y + p.coef.w * cb);
    if (p.mode == 2)
    {
        vec3 lin = vec3(pq_to_linear(rgb.r), pq_to_linear(rgb.g), pq_to_linear(rgb.b)) * (10000.0 / p.sdr_white);
        // BT.2020 → BT.709 (colonnes, GLSL).
        lin = mat3(1.6605, -0.1246, -0.0182,
                   -0.5876, 1.1329, -0.1006,
                   -0.0728, -0.0083, 1.1187) * lin;
        lin = max(lin, vec3(0.0));
        // Reinhard étendu, blanc à 4 fois le blanc SDR.
        lin = lin * (1.0 + lin / 16.0) / (1.0 + lin);
        rgb = pow(lin, vec3(1.0 / 2.2));
    }
    color = vec4(clamp(rgb, 0.0, 1.0), 1.0);
}
