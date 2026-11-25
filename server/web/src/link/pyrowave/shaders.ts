// Shaders WGSL du décodeur PyroWave, traduits de wavelet_dequant.comp et
// idwt.comp de l'amont (commit a32b05e, licence MIT, via le port Metal vendu
// dans apps/macos/PyroWave/upstream/shaders), plus la conversion YCbCr → RGB.
// Valeurs intermédiaires en float32 partout (textures r32float).

/** Déquantification d'un bloc 32 × 32 par groupe de 128 fils : un fil par
 * sous-bloc 4 × 2. `subgroups` : la somme préfixe des coefficients non nuls
 * (position des signes) passe par `subgroupInclusiveAdd`, sinon par la
 * mémoire du groupe. La variante à sous-groupes suppose les fils d'un
 * sous-groupe consécutifs : la sonde la vérifie (décodage de l'image de test)
 * et prend l'autre si l'image est fausse. */
export function dequantShader(subgroups: boolean): string {
  const scan = subgroups
    ? /* wgsl */ `
  let incl = subgroupInclusiveAdd(significant);
  let sg = li / sg_size;
  if (sg_lane == sg_size - 1u) {
    sg_totals[sg] = incl;
  }
  workgroupBarrier();
  var excl = incl - significant;
  for (var k = 0u; k < sg; k++) {
    excl += sg_totals[k];
  }`
    : /* wgsl */ `
  counts[li] = significant;
  workgroupBarrier();
  if (li < 16u) {
    var s = 0u;
    for (var k = 0u; k < 8u; k++) {
      s += counts[li * 8u + k];
    }
    block_sums[li] = s;
  }
  workgroupBarrier();
  var excl = 0u;
  for (var k = 0u; k < lb; k++) {
    excl += block_sums[k];
  }
  for (var k = lb * 8u; k < li; k++) {
    excl += counts[k];
  }`
  return /* wgsl */ `${subgroups ? 'enable subgroups;' : ''}

struct Registers {
  resolution: vec2i,
  output_layer: i32,
  block_offset_32x32: i32,
  block_stride_32x32: i32,
}

@group(0) @binding(0) var<storage, read> payload: array<u32>;
@group(0) @binding(1) var<storage, read> offsets: array<u32>;
@group(0) @binding(2) var<uniform> registers: Registers;
@group(0) @binding(3) var out_img: texture_storage_2d_array<r32float, write>;

var<workgroup> plane_costs: array<u32, 16>;
var<workgroup> plane_offsets: array<u32, 16>;
var<workgroup> sign_base: u32;
${subgroups ? 'var<workgroup> sg_totals: array<u32, 32>;' : 'var<workgroup> counts: array<u32, 128>;\nvar<workgroup> block_sums: array<u32, 16>;'}

fn ld8(b: u32) -> u32 {
  return (payload[b >> 2u] >> ((b & 3u) * 8u)) & 0xffu;
}

fn ld16(h: u32) -> u32 {
  return (payload[h >> 1u] >> ((h & 1u) * 16u)) & 0xffffu;
}

fn low_bits(v: u32, n: u32) -> u32 {
  return v & ((1u << n) - 1u);
}

fn unswizzle8x8(index: u32) -> vec2i {
  var y = extractBits(index, 0u, 1u);
  var x = extractBits(index, 1u, 2u);
  y |= extractBits(index, 3u, 2u) << 1u;
  x |= extractBits(index, 5u, 1u) << 2u;
  return vec2i(i32(x), i32(y));
}

fn plane_bytes(code_word: u32) -> u32 {
  let lsbs = code_word & 0x5555u;
  var msbs = code_word & 0xaaaau;
  msbs |= msbs >> 1u;
  return countOneBits(lsbs) + countOneBits(msbs);
}

fn decode_quant(quant_code: u32) -> f32 {
  let e = 4 - i32(quant_code >> 3u);
  let m = i32(quant_code & 7u);
  return 1.1920928955078125e-07 * f32(8 + m) * exp2(f32(20 + e));
}

@compute @workgroup_size(128)
fn main(@builtin(local_invocation_index) li: u32,
        @builtin(workgroup_id) wg: vec3u${subgroups ? ',\n        @builtin(subgroup_invocation_id) sg_lane: u32,\n        @builtin(subgroup_size) sg_size: u32' : ''}) {
  let block32 = u32(registers.block_offset_32x32) + wg.y * u32(registers.block_stride_32x32) + wg.x;
  let sub = li & 7u;
  let bx = (li >> 3u) & 3u;
  let by = (li >> 5u) & 3u;
  let lb = by * 4u + bx;
  let coord = vec2i(wg.xy) * 32 + 8 * vec2i(i32(bx), i32(by)) + unswizzle8x8(sub << 3u);

  let off = offsets[block32];
  let missing = off == 0xffffffffu;
  let base = select(off, 0u, missing);
  let ballot = select(payload[base] & 0xffffu, 0u, missing);
  let q_code = payload[base + 1u] & 0xffu;
  let nb = countOneBits(ballot);

  if (li < 16u) {
    var cost = 0u;
    if (((ballot >> li) & 1u) != 0u) {
      let lco = countOneBits(low_bits(ballot, li));
      let cw = ld16(base * 2u + 4u + lco);
      let qb = ld8(base * 4u + 8u + nb * 2u + lco) & 15u;
      cost = plane_bytes(cw) + qb * 8u;
    }
    plane_costs[li] = cost;
  }
  workgroupBarrier();
  if (li < 16u) {
    var s = 0u;
    for (var k = 0u; k < li; k++) {
      s += plane_costs[k];
    }
    let start = base * 4u + 8u + 3u * nb + s;
    plane_offsets[li] = start;
    if (li == 15u) {
      sign_base = 8u * (start + plane_costs[15]);
    }
  }
  workgroupBarrier();

  var v: array<f32, 8>;
  var significant = 0u;
  if (((ballot >> lb) & 1u) != 0u) {
    let lco = countOneBits(low_bits(ballot, lb));
    let code_word = ld16(base * 2u + 4u + lco);
    let control2 = ld8(base * 4u + 8u + nb * 2u + lco);
    let q_bits = control2 & 15u;
    if (code_word != 0u) {
      let bit_offset = 2u * sub;
      let lsbs = code_word & 0x5555u;
      var msbs = code_word & 0xaaaau;
      msbs |= msbs >> 1u;
      var byte_offset = countOneBits(low_bits(lsbs, bit_offset)) + countOneBits(low_bits(msbs, bit_offset)) + q_bits * sub + plane_offsets[lb];
      let planes = i32(q_bits + extractBits(code_word, bit_offset, 2u));
      var mags: array<u32, 8>;
      for (var q = planes - 1; q >= 0; q--) {
        let p = ld8(byte_offset);
        byte_offset++;
        for (var b = 0u; b < 8u; b++) {
          mags[b] |= ((p >> b) & 1u) << u32(q);
        }
      }
      let scale = decode_quant(q_code) * (f32(extractBits(control2, 4u, 4u)) / 8.0 + 0.25);
      for (var k = 0u; k < 8u; k++) {
        var f = f32(mags[k]);
        if (f != 0.0) {
          f += 0.5;
          significant++;
        }
        v[k] = f * scale;
      }
    }
  }
${scan}

  let sign_offset = sign_base + excl;
  var sign_word = payload[sign_offset / 32u];
  let shift = sign_offset & 31u;
  if (shift != 0u) {
    sign_word = (sign_word >> shift) | (payload[sign_offset / 32u + 1u] << (32u - shift));
  }
  var counter = 0u;
  for (var k = 0u; k < 8u; k++) {
    if (v[k] != 0.0) {
      if (((sign_word >> counter) & 1u) != 0u) {
        v[k] = -v[k];
      }
      counter++;
    }
  }

  let dims = vec2i(textureDimensions(out_img));
  for (var j = 0; j < 2; j++) {
    for (var i = 0; i < 4; i++) {
      let c = coord + vec2i(i, j);
      if (all(c < dims)) {
        textureStore(out_img, c, registers.output_layer, vec4f(v[i * 2 + j]));
      }
    }
  }
}
`
}

/** Transformée inverse (CDF 9/7, relèvement) d'un niveau : 4 bandes en
 * entrée (couches 0 à 3, lues transposées avec miroir), un plan de taille
 * double en sortie. `dc_shift` : dernier niveau, + 0,5. */
export const idwtShader = /* wgsl */ `
struct Registers {
  resolution: vec2i,
  inv_resolution: vec2f,
  dc_shift: u32,
}

@group(0) @binding(0) var<uniform> registers: Registers;
@group(0) @binding(1) var in_tex: texture_2d_array<f32>;
@group(0) @binding(2) var mirror: sampler;
@group(0) @binding(3) var out_img: texture_storage_2d<r32float, write>;

const K = 1.23017406463623046875;
const INV_K = 0.812893092632293701171875;
const DELTA = 0.4435068666934967041015625;
const GAMMA = 0.88291108608245849609375;
const BETA = -0.052980117499828338623046875;
const ALPHA = -1.58613431453704833984375;

var<workgroup> shared_block: array<array<vec2f, 41>, 20>;

fn unswizzle8x8(index: u32) -> vec2i {
  var y = extractBits(index, 0u, 1u);
  var x = extractBits(index, 1u, 2u);
  y |= extractBits(index, 3u, 2u) << 1u;
  x |= extractBits(index, 5u, 1u) << 2u;
  return vec2i(i32(x), i32(y));
}

fn mirror_uv(coord_in: vec2i, even_x: bool, even_y: bool) -> vec2f {
  var coord = coord_in;
  let even = vec2<bool>(even_x, even_y);
  coord -= select(vec2i(0), vec2i(1), even & (coord < vec2i(0)));
  coord += vec2i(1);
  coord += select(vec2i(0), vec2i(1), !even & (coord >= registers.resolution));
  let uv = vec2f(coord) * registers.inv_resolution;
  return uv.yx;
}

fn gather(coord: vec2i, even_x: bool, even_y: bool, layer: i32) -> vec4f {
  return textureGather(0, in_tex, mirror, mirror_uv(coord, even_x, even_y), layer).wxzy;
}

fn write_shared_4x4(coord: vec2i, t0: vec4f, t1: vec4f, t2: vec4f, t3: vec4f) {
  let y = coord.y;
  let x = 2 * coord.x;
  shared_block[y][x + 0] = vec2f(t0.x, t2.x);
  shared_block[y][x + 1] = vec2f(t1.x, t3.x);
  shared_block[y][x + 2] = vec2f(t0.y, t2.y);
  shared_block[y][x + 3] = vec2f(t1.y, t3.y);
  shared_block[y + 1][x + 0] = vec2f(t0.z, t2.z);
  shared_block[y + 1][x + 1] = vec2f(t1.z, t3.z);
  shared_block[y + 1][x + 2] = vec2f(t0.w, t2.w);
  shared_block[y + 1][x + 3] = vec2f(t1.w, t3.w);
}

fn load_quad(base: vec2i, local: vec2i) {
  let c = base + local;
  write_shared_4x4(local, gather(c, true, true, 0), gather(c, false, true, 2), gather(c, true, false, 1), gather(c, false, false, 3));
}

fn inverse_transform8x2(li: u32) {
  let lc = vec2i(i32(8u * (li % 4u)), i32(li / 4u));
  var values: array<vec2f, 16>;
  for (var i = 0; i < 16; i += 2) {
    values[i] = shared_block[lc.y][lc.x + i] * K;
    values[i + 1] = shared_block[lc.y][lc.x + i + 1] * INV_K;
  }
  for (var i = 2; i < 15; i += 2) { values[i] -= DELTA * (values[i - 1] + values[i + 1]); }
  for (var i = 3; i < 14; i += 2) { values[i] -= GAMMA * (values[i - 1] + values[i + 1]); }
  for (var i = 4; i < 13; i += 2) { values[i] -= BETA * (values[i - 1] + values[i + 1]); }
  for (var i = 5; i < 12; i += 2) { values[i] -= ALPHA * (values[i - 1] + values[i + 1]); }
  workgroupBarrier();
  for (var i = 2; i < 6; i++) {
    let a = values[2 * i];
    let b = values[2 * i + 1];
    let y = (lc.x >> 1u) + (i - 2);
    shared_block[y][2 * lc.y] = vec2f(a.x, b.x);
    shared_block[y][2 * lc.y + 1] = vec2f(a.y, b.y);
  }
}

fn inverse_transform4x2(li: u32, lane_active: bool, y_offset: i32) {
  let lc = vec2i(i32(4u * (li % 8u)), i32(li / 8u) + y_offset);
  var values: array<vec2f, 12>;
  if (lane_active) {
    for (var i = 0; i < 12; i += 2) {
      values[i] = shared_block[lc.y][lc.x + i] * K;
      values[i + 1] = shared_block[lc.y][lc.x + i + 1] * INV_K;
    }
    for (var i = 2; i < 11; i += 2) { values[i] -= DELTA * (values[i - 1] + values[i + 1]); }
    for (var i = 3; i < 10; i += 2) { values[i] -= GAMMA * (values[i - 1] + values[i + 1]); }
    for (var i = 4; i < 9; i += 2) { values[i] -= BETA * (values[i - 1] + values[i + 1]); }
    for (var i = 5; i < 8; i += 2) { values[i] -= ALPHA * (values[i - 1] + values[i + 1]); }
  }
  workgroupBarrier();
  if (lane_active) {
    for (var i = 2; i < 4; i++) {
      let a = values[2 * i];
      let b = values[2 * i + 1];
      let y = (lc.x >> 1u) + (i - 2);
      shared_block[y][2 * lc.y] = vec2f(a.x, b.x);
      shared_block[y][2 * lc.y + 1] = vec2f(a.y, b.y);
    }
  }
}

@compute @workgroup_size(64)
fn main(@builtin(local_invocation_index) li: u32, @builtin(workgroup_id) wg: vec3u) {
  let base = vec2i(wg.xy) * 16 - vec2i(2);
  load_quad(base, 2 * unswizzle8x8(li));
  let horiz = vec2i(i32(16u + 2u * (li % 2u)), i32(2u * (li / 2u)));
  if (horiz.y < 20) {
    load_quad(base, horiz);
  }
  let vert = horiz.yx;
  if (vert.x < 16) {
    load_quad(base, vert);
  }
  workgroupBarrier();

  inverse_transform8x2(li);
  inverse_transform4x2(li, li < 32u, 16);
  workgroupBarrier();
  inverse_transform8x2(li);
  workgroupBarrier();

  let dims = vec2i(textureDimensions(out_img));
  let lc = unswizzle8x8(li);
  let origin = 32 * vec2i(wg.yx);
  for (var y = lc.y; y < 16; y += 8) {
    for (var x = lc.x; x < 32; x += 8) {
      var v = shared_block[y][x];
      if (registers.dc_shift != 0u) {
        v += vec2f(0.5);
      }
      let c0 = vec2i(2 * y, x) + origin;
      let c1 = vec2i(2 * y + 1, x) + origin;
      if (all(c0 < dims)) { textureStore(out_img, c0, vec4f(v.x)); }
      if (all(c1 < dims)) { textureStore(out_img, c1, vec4f(v.y)); }
    }
  }
}
`

/** Y, Cb, Cr (0 à 1) → RGB de la toile : BT.709 (SDR) ou BT.2020 PQ (HDR10,
 * ramené en SDR BT.709 : la toile du client web est SDR), plage pleine ou
 * réduite ; chroma 4:2:0 interpolée (échantillons centrés). */
export const colorShader = /* wgsl */ `
struct Color {
  chroma420: u32,
  full_range: u32,
  hdr: u32,
  pad: u32,
}

@group(0) @binding(0) var<uniform> color: Color;
@group(0) @binding(1) var plane_y: texture_2d<f32>;
@group(0) @binding(2) var plane_cb: texture_2d<f32>;
@group(0) @binding(3) var plane_cr: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
  return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}

fn chroma_at(t: texture_2d<f32>, p: vec2i) -> f32 {
  let d = vec2i(textureDimensions(t)) - vec2i(1);
  return clamp(textureLoad(t, clamp(p, vec2i(0), d), 0).r, 0.0, 1.0);
}

fn chroma(t: texture_2d<f32>, xy: vec2i) -> f32 {
  if (color.chroma420 == 0u) {
    return chroma_at(t, xy);
  }
  let pos = (vec2f(xy) + 0.5) * 0.5 - 0.5;
  let i0 = vec2i(floor(pos));
  let f = pos - floor(pos);
  let a = mix(chroma_at(t, i0), chroma_at(t, i0 + vec2i(1, 0)), f.x);
  let b = mix(chroma_at(t, i0 + vec2i(0, 1)), chroma_at(t, i0 + vec2i(1, 1)), f.x);
  return mix(a, b, f.y);
}

fn pq_eotf(e: vec3f) -> vec3f {
  let m1 = 0.1593017578125;
  let m2 = 78.84375;
  let c1 = 0.8359375;
  let c2 = 18.8515625;
  let c3 = 18.6875;
  let p = pow(max(e, vec3f(0.0)), vec3f(1.0 / m2));
  return pow(max(p - c1, vec3f(0.0)) / (c2 - c3 * p), vec3f(1.0 / m1));
}

fn srgb_oetf(l: vec3f) -> vec3f {
  let c = clamp(l, vec3f(0.0), vec3f(1.0));
  return select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, 12.92 * c, c <= vec3f(0.0031308));
}

@fragment
fn fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  let xy = vec2i(pos.xy);
  var y = clamp(textureLoad(plane_y, xy, 0).r, 0.0, 1.0);
  var cb = chroma(plane_cb, xy);
  var cr = chroma(plane_cr, xy);
  if (color.hdr != 0u) {
    // P010 lu en R16_UNORM par la Station : code 10 bits = v · 65535 / 64.
    let k = 65535.0 / 64.0;
    if (color.full_range != 0u) {
      y = y * k / 1023.0;
      cb = cb * k / 1023.0 - 0.5;
      cr = cr * k / 1023.0 - 0.5;
    } else {
      y = (y * k - 64.0) / 876.0;
      cb = (cb * k - 512.0) / 896.0;
      cr = (cr * k - 512.0) / 896.0;
    }
    let rgb2020 = vec3f(y + 1.4746 * cr, y - 0.16455 * cb - 0.57135 * cr, y + 1.8814 * cb);
    // PQ → lumière (1,0 = 10 000 cd/m²), blanc SDR à 203 cd/m², puis BT.2020 → BT.709
    // et compression douce des hautes lumières.
    var lin = pq_eotf(rgb2020) * (10000.0 / 203.0);
    lin = vec3f(
      1.6605 * lin.r - 0.5876 * lin.g - 0.0728 * lin.b,
      -0.1246 * lin.r + 1.1329 * lin.g - 0.0083 * lin.b,
      -0.0182 * lin.r - 0.1006 * lin.g + 1.1187 * lin.b);
    lin = max(lin, vec3f(0.0));
    lin = lin / (1.0 + max(lin - vec3f(0.8), vec3f(0.0)));
    return vec4f(srgb_oetf(lin), 1.0);
  }
  if (color.full_range != 0u) {
    // Milieu de chroma de l'encodeur : 0,5.
    cb -= 0.5;
    cr -= 0.5;
  } else {
    y = (y * 255.0 - 16.0) / 219.0;
    cb = (cb * 255.0 - 128.0) / 224.0;
    cr = (cr * 255.0 - 128.0) / 224.0;
  }
  let rgb = vec3f(y + 1.5748 * cr, y - 0.18733 * cb - 0.46813 * cr, y + 1.8556 * cb);
  return vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)), 1.0);
}
`
