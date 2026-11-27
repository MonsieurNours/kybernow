// Ce que la session règle en cours de route (docs/link-ui.md, « Barre de session ») : la souris
// (« Auto », du bureau, des jeux ; « Auto » suit le jeu au premier plan sur l'ordinateur), le son (coupé
// dans la barre, ou en attente d'un geste du navigateur), et le débit d'une image qui n'est plus
// regardée (onglet masqué : 20 Mbit/s, « Image qui n'est plus regardée »).

import { backend } from './backend'
import type { Engine } from './engine'
import type { LiveSession } from './livesession.svelte'
import { releasePointer } from './pointer'
import type { MouseMode } from './prefs'

/** Débit demandé quand l'onglet est masqué. */
const HIDDEN_BPS = 20_000_000

export class SessionControls {
  private s: LiveSession
  mouseMode = $state<MouseMode>('bureau')
  /** Un jeu est au premier plan sur l'ordinateur (souris « Auto » de l'hôte). */
  gameInForeground = $state(false)
  /** Le navigateur attend un geste pour jouer le son. */
  soundBlocked = $state(false)
  muted = $state(false)
  /** L'onglet est masqué et le débit plafonné (« 20 Mbit/s · fenêtre cachée »). */
  capped = $state(false)
  /** Sonde du son en cours (une seule à la fois). */
  private audioProbe = 0

  /** Le mode de la souris en ce moment (« Auto » : celle des jeux quand un jeu est au premier plan). */
  readonly gameMouse = $derived(this.mouseMode === 'jeu' || (this.mouseMode === 'auto' && this.gameInForeground))

  constructor(s: LiveSession) {
    this.s = s
    // Un invité ou un ami n'a pas le mode « auto » (il ne sait pas si un jeu est au premier plan).
    const mouse = s.prefs.mouse
    this.mouseMode = (s.g || s.b) && mouse === 'auto' ? 'bureau' : mouse
  }

  // ——— Souris ———

  /** Choisir la souris l'applique tout de suite et la garde pour cet ordinateur (hôte). */
  setMouseMode(m: MouseMode) {
    this.mouseMode = m
    if (!this.s.g && !this.s.b) this.s.prefs.mouse = m
    if (!this.pointerAllowed()) releasePointer()
  }

  /** Le raccourci de la souris bascule entre celle du bureau et celle des jeux. */
  toggleMouse() {
    this.setMouseMode(this.gameMouse ? 'bureau' : 'jeu')
  }

  /** La souris peut être capturée (souris des jeux, avec le clavier et la souris). */
  pointerAllowed(): boolean {
    return this.s.canKbm && this.gameMouse
  }

  /** Souris « Auto » de l'hôte : un jeu est-il au premier plan sur l'ordinateur ? Un invité ou un ami n'a
   * pas de compte pour le demander : sa souris reste celle du bureau (ou des jeux, à la main). */
  async pollForeground() {
    const s = this.s
    if (s.g || s.b || s.phase !== 'live' || this.mouseMode !== 'auto') return
    const f = await backend.foreground().catch(() => null)
    const game = !!f?.game
    if (game === this.gameInForeground) return
    this.gameInForeground = game
    if (!game) releasePointer()
  }

  // ——— Son ———

  toggleMute() {
    this.muted = !this.muted
    this.s.engine?.setMute(this.muted)
  }

  /** Après la première image : le son a-t-il démarré, ou le navigateur attend-il un geste ? Le moteur crée
   * son lecteur au premier paquet de son, parfois quelques secondes après l'image : on sonde toutes les
   * 200 ms jusqu'à le savoir, 30 s au plus (un ordinateur sans son n'en envoie pas). */
  async probeAudio(e: Engine) {
    const s = this.s
    if (!s.prefs.audio) return
    const id = ++this.audioProbe
    const t0 = performance.now()
    while (id === this.audioProbe && s.engine === e && s.phase === 'live' && performance.now() - t0 < 30_000) {
      const state = await e.audioState()
      if (id !== this.audioProbe || s.engine !== e) return
      if (state !== 'absent') {
        this.soundBlocked = state === 'suspended'
        return
      }
      await new Promise((r) => setTimeout(r, 200))
    }
  }

  /** La sonde en cours s'arrête (démontage). */
  stopProbe() {
    this.audioProbe++
  }

  /** « Toucher pour le son » : la reprise se demande dans le geste. */
  async unlockSound() {
    const e = this.s.engine
    if (!e) return
    await e.audioResume()
    this.soundBlocked = (await e.audioState()) === 'suspended'
  }

  /** Tout clic ou toute touche est un geste : le son peut y reprendre. */
  async soundGesture() {
    const e = this.s.engine
    if (e && (await e.audioState()) === 'running') this.soundBlocked = false
  }

  // ——— Débit ———

  /**
   * L'onglet est masqué ou revient : 20 Mbit/s demandés à l'ordinateur tant qu'il est masqué, rendus au
   * retour, sans avis ; le navigateur ne dessine plus l'image d'un onglet masqué. Pas pour un invité : le
   * débit de l'image de l'hôte ne lui appartient pas (Kyber refuse). Un moteur sans `set_bitrate` : rien
   * ne change.
   */
  applyBitrate() {
    const { engine: e, plan, g, phase } = this.s
    if (!e || !plan || g || phase !== 'live') return
    const hidden = document.visibilityState === 'hidden'
    if (hidden === this.capped) return
    if (hidden && plan.bitrate <= HIDDEN_BPS) return
    if (e.setBitrate(hidden ? HIDDEN_BPS : plan.bitrate)) this.capped = hidden
  }

  /** Le débit visé (Statistiques) : celui du plan, ou 20 Mbit/s onglet masqué ; `null` pour un invité. */
  target(): { mbps: number; hidden: boolean } | null {
    const plan = this.s.plan
    if (!plan || this.s.g) return null
    return this.capped ? { mbps: HIDDEN_BPS / 1_000_000, hidden: true } : { mbps: plan.mbps, hidden: false }
  }
}
