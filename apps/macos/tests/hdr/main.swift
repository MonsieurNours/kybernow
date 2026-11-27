// Les courbes du HDR de KyberKit (Metal, sur le GPU) contre celles du cœur.
import CKyberNow
import Foundation
import Metal

let device = MTLCreateSystemDefaultDevice()!
let source = """
#include <metal_stdlib>
using namespace metal;
\(HDRShader.functions)
kernel void kn_hdr_probe(device const float *in [[buffer(0)]],
                         device float *eotf [[buffer(1)]],
                         device float *curve [[buffer(2)]],
                         device float3 *matrix [[buffer(3)]],
                         uint i [[thread_position_in_grid]]) {
    eotf[i] = kn_pq_eotf(float3(in[i])).r;
    curve[i] = kn_sdr_curve(in[i] * 5.0);
    if (i < 3) {
        float3 unit = float3(i == 0, i == 1, i == 2);
        matrix[i] = kn_bt2020_to_709(unit);
    }
}
"""
let library = try! device.makeLibrary(source: source, options: nil)
let pipeline = try! device.makeComputePipelineState(function: library.makeFunction(name: "kn_hdr_probe")!)
let n = 2001
let grid = (0..<n).map { Float($0) / Float(n - 1) }
let input = device.makeBuffer(bytes: grid, length: n * 4)!
let eotf = device.makeBuffer(length: n * 4)!
let curve = device.makeBuffer(length: n * 4)!
let matrix = device.makeBuffer(length: 3 * 16)!
let queue = device.makeCommandQueue()!
let commands = queue.makeCommandBuffer()!
let pass = commands.makeComputeCommandEncoder()!
pass.setComputePipelineState(pipeline)
pass.setBuffer(input, offset: 0, index: 0)
pass.setBuffer(eotf, offset: 0, index: 1)
pass.setBuffer(curve, offset: 0, index: 2)
pass.setBuffer(matrix, offset: 0, index: 3)
pass.dispatchThreads(MTLSize(width: n, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 64, height: 1, depth: 1))
pass.endEncoding()
commands.commit()
commands.waitUntilCompleted()

var failures = 0
func check(_ what: String, _ got: Double, _ want: Double, tolerance: Double) {
    guard abs(got - want) > tolerance else { return }
    failures += 1
    if failures <= 10 { print("ÉCART \(what) : shader \(got), cœur \(want)") }
}

// PQ : en unités de 10 000 nits (1e-4 = 1 nit) ; en relatif au-dessus d'1 nit.
let e = eotf.contents().bindMemory(to: Float.self, capacity: n)
let c = curve.contents().bindMemory(to: Float.self, capacity: n)
var worstPQ = 0.0, worstCurve = 0.0
for i in 0..<n {
    let x = Double(grid[i])
    let want = kn_hdr_pq_to_nits(x) / 10_000
    let got = Double(e[i])
    worstPQ = max(worstPQ, abs(got - want) / max(want, 1e-4))
    check("PQ(\(x))", got, want, tolerance: max(1e-4 * want, 1e-6))
    let m = x * 5
    let wantCurve = kn_hdr_sdr_curve(m)
    worstCurve = max(worstCurve, abs(Double(c[i]) - wantCurve))
    check("courbe SDR(\(m))", Double(c[i]), wantCurve, tolerance: 1e-4)
}
// Matrice : colonnes rendues par le shader pour les trois vecteurs unité.
var core = [Double](repeating: 0, count: 9)
core.withUnsafeMutableBufferPointer { kn_hdr_bt2020_to_709($0.baseAddress) }
let cols = matrix.contents().bindMemory(to: SIMD4<Float>.self, capacity: 3)
for col in 0..<3 {
    for row in 0..<3 {
        check("BT.2020→709[\(row)][\(col)]", Double(cols[col][row]), core[row * 3 + col], tolerance: 1e-5)
    }
}
print(String(format: "PQ : écart relatif au plus %.2e ; courbe vers le SDR : écart au plus %.2e ; matrice BT.2020→709 : %@ (%d points)",
             worstPQ, worstCurve, failures == 0 ? "identique" : "ÉCART", n))
if failures > 0 {
    print("\(failures) écart(s) au-delà de 1e-4")
    exit(1)
}
print("hdr : tout passe")
