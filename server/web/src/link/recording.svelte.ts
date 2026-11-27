// L'enregistrement de la session dans la barre (docs/link-ui.md, « Enregistrement de la session ») :
// l'état que donne le dernier signe de vie (hôte ou invité ; `null` : la Station n'enregistre pas,
// rien n'est montré), le temps qui passe, l'avis bref quand il change, et « Enregistrer » de l'hôte.

import { elapsedNow, readRecording, recordingClock, recordingNotice, recordingTooltip, type RecordingState } from '../lib/recording'
import { t } from '../lib/i18n/index.svelte'
import { backend } from './backend'
import { deviceId } from './prefs'
import type { LiveSession } from './livesession.svelte'

export class SessionRecording {
  private s: LiveSession
  state = $state<RecordingState | null>(null)
  /** Quand l'état est arrivé, et l'heure de la page, relue chaque seconde pendant l'enregistrement. */
  private at = $state(Date.now())
  private now = $state(Date.now())
  busy = $state(false)
  /** Un signe de vie a déjà dit l'état : sans lui, un enregistrement en cours n'est pas « lancé » (hôte). */
  private seen = false

  /** « 02:14 ». */
  readonly time = $derived(recordingClock(elapsedNow(this.state, this.at, this.now)))
  /** « Enregistrement · 02:14 » (hôte), « Enregistrement » (invité). */
  readonly pill = $derived.by(() => this.s.g ? t('recording.guest_pill') : t('recording.pill', { time: this.time }))
  /** Infobulle : ce que fait le bouton de l'hôte, ou « {nom} enregistre la session. » pour un invité. */
  readonly hint = $derived.by(() => !this.state ? '' : this.s.g ? t('recording.guest_notice', { name: this.s.hostname }) : recordingTooltip(this.state, this.s.shortcut('R')))

  constructor(s: LiveSession) {
    this.s = s
  }

  /** Le nouvel état (signe de vie, réponse de l'appel) : l'avis bref de link-ui quand il change. */
  apply(next: RecordingState | null) {
    const text = recordingNotice(this.seen ? this.state : undefined, next, this.s.g ? { guest: this.s.hostname } : 'host')
    this.state = next
    this.at = this.now = Date.now()
    this.seen = true
    if (text) this.s.say(text)
  }

  /** L'état que porte un signe de vie (hôte ou invité). */
  applyBeat(v: unknown) {
    this.apply(readRecording(v))
  }

  /** Une seconde de plus pendant l'enregistrement (barre fermée comprise). */
  tick() {
    if (this.state?.active) this.now = Date.now()
  }

  /** « Enregistrer » ou la pastille (hôte) : lance ou arrête. Coupé dans Admin : rien à la souris
   * (l'infobulle dit pourquoi), la raison en avis au toucher (`touched`). Un refus : le texte d'Admin. */
  async toggle(touched: boolean) {
    const state = this.state
    if (!state || this.busy || this.s.g || this.s.b) return
    if (!state.active && !state.allowed) {
      if (touched) this.s.showNotice(t('recording.disabled'))
      return
    }
    this.busy = true
    try {
      this.apply(readRecording(await backend.recording(!state.active, deviceId())))
    } catch (e) {
      this.s.showNotice(e instanceof Error ? e.message : String(e))
    } finally {
      this.busy = false
    }
  }
}
