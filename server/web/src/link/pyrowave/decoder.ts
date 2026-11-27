// Décodeur PyroWave en WebGPU (docs/pyrowave-client-web.md) : lecture du flux
// sur le processeur (bitstream.ts), puis sur la carte graphique la
// déquantification des blocs, la transformée inverse sur 5 niveaux et la
// conversion en RGB dans une toile. Même découpage que le décodeur de l'amont
// (pyrowave_decoder.cpp) et que son port Metal.

import { BitstreamParser, BlockLayout, DecompositionLevels, NumComponents, type PyroWaveConfig } from './bitstream'
import { colorShader, dequantShader, idwtShader } from './shaders'

// Constantes de WebGPU (les espaces de noms GPUShaderStage, GPUBufferUsage… ne
// sont pas déclarés par TypeScript) ; valeurs de la spécification.
const Stage = { FRAGMENT: 2, COMPUTE: 4 }
const TextureUsage = { COPY_SRC: 1, TEXTURE_BINDING: 4, STORAGE_BINDING: 8 }
const BufferUsage = { MAP_READ: 1, COPY_SRC: 4, COPY_DST: 8, UNIFORM: 64, STORAGE: 128, QUERY_RESOLVE: 512 }
const MapRead = 1

/** Le device WebGPU et les pipelines, partagés par les décodeurs. */
export class PyroWaveGPU {
  readonly dequantLayout: GPUBindGroupLayout
  readonly idwtLayout: GPUBindGroupLayout
  readonly colorLayout: GPUBindGroupLayout
  readonly dequant: GPUComputePipeline
  readonly idwt: GPUComputePipeline
  readonly mirror: GPUSampler
  private colorPipelines = new Map<GPUTextureFormat, GPURenderPipeline>()
  lost = false

  private constructor(
    readonly device: GPUDevice,
    /** La variante à sous-groupes de la déquantification est prise. */
    readonly subgroups: boolean,
    readonly timestamps: boolean,
    readonly adapterName: string,
  ) {
    const d = device
    this.dequantLayout = d.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: Stage.COMPUTE, buffer: { type: 'read-only-storage' } },
        { binding: 1, visibility: Stage.COMPUTE, buffer: { type: 'read-only-storage' } },
        { binding: 2, visibility: Stage.COMPUTE, buffer: { type: 'uniform' } },
        { binding: 3, visibility: Stage.COMPUTE, storageTexture: { access: 'write-only', format: 'r32float', viewDimension: '2d-array' } },
      ],
    })
    this.idwtLayout = d.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: Stage.COMPUTE, buffer: { type: 'uniform' } },
        { binding: 1, visibility: Stage.COMPUTE, texture: { sampleType: 'unfilterable-float', viewDimension: '2d-array' } },
        { binding: 2, visibility: Stage.COMPUTE, sampler: { type: 'non-filtering' } },
        { binding: 3, visibility: Stage.COMPUTE, storageTexture: { access: 'write-only', format: 'r32float', viewDimension: '2d' } },
      ],
    })
    this.colorLayout = d.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: Stage.FRAGMENT, buffer: { type: 'uniform' } },
        { binding: 1, visibility: Stage.FRAGMENT, texture: { sampleType: 'unfilterable-float' } },
        { binding: 2, visibility: Stage.FRAGMENT, texture: { sampleType: 'unfilterable-float' } },
        { binding: 3, visibility: Stage.FRAGMENT, texture: { sampleType: 'unfilterable-float' } },
      ],
    })
    this.dequant = d.createComputePipeline({
      layout: d.createPipelineLayout({ bindGroupLayouts: [this.dequantLayout] }),
      compute: { module: d.createShaderModule({ code: dequantShader(subgroups) }), entryPoint: 'main' },
    })
    this.idwt = d.createComputePipeline({
      layout: d.createPipelineLayout({ bindGroupLayouts: [this.idwtLayout] }),
      compute: { module: d.createShaderModule({ code: idwtShader }), entryPoint: 'main' },
    })
    this.mirror = d.createSampler({
      addressModeU: 'mirror-repeat',
      addressModeV: 'mirror-repeat',
      magFilter: 'nearest',
      minFilter: 'nearest',
    })
    device.lost.then(() => (this.lost = true))
  }

  /** `subgroups` : `false` pour forcer la variante sans sous-groupes (repli,
   * mesures) ; sinon prise quand le navigateur a la fonctionnalité. Null sans
   * WebGPU, sans adaptateur ou si les shaders ne se compilent pas. */
  static async create(opts: { subgroups?: boolean; timestamps?: boolean; gpu?: GPU } = {}): Promise<PyroWaveGPU | null> {
    const gpu = opts.gpu ?? (typeof navigator !== 'undefined' ? navigator.gpu : undefined)
    if (!gpu) return null
    const adapter = await gpu.requestAdapter({ powerPreference: 'high-performance' }).catch(() => null)
    if (!adapter) return null
    const wantSubgroups = opts.subgroups !== false && adapter.features.has('subgroups')
    const wantTimestamps = !!opts.timestamps && adapter.features.has('timestamp-query')
    const requiredFeatures: GPUFeatureName[] = []
    if (wantSubgroups) requiredFeatures.push('subgroups')
    if (wantTimestamps) requiredFeatures.push('timestamp-query')
    const device = await adapter.requestDevice({ requiredFeatures }).catch(() => null)
    if (!device) return null
    const info: any = (adapter as any).info ?? {}
    const name = [info.vendor, info.architecture, info.device, info.description].filter((s) => s).join(' ')
    device.pushErrorScope('validation')
    let created: PyroWaveGPU
    try {
      created = new PyroWaveGPU(device, wantSubgroups, wantTimestamps, name)
    } catch {
      device.destroy()
      return null
    }
    const error = await device.popErrorScope()
    if (error) {
      console.warn('KyberNow : PyroWave, shaders refusés', error.message)
      device.destroy()
      return null
    }
    return created
  }

  colorPipeline(format: GPUTextureFormat): GPURenderPipeline {
    let p = this.colorPipelines.get(format)
    if (!p) {
      const module = this.device.createShaderModule({ code: colorShader })
      p = this.device.createRenderPipeline({
        layout: this.device.createPipelineLayout({ bindGroupLayouts: [this.colorLayout] }),
        vertex: { module, entryPoint: 'vs' },
        fragment: { module, entryPoint: 'fs', targets: [{ format }] },
        primitive: { topology: 'triangle-list' },
      })
      this.colorPipelines.set(format, p)
    }
    return p
  }

  destroy() {
    this.device.destroy()
  }
}

export type DecodeStatus = 'decoded' | 'incomplete' | 'corrupt' | 'busy'

export interface Submitted {
  status: DecodeStatus
  /** Image décodée (et dessinée) par la carte graphique. */
  done?: Promise<void>
}

const UniformStride = 256

/** Un décodeur pour une configuration (définition, chroma, couleur, plage). */
export class PyroWaveDecoder {
  readonly layout: BlockLayout
  private parser: BitstreamParser
  private levels: (GPUTexture | null)[][] = []
  readonly planes: GPUTexture[]
  private uniforms: GPUBuffer
  private offsetsBuffer: GPUBuffer
  private payloadBuffer: GPUBuffer
  private dequantDispatches: { group: GPUBindGroup | null; view: GPUTextureView; offset: number; x: number; y: number }[] = []
  private idwtDispatches: { group: GPUBindGroup; x: number; y: number }[][] = []
  private colorUniform: GPUBuffer
  private colorGroup: GPUBindGroup
  private queryset: GPUQuerySet | null = null
  private queryResolve: GPUBuffer | null = null
  inFlight = 0
  /** Au plus tant d'images sur la carte graphique : au-delà, l'image est sautée. */
  static maxInFlight = 3
  /** Temps de la carte graphique de la dernière image mesurée (`timestamp-query`), en ms. */
  lastGpuMs: number | null = null

  constructor(
    private gpu: PyroWaveGPU,
    readonly config: PyroWaveConfig,
  ) {
    const d = gpu.device
    const c444 = config.chroma444
    this.layout = new BlockLayout(config.width, config.height, c444)
    this.parser = new BitstreamParser(this.layout)
    const L = this.layout
    const usage = TextureUsage.STORAGE_BINDING | TextureUsage.TEXTURE_BINDING
    for (let c = 0; c < NumComponents; c++) {
      this.levels.push([])
      for (let level = 0; level < DecompositionLevels; level++) {
        if (level === 0 && c !== 0 && !c444) {
          this.levels[c].push(null)
          continue
        }
        this.levels[c].push(d.createTexture({ size: [L.levelWidth(level), L.levelHeight(level), 4], format: 'r32float', usage }))
      }
    }
    const cw = c444 ? config.width : config.width / 2
    const ch = c444 ? config.height : config.height / 2
    const planeUsage = usage | TextureUsage.COPY_SRC
    this.planes = [
      d.createTexture({ size: [config.width, config.height], format: 'r32float', usage: planeUsage }),
      d.createTexture({ size: [cw, ch], format: 'r32float', usage: planeUsage }),
      d.createTexture({ size: [cw, ch], format: 'r32float', usage: planeUsage }),
    ]

    // Paramètres de chaque passe, une tranche de 256 octets chacune.
    const dq: { level: number; c: number; band: number }[] = []
    for (let level = 0; level < DecompositionLevels; level++)
      for (let c = 0; c < NumComponents; c++) {
        if (level === 0 && c !== 0 && !c444) continue
        for (let band = level === DecompositionLevels - 1 ? 0 : 1; band < 4; band++) dq.push({ level, c, band })
      }
    const idwtCount = DecompositionLevels * NumComponents
    this.uniforms = d.createBuffer({ size: (dq.length + idwtCount) * UniformStride, usage: BufferUsage.UNIFORM | BufferUsage.COPY_DST })
    const data = new ArrayBuffer((dq.length + idwtCount) * UniformStride)
    const view = new DataView(data)
    dq.forEach((e, i) => {
      const meta = L.meta[e.c][e.level][e.band]!
      const o = i * UniformStride
      view.setInt32(o, L.levelWidth(e.level), true)
      view.setInt32(o + 4, L.levelHeight(e.level), true)
      view.setInt32(o + 8, e.band, true)
      view.setInt32(o + 12, meta.offset32, true)
      view.setInt32(o + 16, meta.stride32, true)
      this.dequantDispatches.push({
        group: null,
        view: this.levels[e.c][e.level]!.createView({ dimension: '2d-array' }),
        offset: o,
        x: (L.levelWidth(e.level) + 31) >> 5,
        y: (L.levelHeight(e.level) + 31) >> 5,
      })
    })
    let slot = dq.length
    for (let level = DecompositionLevels - 1; level >= 0; level--) {
      const list: { group: GPUBindGroup; x: number; y: number }[] = []
      const rx = L.levelHeight(level)
      const ry = L.levelWidth(level)
      for (let c = 0; c < NumComponents; c++) {
        const input = this.levels[c][level]
        if (!input) continue
        let output: GPUTextureView
        let dc = false
        if (level === 0) {
          output = this.planes[c].createView()
          dc = true
        } else if (!c444 && c !== 0 && level === 1) {
          output = this.planes[c].createView()
          dc = true
        } else {
          output = this.levels[c][level - 1]!.createView({ dimension: '2d', baseArrayLayer: 0, arrayLayerCount: 1 })
        }
        const o = slot++ * UniformStride
        view.setInt32(o, rx, true)
        view.setInt32(o + 4, ry, true)
        view.setFloat32(o + 8, 1 / rx, true)
        view.setFloat32(o + 12, 1 / ry, true)
        view.setUint32(o + 16, dc ? 1 : 0, true)
        list.push({
          group: d.createBindGroup({
            layout: gpu.idwtLayout,
            entries: [
              { binding: 0, resource: { buffer: this.uniforms, offset: o, size: 24 } },
              { binding: 1, resource: input.createView({ dimension: '2d-array' }) },
              { binding: 2, resource: gpu.mirror },
              { binding: 3, resource: output },
            ],
          }),
          x: (rx + 15) >> 4,
          y: (ry + 15) >> 4,
        })
      }
      this.idwtDispatches.push(list)
    }
    d.queue.writeBuffer(this.uniforms, 0, data)

    this.offsetsBuffer = d.createBuffer({ size: Math.max(16, L.blockCount32 * 4), usage: BufferUsage.STORAGE | BufferUsage.COPY_DST })
    this.payloadBuffer = this.makePayload(1 << 20)

    this.colorUniform = d.createBuffer({ size: 16, usage: BufferUsage.UNIFORM | BufferUsage.COPY_DST })
    d.queue.writeBuffer(this.colorUniform, 0, new Uint32Array([c444 ? 0 : 1, config.fullRange ? 1 : 0, config.hdr ? 1 : 0, 0]))
    this.colorGroup = d.createBindGroup({
      layout: gpu.colorLayout,
      entries: [
        { binding: 0, resource: { buffer: this.colorUniform } },
        { binding: 1, resource: this.planes[0].createView() },
        { binding: 2, resource: this.planes[1].createView() },
        { binding: 3, resource: this.planes[2].createView() },
      ],
    })
    if (gpu.timestamps) {
      this.queryset = d.createQuerySet({ type: 'timestamp', count: 2 })
      this.queryResolve = d.createBuffer({ size: 16, usage: BufferUsage.QUERY_RESOLVE | BufferUsage.COPY_SRC })
    }
  }

  private makePayload(bytes: number): GPUBuffer {
    // Le shader lit un mot de signes au-delà de la fin : marge.
    const buffer = this.gpu.device.createBuffer({ size: bytes + 64, usage: BufferUsage.STORAGE | BufferUsage.COPY_DST })
    for (const dq of this.dequantDispatches) dq.group = null
    return buffer
  }

  private dequantGroup(i: number): GPUBindGroup {
    const dq = this.dequantDispatches[i]
    if (!dq.group) {
      dq.group = this.gpu.device.createBindGroup({
        layout: this.gpu.dequantLayout,
        entries: [
          { binding: 0, resource: { buffer: this.payloadBuffer } },
          { binding: 1, resource: { buffer: this.offsetsBuffer } },
          { binding: 2, resource: { buffer: this.uniforms, offset: dq.offset, size: 24 } },
          { binding: 3, resource: dq.view },
        ],
      })
    }
    return dq.group
  }

  /** Une image entière (paquets PyroWave bout à bout). Lue sur le processeur,
   * puis décodée par la carte graphique ; `target` : la toile où la dessiner.
   * Une image incomplète (moins de 90 % des blocs ou bandes basses manquantes)
   * ou illisible est sautée ; rien n'est demandé à l'ordinateur. */
  submit(data: Uint8Array, target?: GPUCanvasContext, format?: GPUTextureFormat): Submitted {
    if (this.inFlight >= PyroWaveDecoder.maxInFlight) return { status: 'busy' }
    const p = this.parser
    p.clear()
    if (p.push(data) !== 'ok') return { status: 'corrupt' }
    if (!p.ready()) return { status: 'incomplete' }
    p.markDecoded()

    const d = this.gpu.device
    const payload = p.payload
    if (payload.byteLength + 64 > this.payloadBuffer.size) {
      this.payloadBuffer.destroy()
      this.payloadBuffer = this.makePayload(Math.ceil((payload.byteLength * 1.5) / 4) * 4)
    }
    d.queue.writeBuffer(this.offsetsBuffer, 0, p.offsets)
    d.queue.writeBuffer(this.payloadBuffer, 0, payload)

    const enc = d.createCommandEncoder()
    const timestampWrites = this.queryset ? { querySet: this.queryset, beginningOfPassWriteIndex: 0, endOfPassWriteIndex: 1 } : undefined
    const pass = enc.beginComputePass(timestampWrites ? { timestampWrites } : {})
    pass.setPipeline(this.gpu.dequant)
    for (let i = 0; i < this.dequantDispatches.length; i++) {
      const dq = this.dequantDispatches[i]
      pass.setBindGroup(0, this.dequantGroup(i))
      pass.dispatchWorkgroups(dq.x, dq.y)
    }
    pass.setPipeline(this.gpu.idwt)
    for (const level of this.idwtDispatches)
      for (const e of level) {
        pass.setBindGroup(0, e.group)
        pass.dispatchWorkgroups(e.x, e.y)
      }
    pass.end()
    let readback: GPUBuffer | null = null
    if (this.queryset && this.queryResolve) {
      enc.resolveQuerySet(this.queryset, 0, 2, this.queryResolve, 0)
      readback = d.createBuffer({ size: 16, usage: BufferUsage.COPY_DST | BufferUsage.MAP_READ })
      enc.copyBufferToBuffer(this.queryResolve, 0, readback, 0, 16)
    }
    if (target) this.encodeColor(enc, target, format)
    d.queue.submit([enc.finish()])
    this.inFlight++
    const done = d.queue.onSubmittedWorkDone().finally(() => this.inFlight--)
    if (readback) {
      const rb = readback
      rb.mapAsync(MapRead)
        .then(() => {
          const t = new BigUint64Array(rb.getMappedRange())
          this.lastGpuMs = Number(t[1] - t[0]) / 1e6
          rb.unmap()
        })
        .catch(() => {})
        // Détruit aussi quand la lecture échoue (device perdu : WEB-02).
        .finally(() => rb.destroy())
    }
    return { status: 'decoded', done }
  }

  /** YCbCr → RGB dans la toile (à la taille de l'image). */
  private encodeColor(enc: GPUCommandEncoder, target: GPUCanvasContext, format?: GPUTextureFormat) {
    const tex = target.getCurrentTexture()
    const pass = enc.beginRenderPass({
      colorAttachments: [{ view: tex.createView(), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }],
    })
    pass.setPipeline(this.gpu.colorPipeline(format ?? tex.format))
    pass.setBindGroup(0, this.colorGroup)
    pass.draw(3)
    pass.end()
  }

  /** Relit les plans Y, Cb, Cr (valeurs PyroWave, 0 à 1) : essais et sonde. */
  async readPlanes(): Promise<Float32Array[]> {
    const d = this.gpu.device
    const out: Float32Array[] = []
    const jobs: { buf: GPUBuffer; w: number; h: number; row: number }[] = []
    const enc = d.createCommandEncoder()
    for (const t of this.planes) {
      const row = Math.ceil((t.width * 4) / 256) * 256
      const buf = d.createBuffer({ size: row * t.height, usage: BufferUsage.COPY_DST | BufferUsage.MAP_READ })
      enc.copyTextureToBuffer({ texture: t }, { buffer: buf, bytesPerRow: row }, [t.width, t.height])
      jobs.push({ buf, w: t.width, h: t.height, row })
    }
    d.queue.submit([enc.finish()])
    for (const j of jobs) {
      await j.buf.mapAsync(MapRead)
      const src = new Float32Array(j.buf.getMappedRange())
      const dst = new Float32Array(j.w * j.h)
      for (let y = 0; y < j.h; y++) dst.set(src.subarray((y * j.row) / 4, (y * j.row) / 4 + j.w), y * j.w)
      j.buf.unmap()
      j.buf.destroy()
      out.push(dst)
    }
    return out
  }

  destroy() {
    for (const list of this.levels) for (const t of list) t?.destroy()
    for (const t of this.planes) t.destroy()
    this.uniforms.destroy()
    this.offsetsBuffer.destroy()
    this.payloadBuffer.destroy()
    this.colorUniform.destroy()
    this.queryset?.destroy()
    this.queryResolve?.destroy()
  }
}
