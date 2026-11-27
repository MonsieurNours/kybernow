// Micro de l'appareil vers l'ordinateur (docs/micro.md, docs/link-ui.md
// « Micro ») : capte le micro du navigateur (getUserMedia), l'encode en Opus
// par WebCodecs (mono, 48 kHz, trames de 10 ms, 32 kbit/s, correction
// d'erreurs intégrée pour 10 % de pertes) et donne chaque paquet au moteur.
// Le choix allumé ou coupé est retenu pour chaque ordinateur (coupé à la
// première session) ; le micro choisi et « Réduire l'écho et le bruit » sont
// des réglages de l'app (prefs.ts).
//
// Rien ne s'affiche sans les deux : un navigateur qui encode l'Opus
// (AudioEncoder) et un moteur qui sait envoyer le micro
// (`engineSendsMicrophone`, faux tant que le moteur web n'a pas le flux
// montant).

import { engineSendsMicrophone } from './engine'

const CHOICES_KEY = 'kybernow.link.micro'

/** Ce que dit l'ordinateur du micro (états du cœur, docs/micro.md). */
export type MicRefusal = 'not_allowed' | 'no_device' | 'unsupported' | 'old_station'

export const OPUS_CONFIG = {
  codec: 'opus',
  sampleRate: 48000,
  numberOfChannels: 1,
  bitrate: 32000,
  opus: { frameDuration: 10_000, useinbandfec: true, packetlossperc: 10, complexity: 9 },
}

function loadChoices(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(CHOICES_KEY)
    const v = raw ? JSON.parse(raw) : null
    if (v && typeof v === 'object') return v
  } catch {
    // Stockage indisponible : coupé à chaque session.
  }
  return {}
}

/** `browser` : ce navigateur encode l'Opus (connu après `probeMicrophone`). `choices` : allumé
 * ou coupé, par ordinateur (son nom). */
export const micro = $state({ browser: false, choices: loadChoices() })

let probing: Promise<boolean> | null = null

/** Ce navigateur sait-il capter le micro et l'encoder en Opus ? Une fois par page. */
export function probeMicrophone(): Promise<boolean> {
  if (!probing) {
    probing = (async () => {
      try {
        if (!navigator.mediaDevices?.getUserMedia || typeof AudioEncoder === 'undefined' || typeof AudioWorkletNode === 'undefined') return false
        const r = await AudioEncoder.isConfigSupported(OPUS_CONFIG as AudioEncoderConfig)
        return !!r.supported
      } catch {
        return false
      }
    })().then((ok) => (micro.browser = ok))
  }
  return probing
}

/** « Micro » proposé : ce navigateur et le moteur savent l'envoyer. Sinon, ni puce ni réglage. */
export function micAvailable(): boolean {
  return micro.browser && engineSendsMicrophone()
}

/** Le dernier choix pour cet ordinateur (coupé la première fois). */
export function micOn(host: string): boolean {
  return micro.choices[host] === true
}

export function setMicOn(host: string, on: boolean) {
  micro.choices[host] = on
  try {
    localStorage.setItem(CHOICES_KEY, JSON.stringify(micro.choices))
  } catch {
    // Sans stockage, le choix vaut pour cette visite.
  }
}

export interface MicDevice {
  id: string
  label: string
}

/** Les micros de l'appareil (leurs noms ne sont connus qu'une fois le micro autorisé). */
export async function listMicrophones(): Promise<MicDevice[]> {
  try {
    const all = await navigator.mediaDevices.enumerateDevices()
    return all
      .filter((d) => d.kind === 'audioinput' && d.deviceId && d.deviceId !== 'default' && d.deviceId !== 'communications')
      .map((d, i) => ({ id: d.deviceId, label: d.label || `Micro ${i + 1}` }))
  } catch {
    return []
  }
}

/** `denied` : le navigateur n'a pas l'autorisation ; `none` : pas de micro ; `failed` : le reste. */
/** `cancelled` : arrêtée par `stop` pendant la demande d'autorisation. */
export type MicStart = 'ok' | 'denied' | 'none' | 'failed' | 'cancelled'

/**
 * Une capture du micro, de `start` à `stop`. `onPacket` reçoit chaque paquet
 * Opus et son heure de capture (µs) ; un paquet vide à l'arrêt dit à
 * l'ordinateur que le micro est coupé.
 */
export class MicCapture {
  private stream: MediaStream | null = null
  private ctx: AudioContext | null = null
  private node: AudioWorkletNode | null = null
  private encoder: AudioEncoder | null = null
  private frames = 0
  private origin = 0
  private running = false

  constructor(private onPacket: (packet: Uint8Array, ptsUs: number) => void) {}

  get active(): boolean {
    return this.running
  }

  async start(deviceId: string, processing: boolean): Promise<MicStart> {
    this.stop(false)
    this.running = true
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          // Un micro choisi absent : celui du système, sans perdre le choix (link-ui.md).
          ...(deviceId ? { deviceId: { ideal: deviceId } } : {}),
          echoCancellation: processing,
          noiseSuppression: processing,
          autoGainControl: processing,
          channelCount: 1,
        },
      })
    } catch (e) {
      this.running = false
      const name = (e as DOMException)?.name
      return name === 'NotAllowedError' || name === 'SecurityError' ? 'denied' : name === 'NotFoundError' || name === 'OverconstrainedError' ? 'none' : 'failed'
    }
    if (!this.running) {
      // Arrêtée pendant la demande d'autorisation (la session est partie) : rien n'a échoué (WEB-13).
      this.release()
      return 'cancelled'
    }
    try {
      await this.pipeline()
      return 'ok'
    } catch (e) {
      console.warn('KyberNow : micro non démarré', e)
      this.stop(false)
      return 'failed'
    }
  }

  /** Le micro, à 48 kHz si le navigateur le permet (Firefox refuse un contexte à une autre
   * fréquence que celle du micro : on prend alors la sienne, l'encodeur la ramène). */
  private async pipeline() {
    let ctx = new AudioContext({ sampleRate: 48000, latencyHint: 'interactive' })
    let source: MediaStreamAudioSourceNode
    try {
      source = ctx.createMediaStreamSource(this.stream!)
    } catch {
      await ctx.close().catch(() => {})
      ctx = new AudioContext({ latencyHint: 'interactive' })
      source = ctx.createMediaStreamSource(this.stream!)
    }
    this.ctx = ctx
    const rate = ctx.sampleRate
    const config = { ...OPUS_CONFIG, sampleRate: rate } as AudioEncoderConfig
    if (!(await AudioEncoder.isConfigSupported(config)).supported) throw new Error(`Opus à ${rate} Hz refusé`)
    this.encoder = new AudioEncoder({
      output: (chunk) => {
        const data = new Uint8Array(chunk.byteLength)
        chunk.copyTo(data)
        if (this.running) this.onPacket(data, chunk.timestamp)
      },
      error: (e) => console.warn('KyberNow : encodeur du micro', e),
    })
    this.encoder.configure(config)
    await ctx.audioWorklet.addModule(new URL('./mic-worklet.js', import.meta.url))
    const node = new AudioWorkletNode(ctx, 'kybernow-mic', { numberOfInputs: 1, numberOfOutputs: 0, processorOptions: { rate } })
    this.node = node
    this.frames = 0
    this.origin = Math.round((performance.timeOrigin + performance.now()) * 1000)
    node.port.onmessage = (ev: MessageEvent<Float32Array<ArrayBuffer>>) => {
      const enc = this.encoder
      if (!this.running || !enc || enc.state !== 'configured') return
      const samples = ev.data
      const audio = new AudioData({
        format: 'f32',
        sampleRate: rate,
        numberOfFrames: samples.length,
        numberOfChannels: 1,
        timestamp: this.origin + Math.round((this.frames * 1_000_000) / rate),
        data: samples,
      })
      this.frames += samples.length
      enc.encode(audio)
      audio.close()
    }
    source.connect(node)
    if (ctx.state === 'suspended') await ctx.resume().catch(() => {})
  }

  private release() {
    this.stream?.getTracks().forEach((t) => t.stop())
    this.stream = null
  }

  /** Arrête la capture ; `silence` : dit tout de suite à l'ordinateur que le micro est coupé. */
  stop(silence = true) {
    const was = this.running
    this.running = false
    if (this.node) {
      this.node.port.onmessage = null
      this.node.disconnect()
      this.node = null
    }
    if (this.encoder && this.encoder.state !== 'closed') this.encoder.close()
    this.encoder = null
    this.ctx?.close().catch(() => {})
    this.ctx = null
    this.release()
    if (was && silence) this.onPacket(new Uint8Array(0), Math.round((performance.timeOrigin + performance.now()) * 1000))
  }
}
