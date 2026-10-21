<!--
  Accueil : l'ordinateur choisi en plein cadre (son image, sinon sa signature),
  le bouton principal et le profil ; à droite la dernière session ; en bas les
  autres ordinateurs, ceux annoncés sur le réseau et l'ajout.
-->
<script lang="ts">
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import type { Computer, Found, Settings, ThisPc } from './lib/api'
  import { duration, hue, ms, profileLabel } from './lib/api'

  let {
    computers,
    discovered,
    thisPc,
    selected,
    busy,
    error,
    onselect,
    onconnect,
    onsettings,
    onprofile,
    onadd,
  }: {
    computers: Computer[]
    discovered: Found[]
    thisPc: ThisPc | null
    selected: Computer | null
    busy: boolean
    error: string | null
    onselect: (id: string) => void
    onconnect: (id: string) => void
    onsettings: (id: string) => void
    onprofile: (id: string, p: Settings['profile']) => void
    onadd: (found?: Found) => void
  } = $props()

  const fresh = $derived(discovered.filter((f) => !computers.some((c) => c.host === f.host && c.port === f.port)))

  const status = $derived.by(() => {
    if (!selected) return null
    if (selected.online === true) return { text: 'En ligne · disponible', tone: 'ok' }
    if (selected.online === false) return { text: 'Hors ligne', tone: 'off' }
    return { text: 'Recherche…', tone: 'wait' }
  })

  const meta = $derived(
    selected ? [selected.info?.os || selected.os, selected.info?.gpu ?? selected.gpu, selected.host].filter(Boolean).join(' · ') : '',
  )

  /** Ce que la session demandera à l'ordinateur. */
  const screen = $derived.by(() => {
    if (!selected || !thisPc) return null
    const s = selected.settings
    if (!s.virtual_display) return 'Écrans de l’ordinateur tels quels'
    let { width: w, height: h } = thisPc
    if (s.max_4k && (w > 3840 || h > 2160)) {
      const k = Math.min(3840 / w, 2160 / h)
      w = Math.round((w * k) / 2) * 2
      h = Math.round((h * k) / 2) * 2
    }
    const hz = s.battery === 'alleger' && thisPc.on_battery ? Math.min(30, thisPc.refresh) : thisPc.refresh
    return `${w} × ${h} · ${hz} Hz`
  })

  const frame = $derived(thisPc ? 1000 / Math.max(30, thisPc.refresh) : 16.7)
  const profiles: { value: Settings['profile']; label: string }[] = [
    { value: 'jeu', label: 'Jeu' },
    { value: 'bureau', label: 'Bureau' },
    { value: 'max', label: 'Max' },
  ]

  const signature = (fp: string | null) =>
    `radial-gradient(900px 600px at 78% 22%, hsl(${hue(fp)} 90% 62% / 0.38), transparent 70%), radial-gradient(700px 520px at 18% 78%, hsl(${hue(fp, 7)} 70% 55% / 0.16), transparent 70%), #070a0e`
</script>

<main>
  <div class="backdrop" class:offline={selected?.online === false}>
    {#if selected?.preview}
      {#key selected.preview}<img src={selected.preview} alt="" />{/key}
    {:else}
      <div class="signature" style:background={signature(selected?.fingerprint ?? null)}></div>
      <div class="watermark"><Mark size={760} state="console" seam="transparent" /></div>
    {/if}
    <div class="shade"></div>
  </div>

  <section class="hero">
    {#if selected}
      <div class="eyebrow status {status?.tone}"><span class="dot"></span>{status?.text}</div>
      <h1>{selected.name}</h1>
      <div class="meta mono">{meta}</div>
      {#if screen}
        <div class="chip">
          <Icon name="monitor" size={20} />
          {#if selected.settings.virtual_display}
            <span>Écran virtuel à <span class="mono">{screen}</span>, comme cet écran</span>
          {:else}
            <span>{screen}</span>
          {/if}
        </div>
      {/if}
      <div class="actions">
        <button class="primary" disabled={busy} onclick={() => onconnect(selected!.id)}>
          <Icon name="play" size={18} />Se connecter
        </button>
        <div class="profiles" role="radiogroup" aria-label="Profil">
          {#each profiles as p (p.value)}
            {@const on = selected.settings.profile === p.value}
            <button role="radio" aria-checked={on} class:on onclick={() => onprofile(selected!.id, p.value)}>{p.label}</button>
          {/each}
        </div>
        <button class="link" onclick={() => onsettings(selected!.id)}>Réglages de cet ordinateur</button>
      </div>
      <div class="note">
        {#if profileLabel(selected.settings) === 'Personnalisé'}Profil personnalisé dans les réglages ·{/if}
        {selected.preview ? 'Image affichée : aperçu de son écran' : 'Image affichée : sa signature'}
      </div>
      {#if error}<div class="error" role="alert">{error}</div>{/if}
    {:else}
      <div class="eyebrow">Bienvenue</div>
      <h1>Votre ordinateur, ici.</h1>
      <p class="lead">Ajoutez l’ordinateur où la console KyberNow est installée : sur le réseau, avec le lien de sa console ou par son adresse.</p>
      <div class="actions">
        <button class="primary" onclick={() => onadd()}><Icon name="plus" size={18} />Ajouter un ordinateur</button>
      </div>
      {#if error}<div class="error" role="alert">{error}</div>{/if}
    {/if}
  </section>

  {#if selected}
    <aside class="last">
      <div class="eyebrow">Dernière session</div>
      {#if selected.last}
        {@const l = selected.last}
        <div class="big"><span class="mono value">{ms(l.median_ms)}</span><span class="mono unit">ms médiane</span></div>
        <div class="ruler" aria-hidden="true">
          <div class="track"></div>
          <div class="fill" style:width="{Math.min(100, ((l.median_ms ?? 0) / frame) * 100)}%"></div>
          <div class="tick" style:left="{Math.min(100, ((l.median_ms ?? 0) / frame) * 100)}%"></div>
          {#if l.p95_ms != null}<div class="p95" style:left="{Math.min(100, (l.p95_ms / frame) * 100)}%"></div>{/if}
        </div>
        <div class="scale mono"><span>0</span><span>p95 {ms(l.p95_ms)}</span><span>{ms(frame)} ms · 1 image</span></div>
        <div class="sub">{duration(l.seconds)} · {l.label}</div>
      {:else}
        <div class="sub">Pas encore de session avec cet ordinateur.</div>
      {/if}
    </aside>
  {/if}

  <section class="row">
    <div class="eyebrow">Vos ordinateurs</div>
    <div class="cards">
      {#each computers as c (c.id)}
        <button class="card" class:current={c.id === selected?.id} class:offline={c.online === false} onclick={() => onselect(c.id)}>
          {#if c.preview}<img src={c.preview} alt="" />{:else}<span class="sig" style:background={signature(c.fingerprint)}></span>{/if}
          <span class="label">
            <span class="name">{c.name}</span>
            <span class="state">{c.online === true ? `En ligne${c.last?.median_ms != null ? ` · ${ms(c.last.median_ms)} ms` : ''}` : c.online === false ? 'Hors ligne' : '…'}</span>
          </span>
        </button>
      {/each}
      {#each fresh as f (f.id)}
        <button class="card found" onclick={() => onadd(f)}>
          <span class="badge eyebrow"><span class="pulse"></span>Sur le réseau</span>
          <span class="label">
            <span class="name">{f.name}</span>
            <span class="state">Ajouter cet ordinateur</span>
          </span>
        </button>
      {/each}
      <button class="card add" onclick={() => onadd()}>
        <Icon name="plus" size={24} />
        <span>Ajouter un ordinateur</span>
      </button>
    </div>
  </section>
</main>

<style>
  main {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    transition: filter 0.6s;
  }

  .backdrop.offline {
    filter: grayscale(0.85) brightness(0.8);
  }

  .backdrop img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation:
      reveal 0.9s var(--ease-out),
      drift 60s linear infinite alternate;
  }

  @keyframes reveal {
    from {
      clip-path: polygon(0 0, 0 0, 0 0);
      opacity: 0.4;
    }
    to {
      clip-path: polygon(0 0, 200% 0, 0 200%);
      opacity: 1;
    }
  }

  @keyframes drift {
    from {
      transform: scale(1.02);
    }
    to {
      transform: scale(1.08);
    }
  }

  .signature {
    position: absolute;
    inset: 0;
  }

  .watermark {
    position: absolute;
    right: -120px;
    top: 40px;
    opacity: 0.07;
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgb(5 7 10 / 0.6) 0%, rgb(5 7 10 / 0.1) 22%, rgb(5 7 10 / 0.15) 50%, rgb(5 7 10 / 0.92) 78%, #05070a 100%),
      linear-gradient(90deg, rgb(5 7 10 / 0.55) 0%, transparent 60%);
  }

  .hero {
    position: absolute;
    left: 72px;
    top: 112px;
    width: min(760px, calc(100% - 460px));
    min-width: 480px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--muted);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: var(--ghost);
  }

  .status.ok .dot {
    background: var(--ok);
  }

  .status.wait .dot {
    background: var(--signal);
    animation: blink 1.2s ease-in-out infinite;
  }

  @keyframes blink {
    50% {
      opacity: 0.3;
    }
  }

  h1 {
    margin: 0;
    font-family: var(--display);
    font-size: clamp(48px, 6vw, 84px);
    line-height: 0.95;
    font-weight: 640;
    font-stretch: 125%;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }

  .meta {
    font-size: 13px;
    color: var(--muted);
    line-height: 1.7;
  }

  .lead {
    margin: 0;
    max-width: 520px;
    font-size: 17px;
    line-height: 1.5;
    color: var(--muted);
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 12px;
    width: fit-content;
    padding: 12px 16px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.7);
    font-size: 14px;
    color: var(--text);
  }

  .chip :global(svg) {
    color: var(--signal);
  }

  .chip .mono {
    font-size: 13px;
  }

  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 16px;
    margin-top: 14px;
  }

  .profiles {
    display: flex;
    padding: 4px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.75);
  }

  .profiles button {
    height: 42px;
    padding: 0 18px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--muted);
    font-size: 14px;
  }

  .profiles button.on {
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--text);
    font-weight: 600;
  }

  .link {
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 14px;
    text-decoration: underline;
    text-underline-offset: 4px;
    padding: 10px 6px;
  }

  .link:hover {
    color: var(--text);
  }

  .note {
    font-size: 12px;
    color: var(--faint);
  }

  .last {
    position: absolute;
    right: 72px;
    top: 128px;
    width: 300px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 22px;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: rgb(9 13 18 / 0.72);
  }

  .big {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .value {
    font-size: 40px;
    font-weight: 500;
  }

  .unit {
    font-size: 14px;
    color: var(--muted);
  }

  .ruler {
    position: relative;
    height: 22px;
  }

  .ruler .track,
  .ruler .fill {
    position: absolute;
    left: 0;
    top: 10px;
    height: 2px;
  }

  .ruler .track {
    right: 0;
    background: var(--line-2);
  }

  .ruler .fill {
    background: var(--signal);
  }

  .ruler .tick {
    position: absolute;
    top: 3px;
    width: 2px;
    height: 16px;
    background: var(--text);
  }

  .ruler .p95 {
    position: absolute;
    top: 6px;
    width: 1px;
    height: 10px;
    background: var(--faint);
  }

  .scale {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: var(--faint);
  }

  .sub {
    font-size: 13px;
    color: var(--muted);
    line-height: 1.5;
  }

  .row {
    position: absolute;
    left: 72px;
    right: 72px;
    bottom: 40px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .cards {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(220px, 1fr);
    gap: 16px;
    overflow-x: auto;
    padding: 2px 2px 6px;
  }

  .card {
    position: relative;
    height: 132px;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-2);
    overflow: hidden;
    text-align: left;
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  .card:hover {
    border-color: var(--line-2);
  }

  .card img,
  .card .sig {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.55;
  }

  .card.offline img,
  .card.offline .sig {
    filter: grayscale(1);
    opacity: 0.3;
  }

  .card.current {
    border-color: var(--signal);
    box-shadow: 0 0 0 1px rgb(154 140 255 / 0.35), 0 0 22px rgb(109 91 255 / 0.25);
  }

  .card .label {
    position: absolute;
    left: 18px;
    right: 18px;
    bottom: 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-shadow: 0 1px 8px rgb(0 0 0 / 0.6);
  }

  .card .name {
    font-size: 17px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card .state {
    font-size: 12px;
    color: var(--muted);
  }

  .card.found {
    border-style: dashed;
    border-color: var(--line-2);
    background: var(--ink-1);
    animation: light 1.2s var(--ease-out);
  }

  @keyframes light {
    from {
      box-shadow: 0 0 0 1px var(--signal), 0 0 30px rgb(109 91 255 / 0.6);
    }
  }

  .badge {
    position: absolute;
    left: 18px;
    top: 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--signal);
    font-size: 10px;
  }

  .pulse {
    width: 6px;
    height: 6px;
    border-radius: 3px;
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
  }

  .card.add {
    background: var(--ink-1);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--muted);
    font-size: 14px;
  }

  .card.add:hover {
    color: var(--text);
  }

  .error {
    max-width: 620px;
  }
</style>
