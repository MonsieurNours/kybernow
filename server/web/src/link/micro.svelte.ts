// Micro de l'appareil vers l'ordinateur (docs/micro.md, docs/link-ui.md
// « Micro ») : ce que ce navigateur sait faire, le choix allumé ou coupé
// retenu pour chaque ordinateur (coupé à la première session), et « Micro »
// pendant la session (`SessionMic`) : la capture (miccapture.ts, chargée à la
// demande) suit le choix, l'état que dit l'ordinateur et ses avis. Le micro
// choisi et « Réduire l'écho et le bruit » sont des réglages de l'app
// (prefs.ts).
//
// Rien ne s'affiche ni ne se charge sans les deux : un navigateur qui encode
// l'Opus (AudioEncoder) et un moteur qui sait envoyer le micro
// (`engineSendsMicrophone`, faux tant que le moteur web n'a pas le flux
// montant) ; la sonde du navigateur n'est même pas faite sans le moteur.

import { untrack } from 'svelte'
import { engineSendsMicrophone, type EngineMicState } from './engine'
import { t } from '../lib/i18n/index.svelte'
import type { MicCapture } from './miccapture'
import type { LiveSession } from './livesession.svelte'

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

/** Ce navigateur sait-il capter le micro et l'encoder en Opus ? Une fois par page, et seulement
 * quand le moteur sait l'envoyer (WEB-24 : sinon, une sonde pour une puce et des réglages cachés). */
export function probeMicrophone(): Promise<boolean> {
  if (!engineSendsMicrophone()) return Promise.resolve(false)
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

/** Où en est « Micro » : coupé, allumé en attendant la réponse de l'ordinateur (« Envoi… »),
 * envoyé, ou refusé par l'ordinateur (sa raison). */
export type MicStatus = 'off' | 'connecting' | 'on' | 'refused'

export function micStatus(wanted: boolean, engine: EngineMicState): MicStatus {
  if (engine.state === 'refused') return 'refused'
  if (!wanted) return 'off'
  return engine.state === 'open' ? 'on' : 'connecting'
}

/**
 * « Micro » dans une session (docs/link-ui.md, « Micro ») : proposé quand ce navigateur encode l'Opus et
 * que le moteur sait l'envoyer ; la capture suit le choix pendant l'image (allumé, et pas refusé par
 * l'ordinateur) ; « Micro envoyé à {nom}. » quand l'ordinateur prend un micro allumé à la main.
 * Créé pendant l'initialisation de la session (ses effets en dépendent).
 */
export class SessionMic {
  private s: LiveSession
  /** Ce que l'ordinateur dit du micro (lu au moteur pendant la session). */
  engine = $state<EngineMicState>({ state: 'idle' })
  /** La capture tourne (autorisation donnée, encodeur prêt). */
  private capturing = $state(false)
  private starting = false
  /** Numéro de la demande de capture : un arrêt pendant le chargement de la capture l'annule. */
  private run = 0
  /** Allumé à la main : l'avis « Micro envoyé à {nom}. » vient quand l'ordinateur le prend. */
  private asked = false
  private capture: MicCapture | null = null

  readonly wanted = $derived.by(() => micOn(this.s.hostname))
  readonly refusal = $derived(this.engine.state === 'refused' ? this.engine.reason : null)
  /** Un invité ne voit « Micro » que si l'hôte le laisse parler. */
  readonly shown = $derived.by(() => micro.browser && engineSendsMicrophone() && !(this.s.g && this.refusal === 'not_allowed'))
  readonly reason = $derived(this.refusal ? t(`micro.${this.refusal}`) : '')
  /** Allumé dans la barre : choisi, et pas refusé par l'ordinateur (allumé aussi avant sa réponse). */
  readonly lit = $derived(this.wanted && !this.refusal)
  readonly tip = $derived.by(() => this.reason || t(this.lit ? 'micro.tooltip_on' : 'micro.tooltip_off', { shortcut: this.s.shortcut('V') }))
  readonly status = $derived(micStatus(this.wanted, this.engine))
  /** Sous-titre au toucher : « Envoi… » jusqu'à la réponse, puis « Envoyé », ou la raison du refus. */
  readonly statusText = $derived(this.status === 'refused' ? this.reason : t(`micro.status_${this.status}`))

  constructor(s: LiveSession) {
    this.s = s
    void probeMicrophone()

    // La capture suit le choix : pendant l'image, allumé et pris (ou pas encore refusé) par l'ordinateur.
    $effect(() => {
      const want = this.s.phase === 'live' && this.shown && this.wanted && !this.refusal
      untrack(() => {
        if (want && !this.capturing && !this.starting) void this.start()
        else if (!want && (this.capturing || this.starting)) this.stop()
      })
    })

    // Micro choisi ou traitement voix changés en cours d'envoi : la capture reprend avec eux.
    $effect(() => {
      void this.s.prefs.micDevice
      void this.s.prefs.micProcessing
      untrack(() => {
        if (!this.capturing) return
        this.capture?.stop(false)
        this.capturing = false
        void this.start()
      })
    })

    // L'état du micro dit par l'ordinateur, et l'avis bref quand il le prend après un allumage.
    $effect(() => {
      if (this.s.phase !== 'live' || !this.shown) return
      const timer = setInterval(() => {
        const next = this.s.engine?.microphoneState() ?? { state: 'idle' as const }
        const was = this.engine
        if (next.state !== was.state || (next.state === 'refused' && was.state === 'refused' && next.reason !== was.reason)) this.engine = next
        if (this.asked && next.state === 'open') {
          this.asked = false
          this.s.showNotice(t('micro.sent', { name: this.s.hostname }))
        }
      }, 500)
      return () => clearInterval(timer)
    })
  }

  /** « Micro » (puce, Ctrl + Alt + Maj + V) : refusé par l'ordinateur, la raison en avis au toucher
   * (`touched`) ; à la souris, l'infobulle la dit déjà. */
  toggle(touched: boolean) {
    if (this.refusal) {
      if (touched) this.s.showNotice(this.reason)
      return
    }
    const on = !this.wanted
    this.asked = on
    setMicOn(this.s.hostname, on)
  }

  private async start() {
    this.starting = true
    const run = ++this.run
    const { MicCapture } = await import('./miccapture')
    if (run !== this.run) return
    this.capture ??= new MicCapture((packet, pts) => this.s.engine?.sendMicrophone(packet, pts))
    const r = await this.capture.start(this.s.prefs.micDevice, this.s.prefs.micProcessing)
    this.starting = false
    if (r === 'ok') {
      this.capturing = true
      return
    }
    // Arrêtée en route (session quittée pendant la demande) : le choix retenu pour cet ordinateur reste (WEB-13).
    if (r === 'cancelled') return
    // Refusé par le navigateur, ou pas de micro : « Micro » reste coupé.
    this.asked = false
    setMicOn(this.s.hostname, false)
    if (r === 'denied') this.s.showNotice(t('micro.permission_web'), 8000)
    else if (r === 'none') this.s.showNotice(t('micro.no_microphone'))
  }

  /** Arrête la capture (fin de l'image, micro coupé, session démontée). */
  stop() {
    this.run++
    this.starting = false
    this.capture?.stop()
    this.capturing = false
  }
}
