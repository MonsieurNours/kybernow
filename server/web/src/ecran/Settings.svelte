<!-- Réglages du client web, gardés dans ce navigateur. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'
  import type { Mode } from './device'
  import { isTouch } from './device'
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

  const profiles: { id: Profile; label: string; desc: string }[] = [
    { id: 'bureau', label: 'Bureau', desc: 'Texte net, aucune image perdue, presse-papiers. Pour travailler.' },
    { id: 'jeu', label: 'Jeu', desc: 'La latence la plus basse et les manettes. Pour jouer.' },
  ]

  let advanced = $state(false)

  /** Chemin de l'image dans ce navigateur, et pourquoi. */
  const pathText =
    websocketReason === 'touch'
      ? 'WebSocket sur cet appareil tactile : c’est par cette connexion que partent le texte tapé et la position du doigt (un peu plus de latence).'
      : websocketReason === 'webkit'
        ? 'WebSocket dans Safari : son WebTransport ne sait pas vérifier le certificat de l’ordinateur (un peu plus de latence).'
        : websocketReason === 'unsupported'
          ? 'WebSocket : ce navigateur n’a pas WebTransport (un peu plus de latence).'
          : 'WebTransport, en direct ; sans image au bout de 12 s, la page passe par WebSocket.'
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
      <h2 id="settings-title" class="disp">Réglages</h2>
      <p class="muted">Gardés dans ce navigateur, pour {hostname}.</p>
    </div>
    <button type="button" class="close" aria-label="Fermer" onclick={onClose}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  </div>

  <div class="body">
    <section>
      <span class="lbl">Profil</span>
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
      <span class="lbl">Image</span>
      <div class="row">
        <div class="text">
          <span>À la taille de cet écran</span>
          <span class="sub">
            {prefs.virtualDisplay
              ? `L'ordinateur crée un écran de ${mode.width} × ${mode.height} à ${mode.refresh} Hz`
              : "Les écrans de l'ordinateur, tels quels"}
          </span>
        </div>
        <Switch checked={prefs.virtualDisplay} label="À la taille de cet écran" onchange={(v) => (prefs.virtualDisplay = v)} />
      </div>
      <div class="row">
        <div class="text">
          <span>Plein écran</span>
          <span class="sub">Toute la place pour l'image, dès la connexion</span>
        </div>
        <Switch checked={prefs.fullscreen} label="Plein écran" onchange={(v) => (prefs.fullscreen = v)} />
      </div>
      <div class="row">
        <div class="text">
          <span>Débit</span>
          <span class="sub">{prefs.bitrate ? 'Réglé à la main' : 'Suivant le profil et la taille de l’écran'}</span>
        </div>
        <Segmented
          label="Débit"
          options={[
            { value: 'auto', label: 'Auto' },
            { value: 'manual', label: 'Manuel' },
          ]}
          value={prefs.bitrate ? 'manual' : 'auto'}
          onchange={(v) => (prefs.bitrate = v === 'auto' ? null : (plan?.mbps ?? 40))}
        />
      </div>
      {#if prefs.bitrate}
        <div class="slider">
          <input type="range" min={MIN_MBPS} max={MAX_MBPS} step="5" bind:value={prefs.bitrate} aria-label="Débit en Mbit/s" />
          <span class="mono value">{prefs.bitrate} Mbit/s</span>
        </div>
      {:else if plan}
        <p class="mono value auto">{plan.mbps} Mbit/s · {plan.codec === 'hevc' ? 'HEVC' : 'H.264'}{plan.yuv444 ? ' 4:4:4' : ''}</p>
      {/if}
    </section>

    <section>
      <span class="lbl">Son et partage</span>
      <div class="row">
        <div class="text"><span>Son</span><span class="sub">Stéréo, depuis l'ordinateur</span></div>
        <Switch checked={prefs.audio} label="Son" onchange={(v) => (prefs.audio = v)} />
      </div>
      <div class="row">
        <div class="text"><span>Presse-papiers partagé</span><span class="sub">Le navigateur demandera l'autorisation une fois</span></div>
        <Switch checked={prefs.clipboard} label="Presse-papiers partagé" onchange={(v) => (prefs.clipboard = v)} />
      </div>
    </section>

    {#if !isTouch}
      <section>
        <span class="lbl">Souris</span>
        <Segmented
          label="Souris"
          options={[
            { value: 'auto', label: 'Auto' },
            { value: 'bureau', label: 'Bureau' },
            { value: 'jeu', label: 'Jeu' },
          ]}
          value={prefs.mouse}
          onchange={(v: MouseMode) => (prefs.mouse = v)}
        />
        <p class="sub">
          {prefs.mouse === 'auto'
            ? 'Capturée seulement quand un jeu est en plein écran sur l’ordinateur. Ctrl + Alt + Maj la rend.'
            : prefs.mouse === 'jeu'
              ? 'Capturée : déplacements bruts, pour viser. Ctrl + Alt + Maj la rend.'
              : 'Toujours libre : le pointeur se pose où vous le placez.'}
        </p>
      </section>
    {/if}

    {#if isTouch}
      <section>
        <span class="lbl">Toucher</span>
        <Segmented
          label="Toucher"
          options={[
            { value: 'trackpad', label: 'Pavé tactile' },
            { value: 'direct', label: 'Toucher direct' },
          ]}
          value={prefs.touch}
          onchange={(v: TouchMode) => (prefs.touch = v)}
        />
        <p class="sub">{prefs.touch === 'trackpad' ? 'Comme celui d’un portable : précis, pour le petit texte.' : 'Le doigt est la souris : rapide, pour les gros boutons.'}</p>
      </section>
    {/if}

    <section>
      <button type="button" class="more" aria-expanded={advanced} onclick={() => (advanced = !advanced)}>
        <span class="text"><span>Avancé</span><span class="sub">Format et transport : choisis pour vous</span></span>
        <svg class:open={advanced} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
      </button>
      {#if advanced}
        <div class="row">
          <div class="text"><span>Chemin de l'image</span><span class="sub">{pathText}</span></div>
        </div>
        <div class="row">
          <div class="text"><span>Transport</span><span class="sub">Fiable : chaque image arrive. Rapide : une image perdue est sautée.</span></div>
        </div>
        <Segmented
          label="Transport"
          options={[
            { value: 'auto', label: 'Auto' },
            { value: 'fiable', label: 'Fiable' },
            { value: 'rapide', label: 'Rapide' },
          ]}
          value={prefs.transport}
          onchange={(v: Transport) => (prefs.transport = v)}
        />
        <div class="row">
          <div class="text"><span>Format</span><span class="sub">Auto : HEVC quand la carte graphique de cet appareil le décode</span></div>
        </div>
        <Segmented
          label="Format"
          options={[
            { value: 'auto', label: 'Auto' },
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
