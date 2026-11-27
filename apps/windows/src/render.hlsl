// Shaders du rendu de l'app Windows (`render.rs`), compilés au lancement
// par D3DCompile. Les courbes HDR recopient celles du cœur
// (`kybernow_core::rules::hdr`) : l'essai `shader_comme_le_coeur` de
// `hdr.rs` les vérifie sur le Mac.
struct VSOut { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
VSOut vs(uint id : SV_VertexID) {
    VSOut o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.uv = uv;
    o.pos = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}
Texture2D t0 : register(t0);
Texture2D t1 : register(t1);
Texture2D t2 : register(t2);
SamplerState smp : register(s0);
cbuffer Params : register(b0) {
    float4 m0; float4 m1; float4 m2;
    uint mode; float2 hint_px; float pad;
    // Sortie (hdr.rs, `Tone`) : 0 SDR telle quelle, 1 PQ vers un écran HDR,
    // 2 PQ ramenée au SDR ; EETF de BT.2390 (sortie 1) de src_pq vers
    // l'écran, de dst_lo_pq (son noir) à dst_pq (sa crête).
    uint out_mode; uint tone; float src_pq; float dst_pq;
    float sdr_white; float hint_level; float dst_lo_pq; float pad2;
};
static const float PQ_M1 = 0.1593017578125;
static const float PQ_M2 = 78.84375;
static const float PQ_C1 = 0.8359375;
static const float PQ_C2 = 18.8515625;
static const float PQ_C3 = 18.6875;
// PQ → linéaire (1 = 10 000 nits), et retour.
float3 pq_eotf(float3 e) {
    float3 p = pow(saturate(e), 1.0 / PQ_M2);
    return pow(max(p - PQ_C1, 0.0) / (PQ_C2 - PQ_C3 * p), 1.0 / PQ_M1);
}
float3 pq_oetf(float3 y) {
    float3 p = pow(saturate(y), PQ_M1);
    return pow((PQ_C1 + PQ_C2 * p) / (1.0 + PQ_C3 * p), PQ_M2);
}
// EETF de BT.2390 sur une valeur PQ (`rules::hdr::eetf` du cœur) : genou
// de Hermite, puis relèvement du noir dst_lo·(1−E)⁴.
float eetf(float e) {
    float e1 = saturate(e / src_pq);
    float maxl = min(dst_pq / src_pq, 1.0);
    float minl = saturate(dst_lo_pq / src_pq);
    float ks = max(1.5 * maxl - 0.5, 0.0);
    float e2 = e1;
    if (e1 >= ks && ks < 1.0) {
        float t = (e1 - ks) / (1.0 - ks);
        float t2 = t * t;
        float t3 = t2 * t;
        e2 = (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * maxl;
    }
    float b = 1.0 - e2;
    return (e2 + minl * b * b * b * b) * src_pq;
}
// Vers le SDR, relatif au blanc du bureau (`rules::hdr::sdr_curve`, la
// courbe de la Station) : rien sous le genou (0,75), puis pente 1 au genou et
// 4 fois le blanc ramené à 1 (`SDR_SLOPE`).
float sdr_curve(float m) {
    if (m <= 0.75) return m;
    float x = m - 0.75;
    return min(0.75 + x / (1.0 + 3.6923077 * x), 1.0);
}
static const float3x3 BT2020_TO_709 = {
     1.660491, -0.587641, -0.072850,
    -0.124550,  1.132900, -0.008349,
    -0.018151, -0.100579,  1.118730
};
float4 ps(VSOut i) : SV_Target {
    float3 yuv;
    if (mode == 0) { yuv = float3(t0.Sample(smp, i.uv).r, t1.Sample(smp, i.uv).rg); }
    else if (mode == 1) { yuv = float3(t0.Sample(smp, i.uv).r, t1.Sample(smp, i.uv).r, t2.Sample(smp, i.uv).r); }
    else { float4 a = t0.Sample(smp, i.uv); yuv = float3(a.b, a.g, a.r); }
    float4 v = float4(yuv, 1);
    float3 rgb = saturate(float3(dot(m0, v), dot(m1, v), dot(m2, v)));
    if (out_mode == 1) {
        if (tone != 0) {
            // Sur le maximum des trois composantes : la teinte reste.
            float m = max(max(rgb.r, rgb.g), rgb.b);
            float lm = pq_eotf(m.xxx).x;
            float k = lm > 0.0 ? pq_eotf(eetf(m).xxx).x / lm : 1.0;
            rgb = pq_oetf(pq_eotf(rgb) * k);
        }
        return float4(rgb, 1);
    }
    if (out_mode == 2) {
        float3 lin = max(mul(BT2020_TO_709, pq_eotf(rgb) * (10000.0 / sdr_white)), 0.0);
        float m = max(max(lin.r, lin.g), lin.b);
        if (m > 0.75) lin *= sdr_curve(m) / m;
        return float4(pow(saturate(lin), 1.0 / 2.2), 1);
    }
    return float4(rgb, 1);
}
// Trait du bord haut : une capsule blanche de `hint_px` pixels (le tracé
// couvre exactement sa zone), bord adouci sur un pixel, opacité de 35 %.
// Sur un écran HDR, son blanc est celui du bureau (`hint_level`, en PQ).
float4 ps_hint(VSOut i) : SV_Target {
    float2 p = (i.uv - 0.5) * hint_px;
    float d = length(float2(max(abs(p.x) - (hint_px.x - hint_px.y) * 0.5, 0.0), p.y)) - hint_px.y * 0.5;
    return float4(hint_level.xxx, saturate(0.5 - d) * 0.35);
}
