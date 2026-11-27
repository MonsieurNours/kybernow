<!--
  Barre de session au toucher (iPhone, iPad, Android dans le navigateur) : une languette en haut de
  l'image l'ouvre ; la barre compacte donne la latence, le rôle ou le temps qui reste, le relais, le mode
  du toucher, le clavier de l'appareil, le son, le micro, « Partager », « Prêter l'ordinateur… »,
  l'enregistrement, « Qualité de l'image » et la fin de la session. La fiche des gestes vient à sa
  première ouverture, jamais à l'arrivée de l'image.
-->
<script lang="ts">
  import Icon from '../lib/Icon.svelte'
  import Mark from '../lib/Mark.svelte'
  import { ms } from '../lib/format'
  import { t, tn } from '../lib/i18n/index.svelte'
  import type { PointerBar } from './bar.svelte'
  import type { TouchMode } from './prefs'
  import type { LiveSession } from './livesession.svelte'
  import { QUALITIES, qualityName } from './share'

  let {
    s,
    bar,
    touchMode,
    onmode,
    onkeyboard,
  }: { s: LiveSession; bar: PointerBar; touchMode: TouchMode; onmode: (m: TouchMode) => void; onkeyboard: () => void } = $props()

  const m = $derived(s.metrics)
  const host = $derived(s.hosting)
  const guest = $derived(s.guest)
  const borrowing = $derived(s.borrowing)
  const rec = $derived(s.recording)
  const mic = $derived(s.mic)
  const latencyText = $derived(m.latency != null ? ms(m.latency) : t('session.bar.fps', { fps: m.fps }))
  /** Micro envoyé (ou en route) : une petite icône à côté de la languette, barre fermée. */
  const micSending = $derived(mic.shown && (mic.status === 'on' || mic.status === 'connecting'))

  function open() {
    bar.touchOpen = true
    bar.guide = s.canKbm && !s.prefs.touchGuideSeen
  }

  function closeBar() {
    bar.touchOpen = false
    bar.guide = false
    bar.close()
  }

  function closeGuide() {
    bar.guide = false
    s.prefs.touchGuideSeen = true
  }
</script>

{#if !bar.touchOpen}
  <button type="button" class="tab frost" aria-label={t('session.bar.show')} onpointerdown={(e) => e.preventDefault()} onclick={open}>
    <Icon name="chevron" size={14} stroke={2.6} />
    {#if micSending}<span class="tmic" aria-label={`${t('micro.label')} · ${mic.statusText}`} role="img"><Icon name="mic" size={13} stroke={2} /></span>{/if}
  </button>
{:else}
  <div class="tbar frost" role="toolbar" tabindex="-1" aria-label={t('session.bar.toolbar')} onpointerdown={(e) => e.preventDefault()}>
    <Mark size={20} state="now" lit />
    <span class="mono tlat">{latencyText}</span>
    {#if guest}<span class="trole">{guest.roleText}</span>{/if}
    {#if borrowing}<span class="trole ttime" class:soon={borrowing.noticeAt !== null}>{borrowing.timeText}</span>{/if}
    {#if guest && rec.state?.active}<span class="trec" role="img" aria-label={rec.hint}><span class="beat" aria-hidden="true"></span></span>{/if}
    {#if s.relayed}
      <span class="trelay" role="img" title={t('session.bar.relay')} aria-label={t('session.bar.relay')}><Icon name="browser" size={18} stroke={1.8} /></span>
    {/if}
    {#if guest?.qualityKnown}
      <button type="button" class="tb" aria-haspopup="menu" aria-expanded={bar.menu === 'quality'} aria-label="{t('share.quality.title')} · {guest.receivedText}" disabled={guest.qualityBusy} onclick={() => bar.toggle('quality')}>
        <Icon name="display" size={20} stroke={1.7} />
      </button>
    {/if}
    <span class="sep" aria-hidden="true"></span>
    {#if s.canKbm}
      <div class="tmodes" role="radiogroup" aria-label={t('session.touch.modes')}>
        <button type="button" role="radio" aria-checked={touchMode === 'trackpad'} onclick={() => onmode('trackpad')}>{t('session.touch.trackpad')}</button>
        <button type="button" role="radio" aria-checked={touchMode === 'direct'} onclick={() => onmode('direct')}>{t('session.touch.direct')}</button>
      </div>
      <button type="button" class="tb" onclick={onkeyboard} aria-label={t('session.touch.keyboard')}>
        <Icon name="keyboard" size={20} stroke={1.7} />
      </button>
    {/if}
    {#if s.prefs.audio}
      <button type="button" class="tb" aria-pressed={s.controls.muted} onclick={() => s.controls.toggleMute()} aria-label={s.controls.muted ? t('session.sound.unmute') : t('session.sound.mute')}>
        <Icon name={s.controls.muted ? 'speakerOff' : 'speaker'} size={20} stroke={1.7} />
      </button>
    {/if}
    {#if mic.shown}
      <!-- « Micro » : « Envoi… » jusqu'à la réponse de l'ordinateur, puis « Envoyé », ou la raison du refus (en avis). -->
      <button type="button" class="tb" class:inactive={!!mic.refusal} aria-pressed={mic.lit} aria-disabled={!!mic.refusal} aria-label={`${t('micro.label')} · ${mic.statusText}`} onclick={() => mic.toggle(true)}>
        <Icon name={mic.lit ? 'mic' : 'micOff'} size={20} stroke={1.7} />
      </button>
    {/if}
    {#if host}
      <button type="button" class="tb" class:inactive={host.shareSupport !== 'ok'} aria-expanded={bar.menu === 'share'} aria-disabled={host.shareSupport !== 'ok'} aria-label={host.shareAria} onclick={() => bar.toggle('share')}>
        <Icon name="share" size={20} stroke={1.7} />
        {#if host.guestCount}<span class="tcount mono" aria-hidden="true">{host.guestCount}</span>{/if}
        {#if host.requestCount}<span class="tcount ask mono" aria-hidden="true">{host.requestCount}</span>{/if}
      </button>
      {#if host.lendSupport !== 'none'}
        <button type="button" class="tb" class:inactive={host.lendSupport !== 'ok'} aria-expanded={bar.menu === 'lend'} aria-disabled={host.lendSupport !== 'ok'} aria-label={host.lendAria} onclick={() => bar.toggle('lend')}>
          <Icon name="key" size={20} stroke={1.7} />
          {#if host.lendRequests.length}<span class="tcount ask mono" aria-hidden="true">{host.lendRequests.length}</span>{/if}
        </button>
      {/if}
      {#if rec.state?.active}
        <button type="button" class="tb trec-on" aria-label="{rec.pill} · {t('recording.stop')}" disabled={rec.busy} onclick={() => rec.toggle(true)}>
          <span class="beat" aria-hidden="true"></span><span class="mono">{rec.time}</span>
        </button>
      {:else if rec.state}
        <button type="button" class="tb" class:inactive={!rec.state.allowed} aria-disabled={!rec.state.allowed} aria-label={t('recording.record')} disabled={rec.busy} onclick={() => rec.toggle(true)}>
          <span class="dot-rec" aria-hidden="true"></span>
        </button>
      {/if}
    {/if}
    <span class="sep" aria-hidden="true"></span>
    <button type="button" class="tb up" onclick={closeBar} aria-label={t('session.touch.hideBar')}>
      <Icon name="chevron" size={18} stroke={2} />
    </button>
    <button type="button" class="tb danger" onclick={() => (borrowing ? bar.toggle('giveBack') : s.quit())} aria-label={borrowing ? t('lend.bar.giveBack') : guest ? t('share.guest.leave') : t('session.bar.quit')}>
      <Icon name="power" size={18} stroke={2} />
    </button>
    {#if bar.menu === 'giveBack' && borrowing}
      <div class="tmenu frost giveback" role="alertdialog" aria-labelledby="giveback-tq">
        <p id="giveback-tq">{t('lend.bar.confirm', { host: borrowing.host })}</p>
        <div class="gb-row">
          <button type="button" class="gb-cancel" onclick={() => bar.close()}>{t('share.cancel')}</button>
          <button type="button" class="gb-ok" onclick={() => { bar.close(); borrowing.giveBack() }}>{t('lend.bar.giveBack')}</button>
        </div>
      </div>
    {:else if bar.menu === 'quality' && guest}
      <div class="tmenu frost" role="menu" aria-label={t('share.quality.title')}>
        <span class="mhead">{t('share.quality.title')} · {guest.receivedText}</span>
        {#each QUALITIES as q (q)}
          <button type="button" role="menuitemradio" aria-checked={q === guest.quality} onclick={() => { bar.close(); void guest.pickQuality(q) }}>{qualityName(q)}{#if q === guest.quality}<Icon name="check" size={16} stroke={2} />{/if}</button>
        {/each}
      </div>
    {/if}
  </div>
{/if}

{#if bar.guide}
  <section class="guide frost" aria-label={t('session.touch.gestures')}>
    <strong>{touchMode === 'trackpad' ? t('session.touch.trackpad') : t('session.touch.direct')}</strong>
    <p>{touchMode === 'trackpad' ? t('session.touch.guideTrackpad') : t('session.touch.guideDirect')}</p>
    <button type="button" onclick={closeGuide}>{t('session.touch.gotIt')}</button>
  </section>
{/if}

<style>
  .frost {
    background: rgb(9 12 17 / 0.7);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  .tab {
    position: absolute;
    left: 50%;
    top: 0;
    translate: -50% 0;
    width: 72px;
    height: 32px;
    border: 0;
    border-radius: 0 0 14px 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--signal-light);
    z-index: 5;
  }

  /* La languette descend la barre : son chevron pointe vers le bas. */
  .tab > :global(svg:first-child) {
    rotate: 90deg;
  }

  .tmic {
    display: grid;
    place-items: center;
    color: var(--signal-light);
  }

  .tbar {
    position: absolute;
    left: 50%;
    top: max(8px, env(safe-area-inset-top));
    translate: -50% 0;
    max-width: calc(100% - 16px);
    padding: 4px 6px 4px 12px;
    border-radius: 16px;
    display: flex;
    align-items: center;
    gap: 2px;
    white-space: nowrap;
    z-index: 5;
    animation: down 0.35s var(--ease-out) both;
  }

  @keyframes down {
    from {
      opacity: 0;
      translate: -50% -12px;
    }
    to {
      opacity: 1;
      translate: -50% 0;
    }
  }

  .tlat {
    font-size: 12px;
    color: var(--signal-light);
    margin: 0 8px;
    min-width: 46px;
  }

  .trole {
    font-size: 12px;
    color: var(--text-2);
    margin-right: 6px;
  }

  .ttime.soon {
    color: var(--warn-text);
  }

  .trelay {
    display: grid;
    place-items: center;
    margin-right: 6px;
    color: var(--signal-light);
  }

  .sep {
    width: 1px;
    height: 26px;
    background: rgb(255 255 255 / 0.12);
    margin: 0 2px;
  }

  .tmodes {
    display: flex;
    padding: 2px;
    margin: 0 4px;
    border-radius: 11px;
    background: rgb(255 255 255 / 0.06);
  }

  .tmodes button {
    height: 40px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--muted);
    font-size: 12.5px;
  }

  .tmodes button[aria-checked='true'] {
    background: rgb(154 140 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.6);
    color: #fff;
  }

  .tb {
    position: relative;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: 11px;
    background: none;
    color: var(--text-2);
    display: grid;
    place-items: center;
  }

  .tb[aria-pressed='true'] {
    color: var(--warn);
  }

  .tb.inactive {
    opacity: 0.5;
  }

  /* « Masquer la barre » : le chevron vers le haut. */
  .tb.up :global(svg) {
    rotate: -90deg;
  }

  .tcount {
    position: absolute;
    right: 3px;
    top: 3px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--signal-strong);
    color: #fff;
    font-size: 10px;
    line-height: 16px;
    text-align: center;
  }

  /* Demandes : en haut à gauche de l'icône, couleur d'une attente. */
  .tcount.ask {
    right: auto;
    left: 3px;
    background: var(--warn);
    color: #1a1406;
  }

  .danger {
    color: var(--bad-text);
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
  }

  .tb.trec-on {
    width: auto;
    padding: 0 10px;
    display: flex;
    gap: 6px;
    font-size: 12px;
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
    font-variant-numeric: tabular-nums;
  }

  .trec {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
  }

  .beat,
  .dot-rec {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--bad);
    flex: none;
  }

  .beat {
    animation: beat 1s ease-in-out infinite;
  }

  @keyframes beat {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  .dot-rec {
    width: 11px;
    height: 11px;
  }

  .tb.inactive .dot-rec {
    background: var(--faint);
  }

  .tmenu {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: 240px;
    border-radius: 12px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    white-space: normal;
  }

  .tmenu button {
    height: 42px;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font-size: 13px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .tmenu button[aria-checked='true'] {
    color: #fff;
  }

  .tmenu button[aria-checked='true'] :global(svg) {
    color: var(--signal-light);
  }

  .mhead {
    padding: 8px 11px 6px;
    font-size: 11.5px;
    color: var(--faint);
  }

  .tmenu.giveback {
    width: 300px;
    padding: 14px;
    gap: 12px;
  }

  .tmenu.giveback p {
    font-size: 14px;
    line-height: 1.4;
    color: var(--text);
  }

  .gb-row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .gb-row button {
    height: 38px;
    padding: 0 14px;
    justify-content: center;
    font-weight: 600;
  }

  .gb-row .gb-ok {
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
  }

  .gb-row .gb-cancel {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.14);
  }

  .guide {
    position: absolute;
    right: max(16px, env(safe-area-inset-right));
    bottom: 90px;
    width: min(340px, calc(100% - 32px));
    border-radius: 16px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
    z-index: 6;
  }

  .guide strong {
    font-size: 15px;
    color: var(--text);
  }

  .guide button {
    align-self: flex-end;
    height: 44px;
    padding: 0 18px;
    border: 0;
    border-radius: 10px;
    background: rgb(154 140 255 / 0.18);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.55);
    color: #fff;
    font-size: 14px;
    font-weight: 600;
  }

  @media (prefers-reduced-motion: reduce) {
    .tbar,
    .beat {
      animation: none;
    }
  }
</style>
