<!--
  Panneau d'un partage ouvert (KyberNow Share) : le code, le lien et son QR
  code, les demandes venues par le relais (« Accepter », « Refuser »), les
  participants, leurs droits et la qualité d'image d'un invité qui ne reçoit
  pas celle de l'hôte, « Nouveau code », « Arrêter le partage ». Le même sur
  l'accueil et dans Appareils Link d'Admin, et dans la barre de session du
  client web (routes de l'hôte) ; textes de docs/link-ui.md (« Partage »).
-->
<script lang="ts">
  import { joinedGuests, localShare, shareRequests, type ShareGuest, type ShareOps, type ShareRole, type ShareView } from './api'
  import { errorText, notify as storeNotify, refreshStatus, setShare } from './store.svelte'
  import { deviceIcon, imageQuality, screenLine, sinceText } from './format'
  import Icon from './Icon.svelte'
  import Qr from './Qr.svelte'
  import { t, tn } from './i18n/index.svelte'

  let {
    share,
    computer,
    host = null,
    ops = localShare,
    onchange = setShare,
    notify = storeNotify,
    refresh = refreshStatus,
    own = false,
  }: {
    share: ShareView
    /** Nom de l'ordinateur : titre d'un partage lancé depuis la session d'un appareil. */
    computer: string
    /** L'appareil en session qui partage, en tête des participants. */
    host?: { name: string; kind: string; since: number } | null
    /** Routes du partage : celles du PC par défaut (Admin). */
    ops?: ShareOps
    /** Le partage après une action (`null` : arrêté). */
    onchange?: (view: ShareView | null) => void
    /** Où dire le résultat d'une action : le fil de messages d'Admin par défaut. */
    notify?: (kind: 'ok' | 'error', text: string) => void
    /** Relit le partage (invité parti ou partage fermé entre-temps). */
    refresh?: () => unknown
    /** Panneau de l'hôte dans sa propre session : pas de « Ouvert depuis la session de… ». */
    own?: boolean
  } = $props()

  /** Action en cours (« code », « stop », ou l'identifiant d'un invité). */
  let busy = $state<string | null>(null)
  let now = $state(Date.now() / 1000)

  $effect(() => {
    const timer = setInterval(() => (now = Date.now() / 1000), 15000)
    return () => clearInterval(timer)
  })

  const title = $derived(
    share.origin === 'local' ? t('share.titleLocal', { n: share.display.index }) : t('share.titleHost', { name: computer }),
  )
  const code = $derived(`${share.code.slice(0, 3)} ${share.code.slice(3)}`)
  const screen = $derived(screenLine(share.display))
  /** Les invités entrés, et les demandes venues par le relais (au-dessus, elles ne comptent pas dans les invités). */
  const guests = $derived(joinedGuests(share))
  const requests = $derived(shareRequests(share))
  /** Le relais du partage : joint (le lien et le code valent ailleurs), injoignable, ou pas de relais. */
  const relay = $derived(share.relay?.state ?? null)

  /** Icône de l'app d'un invité (mêmes dessins que les appareils). */
  function appIcon(app: string): string {
    if (app === 'ios' || app === 'android') return 'phone'
    if (app === 'tvos' || app === 'webos') return 'tv'
    if (app === 'web') return 'browser'
    return ['mac', 'windows', 'linux'].includes(app) ? 'mac' : 'other'
  }

  function appName(app: string): string {
    const name = t(`share.apps.${app}`)
    return name.startsWith('share.') ? app : name
  }

  /** Sa qualité d'image, seulement quand ce n'est pas celle de l'hôte (« 1080p · 60 Hz ») ; l'hôte ne la change pas. */
  function imageLine(g: ShareGuest): string {
    return g.image?.follows_host === false ? imageQuality(g.image) : ''
  }

  /** Le rôle de deux droits : les manettes, le clavier et la souris. */
  function roleOf(pads: boolean, kbm: boolean): ShareRole {
    return pads ? (kbm ? 'player_kbm' : 'player') : kbm ? 'kbm' : 'spectator'
  }

  /**
   * Ce que l'hôte peut faire d'un invité (docs/link-ui.md) : deux droits indépendants, chacun donné ou
   * repris sans toucher à l'autre, « Donner » ou « Reprendre les manettes », puis « Donner » ou
   * « Reprendre le clavier et la souris ».
   */
  function moves(g: ShareGuest): { label: string; role: ShareRole }[] {
    const pads = g.role === 'player' || g.role === 'player_kbm'
    const kbm = g.role === 'kbm' || g.role === 'player_kbm'
    return [
      { label: t(pads ? 'share.takePads' : 'share.givePads'), role: roleOf(!pads, kbm) },
      { label: t(kbm ? 'share.takeKbm' : 'share.giveKbm'), role: roleOf(pads, !kbm) },
    ]
  }

  async function run(key: string, action: () => Promise<unknown>, done?: string) {
    busy = key
    try {
      const view = (await action()) as ShareView | { open: false } | null
      onchange(view?.open ? view : null)
      if (done) notify('ok', done)
    } catch (e) {
      notify('error', errorText(e))
      // Invité parti ou partage fermé entre-temps : l'état est relu.
      refresh()
    } finally {
      busy = null
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(share.web)
      notify('ok', t('share.copied'))
    } catch {
      notify('error', t('share.copyFailed'))
    }
  }
</script>

<div class="share">
  <div class="top">
    <Qr text={share.link} size={152} label={t('share.qrLabel')} />
    <div class="intro">
      <div class="heading">
        <h2 class="disp">{title}</h2>
        {#if !own}<span class="sub">{share.origin === 'host' ? t('share.openedBy', { name: share.host }) : screen}</span>{/if}
      </div>
      <div class="code">
        <span class="lbl">{t('share.codeLabel')}</span>
        <strong class="mono" aria-label={t('share.codeAria', { digits: share.code.split('').join(' ') })}>{code}</strong>
      </div>
      <div class="link-row">
        <button type="button" class="ghost small" onclick={copy}><Icon name="copy" size={15} stroke={2} />{t('share.copy')}</button>
        <span class="mono link">{share.web}</span>
      </div>
      <p class="help">{relay === 'joined' ? t('share.helpRelay') : t('share.help')}</p>
      {#if relay === 'unreachable'}<p class="help">{t('share.relayUnreachable')}</p>{/if}
    </div>
  </div>

  {#if requests.length}
    <div class="people requests">
      <div class="people-head">
        <h3>{t('share.requests')}</h3>
      </div>
      {#each requests as g (g.id)}
        <div class="person" role="group" aria-labelledby="ask-{g.id}">
          <span class="device-icon live"><Icon name={appIcon(g.app)} size={19} stroke={1.6} /></span>
          <div class="who">
            <span class="name" id="ask-{g.id}">{t('share.asks', { name: g.name })}</span>
            <span class="meta">{appName(g.app)} · {t('share.away')}</span>
          </div>
          <div class="moves">
            <button type="button" class="ghost small accept" disabled={busy !== null} onclick={() => run(g.id, () => ops.accept(g.id))}><Icon name="check" size={15} stroke={2} />{t('share.accept')}</button>
            <button type="button" class="ghost small" disabled={busy !== null} onclick={() => run(g.id, () => ops.refuse(g.id))}>{t('share.refuse')}</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <div class="people">
    <div class="people-head">
      <h3>{host ? t('share.participants') : t('share.people')}</h3>
      {#if guests.length}<span class="pill signal">{tn('share.guests', guests.length)}</span>{/if}
      {#if guests.length >= 4}<span class="limit">{t('share.limit')}</span>{/if}
    </div>
    {#if host}
      <div class="person">
        <span class="device-icon live"><Icon name={deviceIcon(host.kind)} size={19} stroke={1.6} /></span>
        <div class="who">
          <span class="name">{t('share.host', { name: host.name })}</span>
          <span class="meta">{t('share.since', { duration: sinceText(now - host.since) })}</span>
        </div>
      </div>
    {/if}
    {#each guests as g (g.id)}
      <div class="person">
        <span class="device-icon" class:live={g.role !== 'spectator'}><Icon name={appIcon(g.app)} size={19} stroke={1.6} /></span>
        <div class="who">
          <span class="name">{t(`share.role.${g.role}`, { name: g.name })}</span>
          <span class="meta">{[appName(g.app), g.via === 'relay' ? t('share.away') : '', imageLine(g), t('share.since', { duration: sinceText(now - g.since) })].filter(Boolean).join(' · ')}</span>
        </div>
        <div class="moves">
          {#each moves(g) as m (m.role)}
            <button type="button" class="ghost small" disabled={busy !== null} onclick={() => run(g.id, () => ops.role(g.id, m.role))}>{m.label}</button>
          {/each}
          <button type="button" class="ghost small remove" disabled={busy !== null} onclick={() => run(g.id, () => ops.remove(g.id))}>{t('share.remove')}</button>
        </div>
      </div>
    {:else}
      <p class="empty">{t('share.nobody')}</p>
    {/each}
  </div>

  <div class="foot">
    <button type="button" class="ghost" disabled={busy !== null} onclick={() => run('code', () => ops.code())}><Icon name="restart" size={15} />{t('share.newCode')}</button>
    <button type="button" class="danger" disabled={busy !== null} onclick={() => run('stop', () => ops.stop(), t('share.stopped'))}>{t('share.stop')}</button>
  </div>
</div>

<style>
  .share {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .top {
    display: flex;
    flex-wrap: wrap;
    gap: 22px 26px;
    align-items: flex-start;
  }

  .intro {
    flex: 1 1 320px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .heading {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .heading h2 {
    font-size: 23px;
    font-weight: 620;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }

  .sub {
    font-size: 13px;
    color: var(--faint);
  }

  .code {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .code .lbl {
    color: var(--signal-light);
    font-size: 9.5px;
  }

  .code strong {
    font-size: 34px;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--neon-core);
    text-shadow: var(--text-glow);
    white-space: nowrap;
    user-select: all;
  }

  .link-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }

  .link {
    font-size: 11px;
    color: var(--faint);
    overflow-wrap: anywhere;
    user-select: all;
  }

  .help {
    font-size: 13px;
    line-height: 1.5;
    color: var(--faint);
    max-width: 60ch;
  }

  .people-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding-bottom: 8px;
  }

  .people-head h3 {
    font-size: 15px;
    font-weight: 600;
  }

  .people-head .pill {
    padding: 3px 10px;
    font-size: 12px;
    font-weight: 600;
  }

  .limit {
    margin-left: auto;
    font-size: 12.5px;
    color: var(--faint);
  }

  .person {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    gap: 10px 14px;
    align-items: center;
    padding: 12px 0;
    border-top: 1px solid var(--line);
  }

  .device-icon {
    width: 36px;
    height: 36px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    color: var(--text-2);
  }

  .device-icon.live {
    background: rgb(109 91 255 / 0.14);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: var(--signal-light);
  }

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .name {
    font-size: 14.5px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .meta {
    font-size: 12.5px;
    color: var(--faint);
  }

  .moves {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  .moves .remove:hover:not(:disabled) {
    color: var(--bad-text);
  }

  /* Une demande venue par le relais : « Accepter » ressort, sans prendre le seul bouton principal. */
  .requests .person {
    border-top-color: var(--signal-line);
  }

  .moves .accept {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
  }

  .moves .accept:hover:not(:disabled) {
    background: rgb(109 91 255 / 0.22);
    color: #fff;
  }

  .empty {
    font-size: 13.5px;
    color: var(--faint);
    padding: 12px 0 4px;
    border-top: 1px solid var(--line);
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 10px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }

  @media (max-width: 760px) {
    .person {
      grid-template-columns: 36px minmax(0, 1fr);
    }

    .moves {
      grid-column: 2;
      justify-content: flex-start;
    }

    .limit {
      margin-left: 0;
      flex-basis: 100%;
    }

    .foot > button {
      flex: 1 1 auto;
    }
  }
</style>
