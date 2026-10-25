<!--
  Réglages de l'ordinateur (docs/link-ui.md) : le profil, puis six rubriques
  dans l'ordre commun à toutes les apps (Image, Son, Commandes, Réseau,
  Statistiques, Station), et en bas ce qui vaut pour l'app dans ce navigateur
  (langue). Gardés dans ce navigateur.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from '../lib/Icon.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'
  import LangChoice from '../lib/LangChoice.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { tName } from './names'
  import type { Info } from './backend'
  import { deviceKind, isFirefox, isStandalone, isTouch, type Mode } from './device'
  import type { CodecSupport } from './engine'
  import type { Plan } from './plan'
  import { MAX_MBPS, MIN_MBPS, websocketReason } from './plan'
  import { isCustom, pickProfile, PROFILES, type Codec, type MouseMode, type Prefs, type TouchMode, type Transport } from './prefs'
  import Stats from './Stats.svelte'
  import StationPanel from './StationPanel.svelte'

  let {
    prefs = $bindable(),
    hostname,
    mode,
    plan,
    codecs,
    protocol,
    blocked,
    info,
    onClose,
    onSignedOut,
  }: {
    prefs: Prefs
    hostname: string
    mode: Mode
    plan: Plan | null
    codecs: CodecSupport[] | null
    protocol: 'kymux' | 'websocket' | null
    blocked: boolean
    info: Info | null
    onClose: () => void
    onSignedOut: () => void
  } = $props()

  const custom = $derived(isCustom(prefs))
  const rubrics = $derived([
    { id: 'picture', label: t('web.settings.picture') },
    { id: 'sound', label: t('web.settings.sound') },
    { id: 'controls', label: t('web.settings.controls') },
    { id: 'network', label: t('web.settings.network') },
    { id: 'stats', label: t('web.settings.stats') },
    { id: 'station', label: t('web.settings.station') },
  ])

  /** Chemin de l'image dans ce navigateur, et pourquoi (relu quand le moteur
   * est chargé : `plan` arrive avec lui). */
  const pathText = $derived.by(() => {
    void plan
    const reason = websocketReason()
    return reason === 'touch'
      ? t('web.settings.pathTouch')
      : reason === 'webkit'
        ? t('web.settings.pathWebkit')
        : reason === 'firefox'
          ? t('web.settings.pathFirefox')
          : reason === 'unsupported'
            ? t('web.settings.pathUnsupported')
            : t('web.settings.pathDirect')
  })

  let dialog: HTMLElement
  let body: HTMLElement

  function jump(id: string) {
    body.querySelector(`#rubric-${id}`)?.scrollIntoView({ block: 'start', behavior: 'smooth' })
  }

  onMount(() => {
    dialog.focus()
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
</script>

<div class="scrim" onclick={onClose} role="presentation"></div>
<div class="sheet" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" bind:this={dialog}>
  <div class="top">
    <div>
      <h2 id="settings-title" class="disp">{tName('web.settings.title', 'name', hostname)}</h2>
      <p class="muted">{t('web.settings.kept')}</p>
    </div>
    <button type="button" class="close" aria-label={t('web.settings.close')} onclick={onClose}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  </div>

  <div class="body" bind:this={body}>
    <section aria-labelledby="rubric-profile">
      <h3 id="rubric-profile" class="lbl">{t('web.settings.profile')}</h3>
      <div class="cards">
        {#each PROFILES as p (p)}
          <button type="button" class="card" aria-pressed={!custom && prefs.profile === p} onclick={() => pickProfile(prefs, p)}>
            <span class="name">{t(`web.profile.${p}`)}</span>
            <span class="desc">{t(`web.profile.${p}Note`)}</span>
          </button>
        {/each}
      </div>
      {#if custom}
        <p class="custom"><b>{t('web.profile.custom')}</b> {t('web.profile.customNote')}</p>
      {/if}
    </section>

    <nav class="jump" aria-label={t('web.settings.rubrics')}>
      {#each rubrics as r (r.id)}
        <button type="button" onclick={() => jump(r.id)}>{r.label}</button>
      {/each}
    </nav>

    <section aria-labelledby="rubric-picture">
      <h3 id="rubric-picture" class="lbl">{t('web.settings.picture')}</h3>
      <div class="row">
        <div class="text">
          <span>{t('web.settings.virtualDisplay')}</span>
          <span class="sub">{info && !info.virtual_display.available ? t('web.settings.virtualMissing') : t('web.settings.virtualHelp')}</span>
        </div>
        <Switch checked={prefs.virtualDisplay} label={t('web.settings.virtualDisplay')} onchange={(v) => (prefs.virtualDisplay = v)} />
      </div>
      <div class="row">
        <div class="text">
          <span>{t('web.settings.fullscreen')}</span>
          <span class="sub">{t('web.settings.fullscreenSub')}</span>
        </div>
        <Switch checked={prefs.fullscreen} label={t('web.settings.fullscreen')} onchange={(v) => (prefs.fullscreen = v)} />
      </div>
      <div class="stack">
        <div class="text"><span>{t('web.settings.format')}</span><span class="sub">{isFirefox ? t('web.settings.formatFirefox') : t('web.settings.formatAuto')}</span></div>
        <Segmented
          label={t('web.settings.format')}
          options={[
            { value: 'auto', label: t('web.settings.auto') },
            { value: 'hevc', label: 'HEVC' },
            { value: 'h264', label: 'H.264' },
          ]}
          value={prefs.codec}
          onchange={(v: Codec) => (prefs.codec = v)}
        />
      </div>
      <div class="stack">
        <div class="text">
          <span>{t('web.settings.bitrate')}</span>
          <span class="sub">{prefs.bitrate ? t('web.settings.bitrateManual') : t('web.settings.bitrateAuto')}</span>
        </div>
        <Segmented
          label={t('web.settings.bitrate')}
          options={[
            { value: 'auto', label: t('web.settings.auto') },
            { value: 'manual', label: t('web.settings.manual') },
          ]}
          value={prefs.bitrate ? 'manual' : 'auto'}
          onchange={(v) => (prefs.bitrate = v === 'auto' ? null : (plan?.mbps ?? 40))}
        />
        {#if prefs.bitrate}
          <div class="slider">
            <input type="range" min={MIN_MBPS} max={MAX_MBPS} step="5" bind:value={prefs.bitrate} aria-label={t('web.settings.bitrateAria')} />
            <span class="mono value">{prefs.bitrate} Mbit/s</span>
          </div>
        {:else if plan}
          <p class="mono value">{plan.mbps} Mbit/s · {plan.codec === 'hevc' ? 'HEVC' : 'H.264'}{plan.yuv444 ? ' 4:4:4' : ''}</p>
        {/if}
      </div>
    </section>

    <section aria-labelledby="rubric-sound">
      <h3 id="rubric-sound" class="lbl">{t('web.settings.sound')}</h3>
      <div class="row">
        <div class="text"><span>{t('web.settings.sound')}</span><span class="sub">{t('web.settings.soundSub')}</span></div>
        <Switch checked={prefs.audio} label={t('web.settings.sound')} onchange={(v) => (prefs.audio = v)} />
      </div>
    </section>

    <section aria-labelledby="rubric-controls">
      <h3 id="rubric-controls" class="lbl">{t('web.settings.controls')}</h3>
      {#if !isTouch}
        <div class="stack">
          <div class="text"><span>{t('web.settings.mouse')}</span></div>
          <Segmented
            label={t('web.settings.mouse')}
            options={[
              { value: 'auto', label: t('web.settings.auto') },
              { value: 'bureau', label: t('web.settings.mouseDesktopLabel') },
              { value: 'jeu', label: t('web.settings.mouseGameLabel') },
            ]}
            value={prefs.mouse}
            onchange={(v: MouseMode) => (prefs.mouse = v)}
          />
          <p class="sub">
            {prefs.mouse === 'auto' ? t('web.settings.mouseAuto') : prefs.mouse === 'jeu' ? t('web.settings.mouseGame') : t('web.settings.mouseDesktop')}
          </p>
        </div>
      {:else}
        <div class="stack">
          <div class="text"><span>{t('web.settings.touch')}</span></div>
          <Segmented
            label={t('web.settings.touch')}
            options={[
              { value: 'trackpad', label: t('web.settings.trackpad') },
              { value: 'direct', label: t('web.settings.direct') },
            ]}
            value={prefs.touch}
            onchange={(v: TouchMode) => (prefs.touch = v)}
          />
          <p class="sub">
            {prefs.touch === 'trackpad'
              ? `${t('web.settings.trackpadDesc')} ${t('session.touch.guideTrackpad')}`
              : `${t('web.settings.directDesc')} ${t('session.touch.guideDirect')}`}
          </p>
        </div>
      {/if}
      <div class="row">
        <div class="text"><span>{t('web.settings.clipboard')}</span><span class="sub">{t('web.settings.clipboardSub')}</span></div>
        <Switch checked={prefs.clipboard} label={t('web.settings.clipboard')} onchange={(v) => (prefs.clipboard = v)} />
      </div>
      {#if !isTouch}
        <p class="sub">{t('web.settings.barShortcut')}</p>
      {/if}
    </section>

    <section aria-labelledby="rubric-network">
      <h3 id="rubric-network" class="lbl">{t('web.settings.network')}</h3>
      <div class="stack">
        <div class="text"><span>{t('web.settings.transport')}</span><span class="sub">{t('web.settings.transportHelp')}</span></div>
        <Segmented
          label={t('web.settings.transport')}
          options={[
            { value: 'auto', label: t('web.settings.auto') },
            { value: 'rapide', label: t('web.settings.fast') },
            { value: 'fiable', label: t('web.settings.reliable') },
          ]}
          value={prefs.transport}
          onchange={(v: Transport) => (prefs.transport = v)}
        />
        {#if prefs.transport === 'auto'}<p class="sub">{t('web.settings.transportAuto')}</p>{/if}
      </div>
      <div class="text"><span>{t('web.settings.path')}</span><span class="sub">{pathText}</span></div>
    </section>

    <section aria-labelledby="rubric-stats">
      <h3 id="rubric-stats" class="lbl">{t('web.settings.stats')}</h3>
      <Stats {info} {mode} {plan} {codecs} {protocol} {blocked} />
    </section>

    <section aria-labelledby="rubric-station">
      <h3 id="rubric-station" class="lbl">{t('web.settings.station')}</h3>
      <StationPanel {info} {onSignedOut} />
    </section>

    <section class="app" aria-labelledby="rubric-app">
      <h3 id="rubric-app" class="lbl">{t('web.settings.thisApp')}</h3>
      <div class="stack">
        <div class="text"><span>{t('web.settings.language')}</span></div>
        <LangChoice />
        <p class="sub">{t('web.settings.languageHint')}</p>
      </div>
      {#if isTouch}
        {#if deviceKind() === 'phone'}
          <p class="tip"><Icon name="phone" size={18} /><span><b>{t('web.settings.rotateTitle')}</b> {t('web.settings.rotate')}</span></p>
        {/if}
        {#if !isStandalone}
          <p class="tip"><Icon name="plus" size={18} /><span><b>{t('web.settings.homeScreenTitle')}</b> {t('web.settings.homeScreen')}</span></p>
        {/if}
      {/if}
    </section>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgb(5 7 10 / 0.45);
    z-index: 20;
  }

  .sheet {
    position: fixed;
    right: 16px;
    top: 16px;
    bottom: 16px;
    width: min(460px, calc(100% - 32px));
    z-index: 21;
    border-radius: 22px;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px #1e2832, 0 30px 80px -20px rgb(0 0 0 / 0.8);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    outline: none;
    animation: sheet 0.45s var(--ease-out) both;
  }

  @keyframes sheet {
    from {
      translate: 40px 0;
      opacity: 0;
    }
    to {
      translate: 0 0;
      opacity: 1;
    }
  }

  .top {
    padding: 22px 22px 14px 26px;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  h2 {
    font-size: 24px;
    font-weight: 650;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
    text-wrap: balance;
  }

  .top p {
    font-size: 13px;
  }

  .close {
    width: 44px;
    height: 44px;
    flex: none;
    border: 0;
    border-radius: 10px;
    background: var(--ink-3);
    color: var(--text);
    display: grid;
    place-items: center;
  }

  .body {
    flex: 1;
    overflow: auto;
    padding: 6px 26px 26px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    scroll-margin-top: 8px;
  }

  h3 {
    margin: 0;
    font-weight: 400;
  }

  section:not(:first-child) > h3 {
    padding-top: 2px;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .card {
    min-height: 96px;
    border: 0;
    border-radius: 14px;
    padding: 12px;
    text-align: left;
    color: var(--text);
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px #1e2832;
  }

  .card[aria-pressed='true'] {
    background: rgb(154 140 255 / 0.12);
    box-shadow: inset 0 0 0 1.5px var(--signal);
  }

  .name {
    font-size: 15px;
    font-weight: 650;
  }

  .desc {
    font-size: 12px;
    line-height: 1.4;
    color: var(--muted);
  }

  .custom {
    font-size: 12.5px;
    color: var(--muted);
    padding: 10px 12px;
    border-radius: 12px;
    background: rgb(154 140 255 / 0.08);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .custom b {
    color: var(--text);
    font-weight: 600;
  }

  .jump {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: -10px 0 -4px;
  }

  .jump button {
    height: 30px;
    padding: 0 11px;
    border: 0;
    border-radius: 15px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--muted);
    font-size: 12.5px;
  }

  .jump button:hover {
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--line-3);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 44px;
  }

  /* L'interrupteur garde sa taille quand le texte passe sur deux lignes. */
  .row :global(.sw) {
    flex: none;
  }

  .stack {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 14px;
    text-align: left;
  }

  .sub {
    font-size: 12px;
    color: var(--faint);
  }

  .slider {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
  }

  .slider input {
    flex: 1;
    height: auto;
    padding: 0;
    box-shadow: none;
    background: none;
    accent-color: var(--signal);
  }

  .value {
    font-size: 12px;
    color: var(--signal-light);
    white-space: nowrap;
  }

  .app {
    padding-top: 22px;
    border-top: 1px solid var(--line-2);
  }

  .tip {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 12px 14px;
    border-radius: 14px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 13px;
    color: var(--muted);
  }

  .tip :global(svg) {
    flex: none;
    color: var(--signal);
    margin-top: 1px;
  }

  .tip b {
    color: var(--text);
    font-weight: 600;
  }

  @media (max-width: 520px) {
    .sheet {
      left: 8px;
      right: 8px;
      top: auto;
      bottom: 8px;
      width: auto;
      max-height: 86vh;
      max-height: 86dvh;
    }

    .body {
      padding: 6px 18px 22px;
    }

    .top {
      padding: 18px 16px 12px 18px;
    }

    h2 {
      font-size: 20px;
    }
  }

  @media (max-width: 380px) {
    .cards {
      grid-template-columns: 1fr;
    }

    .card {
      min-height: 0;
    }
  }
</style>
