// Ce que l'hôte partage et prête depuis sa session (docs/link-ui.md, « Partage » et « Prêt de
// l'ordinateur ») : le partage et le prêt que dit son signe de vie, ce que la Station en sait
// (`/api/stream/info`), les pastilles de la barre (« 2 invités », « 1 demande »), et la demande
// acceptée depuis la session, qui va la fermer. Les panneaux sont ShareBox, LendBox et LendAsk.

import { joinedGuests, lendOps, shareRequests, type LendView, type ShareView } from '../lib/api'
import { t, tn } from '../lib/i18n/index.svelte'
import type { Info } from './backend'
import { deviceId } from './prefs'
import type { LiveSession } from './livesession.svelte'

export class Hosting {
  private s: LiveSession
  /** Le partage de cet appareil (signe de vie de l'hôte), absent sans partage. */
  share = $state<ShareView | null>(null)
  /** `old` : Station d'avant la 0.8.0 ; `off` : partage coupé (`multi_client`). */
  shareSupport = $state<'ok' | 'old' | 'off'>('ok')
  /** Le prêt, absent quand il n'y a rien à dire. */
  lend = $state<LendView | null>(null)
  /** `none` : la Station ne connaît pas le prêt, la puce n'est pas là ; `off` : coupé dans Admin. */
  lendSupport = $state<'ok' | 'off' | 'none'>('none')
  /** L'hôte a accepté qu'un ami emprunte l'ordinateur : sa session va se fermer. */
  accepted = $state<string | null>(null)
  /** Les routes du prêt de l'hôte : le client web dit son appareil (un code ou un accord y ferme la session). */
  readonly ops = lendOps(() => ({ device_id: deviceId() }))

  readonly shareHint = $derived(this.shareSupport === 'old' ? t('share.bar.tooOld') : this.shareSupport === 'off' ? t('share.off') : '')
  /** Invités du partage ouvert : la pastille « 2 invités » (rien quand il n'y en a pas encore). */
  readonly guestCount = $derived(joinedGuests(this.share).length)
  /** Demandes venues par le relais : leur pastille « 1 demande », à côté de celle des invités. */
  readonly requestCount = $derived(shareRequests(this.share).length)
  /** « Partager », avec ses pastilles, pour les lecteurs d'écran. */
  readonly shareAria = $derived(
    [t('share.bar.share'), this.guestCount ? tn('share.guests', this.guestCount) : '', this.requestCount ? tn('share.requestCount', this.requestCount) : ''].filter(Boolean).join(' · '),
  )
  readonly lendRequests = $derived(this.lend?.requests ?? [])
  readonly lendAria = $derived([t('lend.open'), this.lendRequests.length ? tn('share.requestCount', this.lendRequests.length) : ''].filter(Boolean).join(' · '))

  constructor(s: LiveSession) {
    this.s = s
  }

  /** Ce que la Station sait du partage et du prêt : une Station d'avant la 0.8.0 (0.9.0) ne les
   * connaît pas ; le partage est coupé quand `multi_client` l'est, le prêt dans Admin. */
  learn(i: Info) {
    this.shareSupport = !i.share ? 'old' : i.share.enabled ? 'ok' : 'off'
    this.lendSupport = !i.lend ? 'none' : i.lend.enabled ? 'ok' : 'off'
  }

  /** Le signe de vie de l'hôte. */
  onBeat(data: { share?: { open?: boolean }; lend?: unknown } | undefined) {
    this.share = data?.share?.open ? (data.share as ShareView) : null
    this.lend = (data?.lend as LendView | undefined) ?? null
  }

  /** À qui l'ordinateur est prêté quand la session se ferme (demande acceptée, code, créneau). */
  lentTo(): string | null {
    return this.lend?.borrower?.name ?? this.accepted
  }

  /** Demande acceptée depuis cette session : elle va se fermer (le signe de vie le dira). */
  acceptedBy(name: string) {
    this.accepted = name
    this.s.showNotice(t('lend.requests.sessionCloses'), 8000)
  }
}
