<script lang="ts">
  import { api, type AudioChannels, type AudioSettings, type AudioStatus, type Trigger } from '../lib/api'
  import InfoTip from '../lib/InfoTip.svelte'
  import { errorText, notify } from '../lib/store.svelte'

  const TRIGGERS: { value: Trigger; title: string; text: string }[] = [
    {
      value: 'apps',
      title: 'Avec les apps KyberNow',
      text: "L'app Mac ou Apple TV demande le son au format de sa sortie : stéréo au casque, 5.1 ou 7.1 sur un ampli.",
    },
    {
      value: 'all',
      title: 'Pour toute connexion',
      text: 'Aussi pour le navigateur et le client en ligne de commande, au format choisi ci-dessous, dès qu’un flux démarre.',
    },
    {
      value: 'off',
      title: 'Jamais',
      text: "Le son reste sur la sortie de Windows : il sort aussi des haut-parleurs de l'ordinateur, au format réglé dans Windows.",
    },
  ]

  const CHANNELS: { value: AudioChannels; label: string; hint: string }[] = [
    { value: 'auto', label: 'Auto', hint: "Ce que demande l'appareil (stéréo s'il ne précise rien)." },
    { value: 'stereo', label: 'Stéréo', hint: 'Toujours deux canaux.' },
    { value: '5.1', label: '5.1', hint: 'Six canaux, même si l’appareil ne demande rien.' },
    { value: '7.1', label: '7.1', hint: 'Huit canaux, même si l’appareil ne demande rien.' },
  ]

  let status = $state<AudioStatus | null>(null)
  let form = $state<AudioSettings | null>(null)
  let busy = $state(false)

  const dirty = $derived(!!status && !!form && JSON.stringify(form) !== JSON.stringify(status.settings))
  const speakers = $derived(status?.devices.speakers ?? null)
  const channelsLabel = (n: number) => (n === 8 ? '7.1' : n === 6 ? '5.1' : n === 2 ? 'stéréo' : `${n} canaux`)

  async function load() {
    try {
      status = await api.audio()
      if (!form) form = { ...status.settings }
    } catch {
      // erreur 401 gérée globalement
    }
  }

  $effect(() => {
    load()
    const timer = setInterval(load, 3000)
    return () => clearInterval(timer)
  })

  async function save() {
    if (!form) return
    busy = true
    try {
      status = await api.audioSettings(form)
      form = { ...status.settings }
      notify('ok', 'Réglages du son enregistrés.')
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }
</script>

<header>
  <span class="eyebrow">Son à distance</span>
  <h1>Son</h1>
  <p class="muted intro">
    Pendant une session, le son de Windows part vers des haut-parleurs invisibles : l'ordinateur se tait, et votre appareil
    reçoit le son au bon format, de la stéréo au 7.1. À la fin, la sortie habituelle de l'ordinateur revient.
  </p>
</header>

{#if status}
  <section class="card">
    <div class="row head">
      <h2>État</h2>
      <span class="spacer"></span>
      {#if status.session}
        <span class="badge ok">Son envoyé vers l'appareil</span>
      {:else if speakers}
        <span class="badge">En attente d'une connexion</span>
      {:else}
        <span class="badge warn">Haut-parleurs virtuels absents</span>
      {/if}
    </div>

    {#if !status.supported}
      <p class="muted">Sorties audio simulées : cette console ne tourne pas sous Windows.</p>
    {/if}

    <dl class="kv">
      <dt>Haut-parleurs virtuels</dt>
      <dd>
        {#if speakers}
          {speakers.name} <span class="muted">· {channelsLabel(speakers.channels)}</span>
          <InfoTip label="Haut-parleurs virtuels">
            <strong>Pourquoi ceux de Steam ?</strong>
            <span>
              Un pilote audio doit être signé par Microsoft pour s'installer sans bidouille. Celui de Steam l'est, et
              sait faire de la stéréo au 7.1 : Sunshine et Apollo l'utilisent aussi.
            </span>
          </InfoTip>
        {:else}
          <span class="err">non installés</span>
        {/if}
      </dd>
      <dt>Sortie de Windows</dt>
      <dd>
        {#if status.devices.default}
          {status.devices.default.name} <span class="muted">· {channelsLabel(status.devices.default.channels)}</span>
        {:else}
          <span class="muted">{status.devices.error ?? 'aucune'}</span>
        {/if}
      </dd>
      {#if status.session}
        <dt>Format envoyé</dt>
        <dd>{channelsLabel(status.session.channels)}</dd>
      {/if}
    </dl>

    {#if !speakers}
      <div class="notice warn">
        Installez Steam sur cet ordinateur, puis relancez l'installateur KyberNow : il ajoutera les haut-parleurs virtuels. Sans
        eux, le son sort aussi des haut-parleurs de l'ordinateur, au format réglé dans Windows.
      </div>
    {/if}
    {#if status.last_error}
      <div class="notice danger">{status.last_error}</div>
    {/if}
  </section>
{/if}

{#if form}
  <section class="card">
    <div class="row head">
      <h2>Quand envoyer le son vers l'appareil ?</h2>
      <span class="spacer"></span>
      <button class="primary" disabled={!dirty || busy} onclick={save}>Enregistrer</button>
    </div>
    <div class="triggers" role="radiogroup" aria-label="Déclenchement du son à distance">
      {#each TRIGGERS as t (t.value)}
        <button role="radio" aria-checked={form.trigger === t.value} class="trigger" class:on={form.trigger === t.value} onclick={() => (form!.trigger = t.value)}>
          <span class="radio"></span>
          <span class="body">
            <strong>{t.title}</strong>
            <span>{t.text}</span>
          </span>
        </button>
      {/each}
    </div>

    <div class="channels">
      <div>
        <div class="label">
          <strong>Nombre de canaux</strong>
          <InfoTip label="Nombre de canaux">
            <strong>Stéréo, 5.1, 7.1</strong>
            <span>
              Les haut-parleurs virtuels sont réglés au nombre de canaux voulu au début de chaque session. Les jeux et les
              films en surround envoient alors chaque son sur son canal ; l'appareil qui reçoit réduit si sa propre sortie
              en a moins.
            </span>
            <span><strong class="accent">Conseil</strong> Laissez Auto : l'app sait ce que sa sortie accepte.</span>
          </InfoTip>
        </div>
        <p class="muted">{CHANNELS.find((c) => c.value === form!.channels)?.hint}</p>
      </div>
      <div class="seg" role="radiogroup" aria-label="Nombre de canaux">
        {#each CHANNELS as c (c.value)}
          <button role="radio" aria-checked={form.channels === c.value} class:on={form.channels === c.value} onclick={() => (form!.channels = c.value)}>{c.label}</button>
        {/each}
      </div>
    </div>
    <p class="muted small">
      La qualité (débit Opus) se règle dans <a href="#/settings">Réglages › Son</a>.
    </p>
  </section>
{/if}

<style>
  .intro {
    max-width: 720px;
    font-size: 0.92rem;
  }

  .head {
    margin-bottom: 4px;
  }

  .err {
    color: var(--danger);
  }

  .triggers {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .trigger {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    text-align: left;
    padding: 14px;
    border-radius: 12px;
    background: var(--surface-2);
    border: 1px solid transparent;
    font-weight: 400;
  }

  .trigger.on {
    border-color: var(--accent);
    background: #1a2a2a;
  }

  .radio {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    margin-top: 2px;
    border-radius: 50%;
    border: 2px solid var(--border-2);
  }

  .trigger.on .radio {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 3px #1a2a2a, inset 0 0 0 9px var(--accent);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.84rem;
    line-height: 1.5;
    color: var(--muted);
  }

  .body strong {
    font-size: 0.92rem;
    color: var(--text);
  }

  .channels {
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 20px;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .channels p {
    font-size: 0.86rem;
  }

  .accent {
    color: var(--accent);
    margin-right: 4px;
  }

  .seg {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
    border-radius: 11px;
    background: var(--surface-2);
    border: 1px solid var(--border);
  }

  .seg button {
    min-width: 70px;
    height: 34px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font-weight: 600;
  }

  .seg button.on {
    background: var(--accent);
    color: var(--accent-ink, #06251d);
  }

  .small {
    margin-top: 14px;
    font-size: 0.84rem;
  }

  @media (max-width: 860px) {
    .triggers,
    .channels {
      grid-template-columns: 1fr;
    }
  }
</style>
