<!--
  Barre de session au pointeur (docs/link-ui.md, « Barre de session ») : une capsule sombre centrée en
  haut de l'image, ses menus dessous. De gauche à droite, pour l'hôte : l'ordinateur, « Partager »,
  « Prêter l'ordinateur… », l'enregistrement, « Par le relais » | les mesures | le profil, le son, le
  micro, la souris, « Touches », « Presse-papiers » | plein écran, « Se déconnecter ». Réduite pour un
  invité (« Invité sur {nom} », son rôle, « Qualité de l'image », « Quitter le partage ») et pour l'ami
  qui emprunte (« Prêté par {hôte} », le temps qui reste, « Se déconnecter », « Rendre l'ordinateur »).
  Trop large pour la fenêtre : barre étroite, puis menu « Plus » (bar.svelte.ts, overflow.ts).
-->
<script lang="ts">
  import { untrack } from 'svelte'
  import Icon from '../lib/Icon.svelte'
  import Mark from '../lib/Mark.svelte'
  import { ms } from '../lib/format'
  import { i18n, t, tn } from '../lib/i18n/index.svelte'
  import { BAR_IDLE, type PointerBar } from './bar.svelte'
  import { isTouch } from './device'
  import { MAC_KEYS, PC_KEYS, type KeyEntry } from './keysmenu'
  import type { MoreItem } from './overflow'
  import { PROFILES, isCustom, type MouseMode } from './prefs'
  import type { LiveSession } from './livesession.svelte'
  import { QUALITIES, qualityName } from './share'

  let {
    s,
    bar,
    fullscreen,
    ontogglefullscreen,
  }: { s: LiveSession; bar: PointerBar; fullscreen: boolean; ontogglefullscreen: () => void } = $props()

  const m = $derived(s.metrics)
  const host = $derived(s.hosting)
  const guest = $derived(s.guest)
  const borrowing = $derived(s.borrowing)
  const rec = $derived(s.recording)
  const mic = $derived(s.mic)
  const narrow = $derived(bar.fit.narrow)
  const more = $derived(bar.fit.more)

  /** « 12,4 ms », ou « 60 i/s » avant la première latence. */
  const latencyText = $derived(m.latency != null ? ms(m.latency) : t('session.bar.fps', { fps: m.fps }))
  const sparkPath = $derived.by(() => {
    const v = m.spark
    if (v.length < 2) return ''
    const max = Math.max(20, ...v)
    return v.map((x, i) => `${i ? 'L' : 'M'}${((i / (v.length - 1)) * 64).toFixed(1)} ${(20 - (x / max) * 18).toFixed(1)}`).join(' ')
  })

  const profileIcon = $derived(isCustom(s.prefs) ? 'sliders' : s.prefs.profile === 'jeu' ? 'gamepad' : s.prefs.profile === 'max' ? 'bolt' : 'text')
  /** Le mode du moment : « Souris du bureau » ou « Souris des jeux » (en Auto aussi). */
  const mouseText = $derived(s.controls.gameMouse ? t('session.mouse.game') : t('session.mouse.desktop'))
  const mouseChoices = $derived<MouseMode[]>(s.g || s.b ? ['bureau', 'jeu'] : ['auto', 'bureau', 'jeu'])
  const mouseName = (mode: MouseMode) => (mode === 'auto' ? t('session.mouse.auto') : mode === 'jeu' ? t('session.mouse.game') : t('session.mouse.desktop'))
  const clipboard = $derived(!!s.plan?.clipboard)
  const keys = $derived<KeyEntry[]>(s.stationMac ? MAC_KEYS : PC_KEYS)
  const roleIcon = $derived(guest?.role === 'spectator' ? 'eye' : guest?.role === 'kbm' ? 'keyboard' : 'gamepad')
  const soundText = $derived(s.controls.muted ? t('session.sound.off') : t('session.sound.stereo'))

  /** Une commande passée dans « Plus » n'est plus dans la barre ; « Plus » prend la place de la première. */
  const shown = (id: MoreItem) => !more.includes(id)
  const moreAt = (id: MoreItem) => more[0] === id

  // La forme de la barre suit ce qu'elle contient et la langue ; la fenêtre, par `relayout` à chaque
  // changement de taille (Session.svelte).
  const content = $derived(
    [
      i18n.lang,
      s.title,
      s.formatText,
      s.profileText,
      s.relayed,
      s.canKbm,
      mic.shown,
      mic.lit,
      s.controls.muted,
      rec.state ? `${rec.state.active}` : '',
      host ? `${host.guestCount} ${host.requestCount} ${host.lendRequests.length} ${host.lendSupport}` : '',
      guest ? `${guest.roleText} ${guest.receivedText} ${guest.qualityKnown}` : '',
      borrowing?.timeText ?? '',
      s.controls.gameMouse,
    ].join('|'),
  )
  $effect(() => {
    void content
    untrack(() => void bar.relayout())
  })

  let moreButton = $state<HTMLElement | null>(null)
  /** Le dernier appui sur une commande est un doigt ou un stylet (la raison d'un refus en avis). */
  let touched = false
  const pointed = (e: PointerEvent) => (touched = e.pointerType === 'touch' || e.pointerType === 'pen')
  const takeTouch = () => {
    const was = touched || isTouch
    touched = false
    return was
  }

  /** « Partager », « Prêter l'ordinateur… » impossibles : à la souris, sans effet (l'infobulle dit
   * pourquoi) ; au toucher, le panneau réduit à la raison. */
  function openPanel(menu: 'share' | 'lend', ok: boolean, opener: HTMLElement) {
    if (!takeTouch() && !ok && bar.menu !== menu) return
    bar.toggle(menu, opener)
  }

  function pick(k: KeyEntry) {
    bar.close()
    s.pressKey(k)
  }
</script>

<!-- Souris : la fermeture suit le pointeur (bar.svelte.ts). Doigt : elle attend un temps sans toucher. -->
<div
  class="bar frost"
  class:hidden={!bar.visible}
  class:narrow
  role="toolbar"
  tabindex="-1"
  aria-label={t('session.bar.toolbar')}
  bind:this={bar.el}
  bind:clientWidth={bar.width}
  onpointerenter={(e) => e.pointerType === 'touch' && bar.clearTimer()}
  onpointerleave={(e) => e.pointerType === 'touch' && bar.scheduleHide(BAR_IDLE)}
>
  <Mark size={22} state="now" lit breathe />
  <span class="host">{s.title}</span>

  {#if guest}
    <span class="chip static role" title={guest.roleText}>
      <Icon name={roleIcon} size={16} stroke={1.8} />
      <span class="label">{guest.roleText}</span>
    </span>
  {:else if borrowing}
    <!-- Le temps qui reste au prêt (ou jusqu'à la reprise) ; ambre pendant le préavis. -->
    <span class="chip static time" class:soon={borrowing.noticeAt !== null} title={borrowing.timeText}>
      <Icon name="clock" size={16} stroke={1.8} />
      <span class="label">{borrowing.timeText}</span>
    </span>
    <span class="sep" aria-hidden="true"></span>
  {/if}

  {#if host}
    <!-- « Partager » (KyberNow Share) : inactif si la Station ne le sait pas ou l'a coupé. -->
    <button type="button" class="chip" class:inactive={host.shareSupport !== 'ok'} aria-expanded={bar.menu === 'share'} aria-disabled={host.shareSupport !== 'ok'} aria-label={host.shareAria} title={host.shareHint || t('share.bar.share')} onpointerdown={pointed} onclick={(e) => openPanel('share', host.shareSupport === 'ok', e.currentTarget)}>
      <Icon name="share" size={16} stroke={1.7} />
      <span class="label">{t('share.bar.share')}</span>
      {#if host.guestCount}<span class="count" aria-hidden="true"><span class="full">{tn('share.guests', host.guestCount)}</span><span class="num">{host.guestCount}</span></span>{/if}
      {#if host.requestCount}<span class="count ask" aria-hidden="true"><span class="full">{tn('share.requestCount', host.requestCount)}</span><span class="num">{host.requestCount}</span></span>{/if}
    </button>
    {#if host.lendSupport !== 'none'}
      <!-- « Prêter l'ordinateur… » (docs/pret.md) : inactif si le prêt est coupé dans Admin. -->
      <button type="button" class="chip" class:inactive={host.lendSupport !== 'ok'} aria-expanded={bar.menu === 'lend'} aria-disabled={host.lendSupport !== 'ok'} aria-label={host.lendAria} title={host.lendSupport === 'off' ? t('lend.off') : t('lend.open')} onpointerdown={pointed} onclick={(e) => openPanel('lend', host.lendSupport === 'ok', e.currentTarget)}>
        <Icon name="key" size={16} stroke={1.7} />
        <span class="label">{t('lend.open')}</span>
        {#if host.lendRequests.length}<span class="count ask" aria-hidden="true"><span class="full">{tn('share.requestCount', host.lendRequests.length)}</span><span class="num">{host.lendRequests.length}</span></span>{/if}
      </button>
    {/if}
  {/if}

  {#if rec.state && host}
    <!-- « Enregistrer » : pendant, la pastille rouge et son temps, un clic l'arrête ; coupé dans Admin :
         estompé, la raison en infobulle (au toucher, en avis). -->
    {#if rec.state.active}
      <button type="button" class="chip rec" title={rec.hint} aria-label="{rec.pill} · {rec.hint}" disabled={rec.busy} onclick={() => rec.toggle(false)}>
        <span class="beat" aria-hidden="true"></span>
        <span class="label">{rec.pill}</span>
        <span class="clock mono" aria-hidden="true">{rec.time}</span>
      </button>
    {:else}
      <button type="button" class="chip" class:inactive={!rec.state.allowed} aria-disabled={!rec.state.allowed} aria-label={t('recording.record')} title={rec.hint} disabled={rec.busy} onpointerdown={pointed} onclick={() => rec.toggle(takeTouch())}>
        <span class="dot-rec" aria-hidden="true"></span>
        <span class="label">{t('recording.record')}</span>
      </button>
    {/if}
  {/if}

  {#if s.relayed}
    <!-- Par le relais (docs/link-ui.md, « Hors de chez vous ») : toujours visible dans la barre. -->
    <span class="chip static relay" title={t('session.bar.relay')}>
      <Icon name="browser" size={16} stroke={1.8} />
      <span class="label">{t('session.bar.relay')}</span>
    </span>
  {/if}
  {#if guest && rec.state?.active}
    <!-- L'hôte enregistre : la pastille rouge, sans action. -->
    <span class="chip static rec" title={rec.hint} aria-label={rec.hint}>
      <span class="beat" aria-hidden="true"></span>
      <span class="label">{rec.pill}</span>
    </span>
  {/if}
  {#if s.plan?.pyrowave?.warning === 'relay'}
    <!-- PyroWave par le relais : la puce ambre reste dans la barre (docs/link-ui.md, « PyroWave »). -->
    <span class="chip static amber" title={t('session.pyrowave.titleRelay')}>
      <Icon name="warning" size={16} stroke={1.8} />
      <span class="label">{t('session.pyrowave.titleRelay')}</span>
    </span>
  {/if}

  {#if host}<span class="sep" aria-hidden="true"></span>{/if}
  <button type="button" class="lat" aria-expanded={bar.menu === 'stats'} aria-label={t('session.stats.title')} title={t('session.bar.statsHint', { shortcut: s.shortcut('S') })} onclick={() => bar.toggle('stats')}>
    <span class="mono value">{latencyText}</span>
    <svg width="64" height="22" viewBox="0 0 64 22" aria-hidden="true"><path d={sparkPath} fill="none" stroke="#9A8CFF" stroke-width="1.5" stroke-linejoin="round" /></svg>
    <span class="mono fmt">{s.formatText}</span>
  </button>
  <span class="sep" aria-hidden="true"></span>

  {#if host}
    <!-- Le profil de l'ordinateur, celui-ci coché ; « Personnalisé » quand les réglages s'en écartent. -->
    <button type="button" class="chip profile" aria-haspopup="menu" aria-expanded={bar.menu === 'profile'} title={t('session.bar.profileHint')} onclick={(e) => bar.toggle('profile', e.currentTarget)}>
      <Icon name={profileIcon} size={16} stroke={1.7} />
      <span class="label">{s.profileText}</span>
      <span class="caret" aria-hidden="true">▾</span>
    </button>
  {/if}

  {#if s.prefs.audio}
    <button type="button" class="chip" aria-pressed={s.controls.muted} aria-label={s.controls.muted ? t('session.sound.unmute') : t('session.sound.mute')} title={s.controls.muted ? t('session.sound.unmute') : t('session.sound.mute')} onclick={() => s.controls.toggleMute()}>
      <Icon name={s.controls.muted ? 'speakerOff' : 'speaker'} size={16} stroke={1.7} />
      <span class="label">{soundText}</span>
    </button>
  {:else}
    <span class="chip static" title={t('session.sound.off')}>
      <Icon name="speakerOff" size={16} stroke={1.7} />
      <span class="label">{t('session.sound.off')}</span>
    </span>
  {/if}

  {#if mic.shown}
    {#if moreAt('micro')}{@render moreChip()}{/if}
    {#if shown('micro')}
      <!-- « Micro » : allumé tant qu'il est envoyé ; inactif avec la raison quand l'ordinateur ne le prend pas. -->
      <button type="button" class="chip" data-more="micro" class:inactive={!!mic.refusal} aria-pressed={mic.lit} aria-disabled={!!mic.refusal} aria-label={mic.tip} title={mic.tip} onpointerdown={pointed} onclick={() => mic.toggle(takeTouch())}>
        <Icon name={mic.lit ? 'mic' : 'micOff'} size={16} stroke={1.7} />
        <span class="label">{mic.lit ? t('micro.label') : t('micro.label_off')}</span>
      </button>
    {/if}
  {/if}

  {#if guest?.qualityKnown}
    <!-- « Qualité de l'image » : la qualité reçue ; le menu la change (l'image de l'invité seulement). -->
    <button type="button" class="chip" aria-haspopup="menu" aria-expanded={bar.menu === 'quality'} aria-label="{t('share.quality.title')} · {guest.receivedText}" title={t('share.quality.title')} disabled={guest.qualityBusy} onclick={(e) => bar.toggle('quality', e.currentTarget)}>
      <Icon name="display" size={16} stroke={1.7} />
      <span class="label">{guest.receivedText}</span>
      <span class="caret" aria-hidden="true">▾</span>
    </button>
  {/if}

  {#if s.canKbm}
    {#if moreAt('mouse')}{@render moreChip()}{/if}
    {#if shown('mouse')}
      <button type="button" class="chip" data-more="mouse" aria-haspopup="menu" aria-expanded={bar.menu === 'mouse'} title={t('session.mouse.hint', { shortcut: s.shortcut('M') })} onclick={(e) => bar.toggle('mouse', e.currentTarget)}>
        <Icon name={s.controls.gameMouse ? 'crosshair' : 'pointer'} size={16} stroke={1.7} />
        <span class="label">{mouseText}</span>
        <span class="caret" aria-hidden="true">▾</span>
      </button>
    {/if}
    {#if moreAt('keys')}{@render moreChip()}{/if}
    {#if shown('keys')}
      <button type="button" class="chip" data-more="keys" aria-haspopup="menu" aria-expanded={bar.menu === 'keys'} title={t('session.bar.keys')} onclick={(e) => bar.toggle('keys', e.currentTarget)}>
        <Icon name="keyboard" size={16} stroke={1.7} />
        <span class="label">{t('session.bar.keys')}</span>
        <span class="caret" aria-hidden="true">▾</span>
      </button>
    {/if}
  {/if}

  {#if host}
    {#if moreAt('clipboard')}{@render moreChip()}{/if}
    {#if shown('clipboard')}
      <!-- « Presse-papiers » : une pastille, sans action ; estompée quand il n'est pas partagé. -->
      <span class="chip static" data-more="clipboard" class:inactive={!clipboard} role="img" aria-label={clipboard ? t('session.bar.clipboardOn') : t('session.bar.clipboardOff')} title={clipboard ? t('session.bar.clipboardOn') : t('session.bar.clipboardOff')}>
        <Icon name="clipboard" size={16} stroke={1.7} />
        <span class="label">{t('session.bar.clipboard')}</span>
      </span>
    {/if}
  {/if}

  <span class="sep" aria-hidden="true"></span>
  {#if moreAt('fullscreen')}{@render moreChip()}{/if}
  {#if shown('fullscreen')}
    <button type="button" class="chip icon" data-more="fullscreen" aria-pressed={fullscreen} aria-label={t('session.bar.fullscreen')} title={t('session.bar.fullscreenHint', { shortcut: s.shortcut('F') })} onclick={ontogglefullscreen}>
      <Icon name={fullscreen ? 'fullscreenExit' : 'fullscreen'} size={16} stroke={1.7} />
    </button>
  {/if}

  {#if borrowing}
    <!-- « Se déconnecter » discret (le prêt continue, on peut revenir), puis « Rendre l'ordinateur ». -->
    <button type="button" class="chip quiet" aria-label={t('session.bar.quit')} title={t('session.bar.quitHint', { shortcut: s.shortcut('D') })} onclick={() => s.quit()}>
      <Icon name="power" size={16} stroke={1.9} />
      <span class="label">{t('session.bar.quit')}</span>
    </button>
    <button type="button" class="quit" aria-expanded={bar.menu === 'giveBack'} aria-label={t('lend.bar.giveBack')} title={t('lend.bar.giveBack')} onclick={(e) => bar.toggle('giveBack', e.currentTarget)}>
      <Icon name="key" size={15} stroke={2} />
      <span class="label">{t('lend.bar.giveBack')}</span>
    </button>
  {:else}
    <button type="button" class="quit" aria-label={guest ? t('share.guest.leave') : t('session.bar.quit')} title={t(guest ? 'share.guest.leaveHint' : 'session.bar.quitHint', { shortcut: s.shortcut('D') })} onclick={() => s.quit()}>
      <Icon name="power" size={15} stroke={2} />
      <span class="label">{guest ? t('share.guest.leave') : t('session.bar.quit')}</span>
    </button>
  {/if}

  {#if bar.menu === 'giveBack' && borrowing}
    <div class="menu frost giveback" role="alertdialog" aria-labelledby="giveback-q">
      <p id="giveback-q">{t('lend.bar.confirm', { host: borrowing.host })}</p>
      <div class="gb-row">
        <button type="button" class="gb-cancel" onclick={() => bar.close()}>{t('share.cancel')}</button>
        <button type="button" class="gb-ok" onclick={() => { bar.close(); borrowing.giveBack() }}>{t('lend.bar.giveBack')}</button>
      </div>
    </div>
  {:else if bar.menu === 'quality' && guest}
    <div class="menu frost" role="menu" aria-label={t('share.quality.title')} style:left="{bar.menuLeft}px">
      {#each QUALITIES as q (q)}
        <button type="button" role="menuitemradio" aria-checked={q === guest.quality} onclick={() => { bar.close(); void guest.pickQuality(q) }}>{qualityName(q)}{#if q === guest.quality}<Icon name="check" size={15} stroke={2} />{/if}</button>
      {/each}
    </div>
  {:else if bar.menu === 'profile' && host}
    <div class="menu frost" role="menu" aria-label={t('session.bar.profileHint')} style:left="{bar.menuLeft}px">
      {#each PROFILES as p (p)}
        {@const on = !isCustom(s.prefs) && s.prefs.profile === p}
        <button type="button" role="menuitemradio" aria-checked={on} onclick={() => { bar.close(); s.changeProfile(p) }}>{t(`web.profile.${p}`)}{#if on}<Icon name="check" size={15} stroke={2} />{/if}</button>
      {/each}
    </div>
  {:else if bar.menu === 'mouse' && s.canKbm}
    <div class="menu frost" role="menu" aria-label={t('session.mouse.name')} style:left="{bar.menuLeft}px">
      {#each mouseChoices as choice (choice)}
        <button type="button" role="menuitemradio" aria-checked={s.controls.mouseMode === choice} onclick={() => { bar.close(); s.controls.setMouseMode(choice) }}>{mouseName(choice)}{#if s.controls.mouseMode === choice}<Icon name="check" size={15} stroke={2} />{/if}</button>
      {/each}
    </div>
  {:else if bar.menu === 'keys' && s.canKbm}
    <!-- Les dix entrées d'un PC (face à une Station Mac, ses cinq), chacune avec son indication dessous. -->
    <div class="menu frost keys" role="menu" aria-label={t('session.keys.title')} style:left="{bar.menuLeft}px">
      {#each keys as k (k.label)}
        <button type="button" role="menuitem" onclick={() => pick(k)}><span>{k.label}</span>{#if k.sub}<span class="sub">{k.sub}</span>{/if}</button>
      {/each}
    </div>
  {:else if bar.menu === 'more'}
    <!-- « Plus » : les commandes qui ne tiennent plus dans la barre, avec leur nom et leur état. -->
    <div class="menu frost" role="menu" aria-label={t('session.bar.moreHint')} style:left="{bar.menuLeft}px">
      {#each more as id (id)}
        {#if id === 'keys'}
          <button type="button" role="menuitem" aria-haspopup="menu" onclick={() => bar.toggle('keys', moreButton)}><span>{t('session.bar.keys')}</span><Icon name="chevron" size={14} stroke={2} /></button>
        {:else if id === 'mouse'}
          <button type="button" role="menuitem" aria-haspopup="menu" onclick={() => bar.toggle('mouse', moreButton)}><span>{t('session.mouse.name')}<span class="sub">{mouseText}</span></span><Icon name="chevron" size={14} stroke={2} /></button>
        {:else if id === 'micro'}
          <button type="button" role="menuitemcheckbox" aria-checked={mic.lit} aria-disabled={!!mic.refusal} title={mic.tip} onclick={() => mic.toggle(false)}><span>{t('micro.label')}</span>{#if mic.lit}<Icon name="check" size={15} stroke={2} />{/if}</button>
        {:else if id === 'clipboard'}
          <button type="button" role="menuitemcheckbox" class="static" aria-checked={clipboard} aria-disabled="true" title={clipboard ? t('session.bar.clipboardOn') : t('session.bar.clipboardOff')}><span>{t('session.bar.clipboard')}</span>{#if clipboard}<Icon name="check" size={15} stroke={2} />{/if}</button>
        {:else}
          <button type="button" role="menuitemcheckbox" aria-checked={fullscreen} title={t('session.bar.fullscreenHint', { shortcut: s.shortcut('F') })} onclick={() => { bar.close(); ontogglefullscreen() }}><span>{t('session.bar.fullscreen')}</span>{#if fullscreen}<Icon name="check" size={15} stroke={2} />{/if}</button>
        {/if}
      {/each}
    </div>
  {/if}
</div>

{#snippet moreChip()}
  <button type="button" class="chip" bind:this={moreButton} aria-haspopup="menu" aria-expanded={bar.menu === 'more'} aria-label={t('session.bar.more')} title={t('session.bar.moreHint')} onclick={(e) => bar.toggle('more', e.currentTarget)}>
    <Icon name="more" size={16} stroke={2.4} />
    <span class="label">{t('session.bar.more')}</span>
  </button>
{/snippet}

<style>
  .frost {
    background: rgb(9 12 17 / 0.7);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  .bar {
    position: absolute;
    left: 50%;
    top: 14px;
    translate: -50% 0;
    height: 54px;
    padding: 0 8px 0 14px;
    border-radius: 14px;
    display: flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
    transition:
      translate 0.35s var(--ease-out),
      opacity 0.35s;
    z-index: 5;
  }

  .bar.hidden {
    translate: -50% -90px;
    opacity: 0;
  }

  .host {
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 14px;
    font-weight: 600;
    margin: 0 8px 0 6px;
  }

  .sep {
    width: 1px;
    height: 26px;
    background: rgb(255 255 255 / 0.12);
    margin: 0 2px;
  }

  .lat {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: inherit;
  }

  .lat[aria-expanded='true'] {
    background: rgb(154 140 255 / 0.16);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
  }

  .lat .value {
    font-size: 15px;
    font-weight: 500;
    color: var(--signal-light);
    min-width: 64px;
    text-align: right;
  }

  .fmt {
    font-size: 11px;
    color: var(--muted);
  }

  .chip {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 36px;
    padding: 0 11px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font-size: 13px;
  }

  button.chip:hover,
  .chip[aria-expanded='true'] {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  .chip.static {
    cursor: default;
  }

  .chip.role,
  .chip.relay,
  .chip.time {
    color: var(--signal-light);
  }

  .chip.time.soon {
    color: var(--warn-text);
  }

  .chip.amber {
    color: var(--warn-text);
    box-shadow: inset 0 0 0 1px var(--warn-line);
  }

  .caret {
    margin-left: -2px;
    font-size: 10px;
    color: var(--faint);
  }

  /* Impossible (la Station ne le sait pas, coupé, refusé) : estompé ; à la souris sans effet (l'infobulle
     dit pourquoi), au toucher la raison. */
  .chip.inactive {
    opacity: 0.5;
  }

  button.chip.inactive:hover {
    background: none;
    color: var(--text-2);
  }

  /* Pastilles : « 2 invités », « 1 demande ». */
  .count {
    margin-left: 2px;
    padding: 2px 9px;
    border-radius: 10px;
    background: rgb(154 140 255 / 0.2);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
  }

  .count .num {
    display: none;
  }

  .count.ask {
    background: var(--warn-bg);
    box-shadow: inset 0 0 0 1px var(--warn);
    color: var(--warn-text);
  }

  .chip.icon {
    width: 40px;
    padding: 0;
    justify-content: center;
  }

  .chip.quiet {
    color: var(--muted);
  }

  .quit {
    height: 38px;
    padding: 0 14px;
    margin-left: 4px;
    border: 0;
    border-radius: 9px;
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
    font-size: 13px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* Barre étroite : les puces perdent leur texte, sauf le profil ; les pastilles se réduisent au nombre ;
     le format des mesures disparaît ; l'enregistrement garde son temps (« 02:14 »). */
  .bar.narrow .label,
  .bar.narrow .caret {
    display: none;
  }

  .bar.narrow .chip.profile .label,
  .bar.narrow .chip.profile .caret {
    display: inline;
  }

  .bar.narrow .chip:not(.profile):not(.rec),
  .bar.narrow .quit {
    width: 40px;
    padding: 0;
    justify-content: center;
  }

  .bar.narrow .fmt {
    display: none;
  }

  .bar.narrow .count {
    position: absolute;
    right: 1px;
    top: 1px;
    margin: 0;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--signal-strong);
    box-shadow: none;
    font-size: 10px;
    line-height: 16px;
    text-align: center;
  }

  .bar.narrow .count .full {
    display: none;
  }

  .bar.narrow .count .num {
    display: inline;
  }

  .bar.narrow .count.ask {
    right: auto;
    left: 1px;
    background: var(--warn);
    color: #1a1406;
  }

  .bar.narrow .chip.rec {
    padding: 0 10px;
  }

  .bar.narrow .chip.rec .clock {
    display: inline;
  }

  .menu {
    position: absolute;
    top: 62px;
    left: 8px;
    width: 300px;
    border-radius: 12px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    white-space: normal;
  }

  .menu button {
    min-height: 42px;
    padding: 6px 11px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font-size: 13px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    text-align: left;
  }

  .menu button:hover {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  .menu button.static:hover {
    background: none;
    color: var(--text-2);
  }

  .menu button > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  /* L'indication, en petit dessous. */
  .menu .sub {
    font-size: 11px;
    color: var(--faint);
  }

  .menu.keys button {
    justify-content: flex-start;
  }

  .menu button[aria-checked='true'] {
    color: #fff;
  }

  .menu button[aria-checked='true'] :global(svg) {
    color: var(--signal-light);
  }

  /* « Rendre l'ordinateur à {hôte} ? » sous son bouton. */
  .menu.giveback {
    left: auto;
    right: 8px;
    padding: 14px;
    gap: 12px;
  }

  .menu.giveback p {
    font-size: 14px;
    line-height: 1.4;
    color: var(--text);
  }

  .gb-row {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .menu .gb-row button {
    min-height: 38px;
    padding: 0 14px;
    justify-content: center;
    font-weight: 600;
  }

  .menu .gb-row .gb-ok {
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
  }

  .menu .gb-row .gb-cancel {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.14);
  }

  /* Enregistrement : la pastille rouge (--bad, le --kn-bad de la marque), son rond qui bat une fois par
     seconde ; « Enregistrer » : le rond plein. */
  .chip.rec {
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
    font-variant-numeric: tabular-nums;
  }

  button.chip.rec:hover {
    background: rgb(255 107 107 / 0.22);
    color: #fff;
  }

  .chip.rec .clock {
    display: none;
    font-size: 11.5px;
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

  .chip.inactive .dot-rec {
    background: var(--faint);
  }

  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }

    .beat {
      animation: none;
    }
  }
</style>
