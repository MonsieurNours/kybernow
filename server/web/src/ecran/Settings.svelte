<!-- Réglages du client web, gardés dans ce navigateur. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'
  import LangChoice from '../lib/LangChoice.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import type { Mode } from './device'
  import { isFirefox, isTouch } from './device'
  import type { Plan } from './plan'
  import { MAX_MBPS, MIN_MBPS, websocketReason } from './plan'
  import type { Codec, MouseMode, Prefs, Profile, TouchMode, Transport } from './prefs'

  let {
    prefs = $bindable(),
    hostname,
    mode,
    plan,
    onClose,
  }: { prefs: Prefs; hostname: string; mode: Mode; plan: Plan | null; onClose: () => void } = $props()

  const profiles: { id: Profile; label: string; desc: string }[] = $derived([
    { id: 'bureau', label: t('web.settings.desktop'), desc: t('web.settings.desktopDesc') },
    { id: 'jeu', label: t('web.settings.game'), desc: t('web.settings.gameDesc') },
  ])

  let advanced = $state(false)

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
      <h2 id="settings-title" class="disp">{t('web.settings.title')}</h2>
      <p class="muted">{t('web.settings.kept', { host: hostname })}</p>
    </div>
    <button type="button" class="close" aria-label={t('web.settings.close')} onclick={onClose}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  </div>

  <div class="body">
    <section>
      <span class="lbl">{t('web.settings.profile')}</span>
      <div class="cards">
        {#each profiles as p (p.id)}
          <button type="button" class="card" aria-pressed={prefs.profile === p.id} onclick={() => (prefs.profile = p.id)}>
            <span class="name">{p.label}</span>
            <span class="desc">{p.desc}</span>
          </button>
        {/each}
      </div>
    </section>

    <section>
      <span class="lbl">{t('web.settings.picture')}</span>
      <div class="row">
        <div class="text">
          <span>{t('web.settings.matchDisplay')}</span>
          <span class="sub">
            {prefs.virtualDisplay
              ? t('web.settings.virtualOn', { width: mode.width, height: mode.height, refresh: mode.refresh })
              : t('web.settings.virtualOff')}
          </span>
        </div>
        <Switch checked={prefs.virtualDisplay} label={t('web.settings.matchDisplay')} onchange={(v) => (prefs.virtualDisplay = v)} />
      </div>
      <div class="row">
        <div class="text">
          <span>{t('web.settings.fullscreen')}</span>
          <span class="sub">{t('web.settings.fullscreenSub')}</span>
        </div>
        <Switch checked={prefs.fullscreen} label={t('web.settings.fullscreen')} onchange={(v) => (prefs.fullscreen = v)} />
      </div>
      <div class="row">
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
      </div>
      {#if prefs.bitrate}
        <div class="slider">
          <input type="range" min={MIN_MBPS} max={MAX_MBPS} step="5" bind:value={prefs.bitrate} aria-label={t('web.settings.bitrateAria')} />
          <span class="mono value">{prefs.bitrate} Mbit/s</span>
        </div>
      {:else if plan}
        <p class="mono value auto">{plan.mbps} Mbit/s · {plan.codec === 'hevc' ? 'HEVC' : 'H.264'}{plan.yuv444 ? ' 4:4:4' : ''}</p>
      {/if}
    </section>

    <section>
      <span class="lbl">{t('web.settings.soundSharing')}</span>
      <div class="row">
        <div class="text"><span>{t('web.settings.sound')}</span><span class="sub">{t('web.settings.soundSub')}</span></div>
        <Switch checked={prefs.audio} label={t('web.settings.sound')} onchange={(v) => (prefs.audio = v)} />
      </div>
      <div class="row">
        <div class="text"><span>{t('web.settings.clipboard')}</span><span class="sub">{t('web.settings.clipboardSub')}</span></div>
        <Switch checked={prefs.clipboard} label={t('web.settings.clipboard')} onchange={(v) => (prefs.clipboard = v)} />
      </div>
    </section>

    {#if !isTouch}
      <section>
        <span class="lbl">{t('web.settings.mouse')}</span>
        <Segmented
          label={t('web.settings.mouse')}
          options={[
            { value: 'auto', label: t('web.settings.auto') },
            { value: 'bureau', label: t('web.settings.desktop') },
            { value: 'jeu', label: t('web.settings.game') },
          ]}
          value={prefs.mouse}
          onchange={(v: MouseMode) => (prefs.mouse = v)}
        />
        <p class="sub">
          {prefs.mouse === 'auto'
            ? t('web.settings.mouseAuto')
            : prefs.mouse === 'jeu'
              ? t('web.settings.mouseGame')
              : t('web.settings.mouseDesktop')}
        </p>
      </section>
    {/if}

    {#if isTouch}
      <section>
        <span class="lbl">{t('web.settings.touch')}</span>
        <Segmented
          label={t('web.settings.touch')}
          options={[
            { value: 'trackpad', label: t('web.settings.trackpad') },
            { value: 'direct', label: t('web.settings.direct') },
          ]}
          value={prefs.touch}
          onchange={(v: TouchMode) => (prefs.touch = v)}
        />
        <p class="sub">{prefs.touch === 'trackpad' ? t('web.settings.trackpadDesc') : t('web.settings.directDesc')}</p>
      </section>
    {/if}

    <section>
      <span class="lbl">{t('web.settings.language')}</span>
      <LangChoice />
      <p class="sub">{t('web.settings.languageHint')}</p>
    </section>

    <section>
      <button type="button" class="more" aria-expanded={advanced} onclick={() => (advanced = !advanced)}>
        <span class="text"><span>{t('web.settings.advanced')}</span><span class="sub">{t('web.settings.advancedSub')}</span></span>
        <svg class:open={advanced} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
      </button>
      {#if advanced}
        <div class="row">
          <div class="text"><span>{t('web.settings.path')}</span><span class="sub">{pathText}</span></div>
        </div>
        <div class="row">
          <div class="text"><span>{t('web.settings.transport')}</span><span class="sub">{t('web.settings.transportSub')}</span></div>
        </div>
        <Segmented
          label={t('web.settings.transport')}
          options={[
            { value: 'auto', label: t('web.settings.auto') },
            { value: 'fiable', label: t('web.settings.reliable') },
            { value: 'rapide', label: t('web.settings.fast') },
          ]}
          value={prefs.transport}
          onchange={(v: Transport) => (prefs.transport = v)}
        />
        <div class="row">
          <div class="text"><span>{t('web.settings.format')}</span><span class="sub">{isFirefox ? t('web.settings.formatFirefox') : t('web.settings.formatAuto')}</span></div>
        </div>
        <Segmented
          label={t('web.settings.format')}
          options={[
            { value: 'auto', label: t('web.settings.auto') },
            { value: 'h264', label: 'H.264' },
            { value: 'hevc', label: 'HEVC' },
          ]}
          value={prefs.codec}
          onchange={(v: Codec) => (prefs.codec = v)}
        />
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
    width: min(440px, calc(100% - 32px));
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
    gap: 24px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .card {
    min-height: 108px;
    border: 0;
    border-radius: 14px;
    padding: 14px;
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
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--muted);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 44px;
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

  .auto {
    margin-top: -4px;
  }

  .more {
    min-height: 52px;
    border: 0;
    border-radius: 12px;
    background: var(--ink-3);
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
  }

  .more svg {
    transition: transform 0.2s;
  }

  .more svg.open {
    transform: rotate(90deg);
  }

  @media (max-width: 520px) {
    .sheet {
      left: 8px;
      right: 8px;
      top: auto;
      bottom: 8px;
      width: auto;
      max-height: 86vh;
    }
  }
</style>
