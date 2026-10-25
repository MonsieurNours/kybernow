<!--
  Vue d'ensemble : l'image d'abord (aperçu de l'écran de l'ordinateur), qui
  est connecté en ce moment, l'état en quatre cartes, les appareils et ce qui
  s'est passé aujourd'hui.
-->
<script lang="ts">
  import { api, type ActivityEvent, type AudioStatus, type Device, type DisplayStatus } from '../lib/api'
  import { app, errorText, notify, refreshStatus } from '../lib/store.svelte'
  import { ago, capital, deviceIcon, duration, hhmm, isToday, mode, ms, when } from '../lib/format'
  import Icon from '../lib/Icon.svelte'
  import Mark from '../lib/Mark.svelte'
  import { i18n, t } from '../lib/i18n/index.svelte'

  let display = $state<DisplayStatus | null>(null)
  let audio = $state<AudioStatus | null>(null)
  let devices = $state<Device[]>([])
  let events = $state<ActivityEvent[]>([])
  let preview = $state<{ url: string; at: number } | null>(null)
  let previewFailed = $state(false)
  let ending = $state(false)
  let now = $state(Date.now() / 1000)

  const status = $derived(app.status)
  const live = $derived(status?.activity?.live ?? null)
  const last = $derived(status?.activity?.last ?? null)
  const hostname = $derived(status?.hostname ?? t('overview.thisComputer'))
  // Un autre client que les apps (navigateur, ligne de commande) : écran ou son virtuels actifs.
  const other = $derived(!live && (!!display?.session || !!audio?.session))
  const gpuName = $derived(status?.gpu?.devices[0]?.name ?? null)

  async function loadPreview() {
    try {
      const res = await fetch(api.previewUrl(1600, Date.now()), { credentials: 'same-origin' })
      if (!res.ok) throw new Error()
      const at = Date.parse(res.headers.get('x-captured-at') ?? '') / 1000 || Date.now() / 1000
      const url = URL.createObjectURL(await res.blob())
      if (preview) URL.revokeObjectURL(preview.url)
      preview = { url, at }
      previewFailed = false
    } catch {
      previewFailed = !preview
    }
  }

  async function loadState() {
    try {
      ;[display, audio] = await Promise.all([api.display(), api.audio()])
    } catch {
      // 401 gérée globalement
    }
  }

  async function loadLists() {
    try {
      const [d, e] = await Promise.all([api.devices(), api.events(12)])
      devices = d.devices
      events = e.events
    } catch {
      // idem
    }
  }

  $effect(() => {
    loadPreview()
    loadState()
    loadLists()
    const fast = setInterval(() => {
      loadState()
      now = Date.now() / 1000
    }, 4000)
    const slow = setInterval(loadLists, 10000)
    const image = setInterval(loadPreview, 60000)
    return () => {
      clearInterval(fast)
      clearInterval(slow)
      clearInterval(image)
      if (preview) URL.revokeObjectURL(preview.url)
    }
  })

  // Une session qui commence ou finit : la liste et le fil bougent.
  // La langue change : le fil est réécrit par le serveur dans la nouvelle langue.
  $effect(() => {
    void live?.device_id
    void i18n.lang
    loadLists()
  })

  async function endSession() {
    const who = live?.name ?? t('overview.connectedDevice')
    const restart = !live
    if (!confirm(restart ? t('overview.confirmCut') : t('overview.confirmEnd', { who }))) return
    ending = true
    try {
      const r = await api.endSession()
      notify('ok', r.method === 'app' ? t('overview.willDisconnect', { who: capital(who) }) : r.method === 'restart' ? t('overview.cut') : t('overview.noSession'))
      await refreshStatus()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      ending = false
    }
  }

  const visibleEvents = $derived.by(() => {
    const today = events.filter((e) => isToday(e.at))
    return { today: today.length > 0, list: (today.length ? today : events).slice(0, 5) }
  })

  const serverCard = $derived.by(() => {
    const s = status?.service.state
    if (s === 'running')
      return {
        ok: true,
        title: t('overview.server.running'),
        sub: status?.controller.reachable ? t('overview.server.streamPort', { port: status.controller.port }) : t('overview.server.streamStarting'),
      }
    if (s === 'starting') return { ok: false, title: t('overview.server.starting'), sub: t('overview.server.fewSeconds') }
    if (s === 'stopped') return { ok: false, title: t('overview.server.stopped'), sub: t('overview.server.startInSystem') }
    return { ok: false, title: t('overview.server.missing'), sub: t('overview.server.reinstall') }
  })

  const displayCard = $derived.by(() => {
    if (!display) return { ok: true, title: '…', sub: '' }
    if (display.settings.trigger === 'off') return { ok: true, off: true, title: t('overview.display.off'), sub: t('overview.display.offSub') }
    if (display.session) return { ok: true, title: t('overview.display.active'), sub: mode(display.session.mode) }
    if (!display.driver.available) return { ok: false, title: t('overview.display.noDriver'), sub: t('overview.display.seeImageSound') }
    return { ok: true, title: t('overview.display.ready'), sub: t('overview.display.readySub') }
  })

  const audioCard = $derived.by(() => {
    if (!audio) return { ok: true, title: '…', sub: '' }
    if (audio.settings.trigger === 'off') return { ok: true, off: true, title: t('overview.audio.off'), sub: t('overview.audio.offSub') }
    if (audio.session)
      return {
        ok: true,
        title: t('overview.audio.sent'),
        sub: audio.session.channels === 8 ? '7.1' : audio.session.channels === 6 ? '5.1' : t('overview.audio.stereo'),
      }
    if (!audio.devices.speakers) return { ok: false, title: t('overview.audio.noSpeakers'), sub: t('overview.audio.seeImageSound') }
    return { ok: true, title: t('overview.audio.ready'), sub: t('overview.audio.readySub') }
  })
</script>

<section class="hero" aria-label={t('overview.heroLabel')}>
  <div class="picture" aria-hidden="true">
    {#if preview}
      <img src={preview.url} alt="" />
    {:else}
      <div class="signature"><Mark size={900} seam="rgba(154,140,255,.18)" /></div>
    {/if}
    <div class="fade-up"></div>
    <div class="fade-left"></div>
  </div>

  <div class="top">
    {#if live || other}
      <span class="lbl live"><span class="dot live"></span>{t('overview.live')}</span>
    {:else if preview}
      <span class="lbl">{t('overview.previewAgo', { ago: ago(preview.at) })}</span>
    {:else}
      <span class="lbl">{previewFailed ? t('overview.previewUnavailable') : t('overview.previewLoading')}</span>
    {/if}
    <span class="mono specs">{[status?.os, gpuName].filter(Boolean).join(' · ')}</span>
  </div>

  <div class="bottom">
    {#if live}
      <div class="title">
        <h1 class="disp">{t('overview.liveTitle', { name: live.name })}</h1>
        <span>{t('overview.since', { duration: duration(now - live.started_at) })}{live.profile ? ` · ${t('overview.profile', { profile: live.profile })}` : ''}{live.mode ? ` · ${mode(live.mode)}` : ''}</span>
      </div>
      <div class="actions">
        {#if live.latency_ms}
          <div class="latency">
            <span class="lit-number">{ms(live.latency_ms)}</span>
            <span class="lbl">{t('overview.medianLatency')}</span>
          </div>
        {/if}
        <button type="button" class="danger" disabled={ending || live.end_requested} onclick={endSession}>
          {live.end_requested ? t('overview.endRequested') : t('overview.endSession')}
        </button>
      </div>
    {:else if other}
      <div class="title">
        <h1 class="disp">{t('overview.otherTitle')}</h1>
        <span>{display?.session ? `${display.session.user ? `${t('overview.account', { user: display.session.user })} · ` : ''}${mode(display.session.mode)}` : t('overview.soundSent')}</span>
      </div>
      <button type="button" class="danger" disabled={ending} onclick={endSession}>{t('overview.cutConnection')}</button>
    {:else}
      <div class="title">
        <h1 class="disp">{t('overview.waiting', { hostname })}</h1>
        <span>
          {#if last}
            {t('overview.lastSession', { name: last.name, when: when(last.ended_at), duration: duration(last.ended_at - last.started_at) })}
          {:else}
            {t('overview.nobody')}
          {/if}
        </span>
      </div>
      <div class="ctas">
        <a href="/link/" class="ghost glass-ghost"><Icon name="browser" size={16} />{t('overview.openInBrowser')}</a>
        <a href="#/appareils" class="primary"><Icon name="plus" size={15} stroke={2.2} />{t('overview.addDevice')}</a>
      </div>
    {/if}
  </div>
</section>

<div class="body rise">
  <section class="health" aria-label={t('overview.healthLabel')}>
    <a href="#/systeme/serveur" class="tile">
      <span class="lbl">{t('overview.serverTile')}</span>
      <span class="state" class:warn={!serverCard.ok}><Icon name={serverCard.ok ? 'check' : 'warning'} size={16} stroke={2.4} />{serverCard.title}</span>
      <span class="sub">{serverCard.sub}</span>
    </a>
    <a href="#/image-son/ecran" class="tile">
      <span class="lbl">{t('overview.displayTile')}</span>
      <span class="state" class:warn={!displayCard.ok} class:off={displayCard.off}><Icon name={displayCard.ok ? 'check' : 'warning'} size={16} stroke={2.4} />{displayCard.title}</span>
      <span class="sub">{displayCard.sub}</span>
    </a>
    <a href="#/image-son/son" class="tile">
      <span class="lbl">{t('overview.soundTile')}</span>
      <span class="state" class:warn={!audioCard.ok} class:off={audioCard.off}><Icon name={audioCard.ok ? 'check' : 'warning'} size={16} stroke={2.4} />{audioCard.title}</span>
      <span class="sub">{audioCard.sub}</span>
    </a>
    {#if status?.update_available}
      <a href="#/systeme" class="tile update">
        <span class="lbl">{t('overview.updateTile')}</span>
        <span class="state"><Icon name="download" size={16} stroke={2.2} />{t('overview.updateAvailable', { version: status.update_available })}</span>
        <span class="sub">{t('overview.updateSub')}</span>
      </a>
    {:else}
      <a href="#/systeme" class="tile">
        <span class="lbl">{t('overview.versionTile')}</span>
        <span class="state"><Icon name="check" size={16} stroke={2.4} />{t('overview.upToDate')}</span>
        <span class="sub">KyberNow Station {status?.admin_version ?? ''}</span>
      </a>
    {/if}
  </section>

  <div class="columns">
    <section aria-labelledby="t-app" class="list devices">
      <div class="list-head">
        <h2 id="t-app" class="disp">{t('overview.devices')}</h2>
        <a href="#/appareils">{t('overview.allDevices')}</a>
      </div>
      {#each devices.slice(0, 3) as d (d.id)}
        {@const isLive = live?.device_id === d.id}
        <div class="row">
          <span class="device-icon"><Icon name={deviceIcon(d.kind)} size={19} stroke={1.6} /></span>
          <div class="who">
            <span class="name">{d.name}</span>
            <span class="mono meta">{[d.os, d.latency_ms ? t('overview.median', { ms: ms(d.latency_ms) }) : null].filter(Boolean).join(' · ')}</span>
          </div>
          <span class="when" class:live={isLive}>{isLive ? t('overview.inSession') : capital(ago(d.last_seen))}</span>
        </div>
      {:else}
        <p class="empty">{t('overview.noDevices')} <a href="#/appareils">{t('overview.addDevice')}</a></p>
      {/each}
    </section>

    <section aria-labelledby="t-log" class="list events">
      <div class="list-head">
        <h2 id="t-log" class="disp">{visibleEvents.today ? t('overview.today') : t('overview.lately')}</h2>
        <a href="#/systeme/journaux">{t('overview.logs')}</a>
      </div>
      {#if visibleEvents.list.length}
        <ol>
          {#each visibleEvents.list as e (e.at + e.text)}
            <li class:warn={e.kind === 'warn'}>
              <span class="mono">{visibleEvents.today ? hhmm(e.at) : capital(ago(e.at))}</span>
              <span>{e.text}</span>
            </li>
          {/each}
        </ol>
      {:else}
        <p class="empty">{t('overview.noEvents')}</p>
      {/if}
    </section>
  </div>
</div>

<style>
  .hero {
    position: relative;
    height: 470px;
    overflow: hidden;
    background: #0b0e1a;
  }

  .picture {
    position: absolute;
    inset: 0;
  }

  .picture img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: appear 1.2s ease both;
  }

  .signature {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(60% 70% at 70% 25%, rgb(109 91 255 / 0.32), transparent 65%),
      radial-gradient(50% 60% at 30% 80%, rgb(70 60 170 / 0.22), transparent 65%),
      #070a12;
  }

  .signature :global(svg) {
    position: absolute;
    right: -180px;
    top: -260px;
    opacity: 0.35;
  }

  .fade-up {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, var(--ink-0) 0%, rgb(5 7 10 / 0.86) 30%, rgb(5 7 10 / 0.15) 72%, rgb(5 7 10 / 0.4) 100%);
  }

  .fade-left {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgb(5 7 10 / 0.75) 0%, rgb(5 7 10 / 0) 55%);
  }

  .top {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    padding: 22px 40px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .top .lbl {
    color: var(--text-2);
  }

  .top .lbl.live {
    color: var(--signal-light);
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .specs {
    font-size: 11px;
    color: var(--text-2);
  }

  .ctas {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .glass-ghost {
    height: 42px;
    background: rgb(9 12 17 / 0.55);
    backdrop-filter: blur(18px);
  }

  .bottom {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 0 40px 34px;
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 22px;
  }

  .title {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
    max-width: 680px;
  }

  .title h1 {
    font-size: 50px;
    line-height: 1;
    font-weight: 650;
    letter-spacing: -0.035em;
    text-wrap: balance;
    overflow-wrap: anywhere;
  }

  .title span {
    font-size: 14.5px;
    color: var(--text-2);
  }

  .actions {
    display: flex;
    gap: 22px;
    align-items: flex-end;
  }

  .latency {
    display: flex;
    flex-direction: column;
    gap: 4px;
    align-items: flex-end;
  }

  .latency .lit-number {
    font-size: 30px;
  }

  .body {
    max-width: 1120px;
    padding: 30px 40px 48px;
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  .health {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr));
    gap: 12px;
  }

  .tile {
    border-radius: var(--r-card);
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    text-decoration: none;
    color: inherit;
    transition: box-shadow 0.2s;
  }

  .tile:hover {
    color: inherit;
    box-shadow: inset 0 0 0 1px var(--line-3);
  }

  .tile .state {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 15px;
    font-weight: 600;
  }

  .tile .state :global(svg) {
    color: var(--ok);
  }

  .tile .state.warn :global(svg) {
    color: var(--warn);
  }

  .tile .state.off :global(svg) {
    color: var(--faint);
  }

  .tile .sub {
    font-size: 12.5px;
    color: var(--faint);
  }

  .tile.update {
    background: rgb(109 91 255 / 0.08);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.45);
  }

  .tile.update .lbl {
    color: var(--signal-light);
  }

  .tile.update .state :global(svg) {
    color: var(--signal-light);
  }

  .tile.update .sub {
    color: var(--text-2);
  }

  .columns {
    display: flex;
    flex-wrap: wrap;
    gap: 22px;
    align-items: flex-start;
  }

  .list {
    border-radius: var(--r-card);
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line);
    padding: 22px 24px;
    min-width: 0;
  }

  .devices {
    flex: 999 1 420px;
    padding-bottom: 8px;
  }

  .events {
    flex: 1 1 320px;
  }

  .list-head {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 8px;
  }

  .list-head h2 {
    font-size: 21px;
    font-weight: 620;
    letter-spacing: -0.02em;
  }

  .list-head a {
    font-size: 13px;
  }

  .row {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    gap: 14px;
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

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .who .name {
    font-size: 14.5px;
    font-weight: 600;
  }

  .meta {
    font-size: 11px;
    color: var(--faint);
  }

  .when {
    font-size: 12.5px;
    color: var(--faint);
  }

  .when.live {
    color: var(--signal-light);
  }

  .empty {
    font-size: 13.5px;
    color: var(--faint);
    padding: 12px 0 14px;
  }

  ol {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-size: 13.5px;
    line-height: 1.4;
    color: var(--text-2);
  }

  li {
    display: grid;
    grid-template-columns: 64px 1fr;
    gap: 10px;
  }

  li .mono {
    font-size: 11px;
    color: var(--ghost);
    padding-top: 2px;
  }

  li.warn > span:last-child {
    color: var(--warn-text);
  }

  @keyframes appear {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 760px) {
    .hero {
      margin: 8px 16px 0;
      height: 300px;
      border-radius: 16px;
      box-shadow: 0 0 0 1px rgb(255 255 255 / 0.08);
    }

    .top {
      padding: 14px 16px;
    }

    .specs {
      display: none;
    }

    .bottom {
      padding: 0 16px 16px;
      gap: 14px;
    }

    .title {
      gap: 6px;
    }

    .title h1 {
      font-size: 26px;
      line-height: 1.04;
    }

    .title span {
      font-size: 12.5px;
    }

    .bottom > .danger,
    .ctas,
    .ctas > a {
      width: 100%;
    }

    .ctas {
      flex-direction: column-reverse;
    }

    .body {
      padding: 16px 16px 120px;
      gap: 16px;
    }

    .health {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 8px;
    }

    .tile .sub {
      display: none;
    }
  }
</style>
