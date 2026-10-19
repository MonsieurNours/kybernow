<script lang="ts">
  import { api, type DisplayStatus, type Trigger, type VdSettings } from '../lib/api'
  import InfoTip from '../lib/InfoTip.svelte'
  import { errorText, notify } from '../lib/store.svelte'

  const PRESETS = [
    { label: '1920 × 1080 · 60 Hz', width: 1920, height: 1080, refresh: 60 },
    { label: '1920 × 1080 · 120 Hz', width: 1920, height: 1080, refresh: 120 },
    { label: '2560 × 1440 · 60 Hz', width: 2560, height: 1440, refresh: 60 },
    { label: '2560 × 1440 · 144 Hz', width: 2560, height: 1440, refresh: 144 },
    { label: '2560 × 1600 · 120 Hz (MacBook)', width: 2560, height: 1600, refresh: 120 },
    { label: '3024 × 1964 · 120 Hz (MacBook Pro 14")', width: 3024, height: 1964, refresh: 120 },
    { label: '3456 × 2234 · 120 Hz (MacBook Pro 16")', width: 3456, height: 2234, refresh: 120 },
    { label: '3840 × 2160 · 60 Hz (Apple TV 4K)', width: 3840, height: 2160, refresh: 60 },
    { label: '3840 × 2160 · 120 Hz', width: 3840, height: 2160, refresh: 120 },
  ]

  const TRIGGERS: { value: Trigger; title: string; text: string }[] = [
    {
      value: 'apps',
      title: 'Avec les apps KyberNow',
      text: "L'app Mac ou Apple TV annonce sa résolution avant de se connecter : l'écran virtuel est créé exactement à sa taille.",
    },
    {
      value: 'all',
      title: 'Pour toute connexion',
      text: "Aussi pour le navigateur et le client en ligne de commande, qui n'annoncent pas leur écran : le mode par défaut ci-dessous est utilisé dès qu'un flux démarre.",
    },
    {
      value: 'off',
      title: 'Jamais',
      text: "Vos appareils voient les écrans physiques de l'ordinateur, à leur résolution actuelle.",
    },
  ]

  let status = $state<DisplayStatus | null>(null)
  let form = $state<VdSettings | null>(null)
  let testPreset = $state(4)
  let testSeconds = $state(20)
  let busy = $state(false)

  const dirty = $derived(
    !!status && !!form && JSON.stringify(form) !== JSON.stringify(status.settings),
  )
  const presetIndex = $derived(
    form ? PRESETS.findIndex((p) => p.width === form!.width && p.height === form!.height && p.refresh === form!.refresh) : -1,
  )

  async function load() {
    try {
      status = await api.display()
      if (!form) form = { ...status.settings }
    } catch {
      // erreur 401 gérée globalement
    }
  }

  $effect(() => {
    load()
    const timer = setInterval(load, 2000)
    return () => clearInterval(timer)
  })

  async function run(fn: () => Promise<DisplayStatus>, ok?: string) {
    busy = true
    try {
      status = await fn()
      if (ok) notify('ok', ok)
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  async function save() {
    if (!form) return
    await run(() => api.displaySettings(form!), 'Réglages de l’écran virtuel enregistrés.')
    if (status) form = { ...status.settings }
  }

  function choosePreset(i: number) {
    if (!form || i < 0) return
    const p = PRESETS[i]
    form.width = p.width
    form.height = p.height
    form.refresh = p.refresh
  }

  const originLabel = (o: string, user: string | null) =>
    o === 'app' ? `app KyberNow${user ? ` (${user})` : ''}` : o === 'test' ? 'test depuis la console' : 'connexion détectée'

  const since = (t: number) => {
    const s = Math.max(0, Math.round(Date.now() / 1000 - t))
    return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`
  }
</script>

<header>
  <span class="eyebrow">Affichage à distance</span>
  <h1>Écran virtuel</h1>
  <p class="muted intro">
    Au lieu de vous envoyer les écrans physiques de l'ordinateur, KyberNow peut créer un écran à la taille exacte de l'appareil
    qui se connecte : pas de bandes noires, pas de mise à l'échelle, et vos écrans restent éteints pendant la session.
  </p>
</header>

<div class="phases">
  <div class="phase"><span class="num">1</span><div><strong>À la connexion</strong><span>Un écran virtuel est créé au mode de votre appareil, et il devient le seul écran actif.</span></div></div>
  <div class="phase"><span class="num">2</span><div><strong>Pendant la session</strong><span>Les écrans branchés à l'ordinateur sont éteints : personne ne voit ce que vous faites à distance.</span></div></div>
  <div class="phase"><span class="num">3</span><div><strong>À la déconnexion</strong><span>L'écran virtuel disparaît et vos écrans reviennent avec leur disposition habituelle.</span></div></div>
</div>

{#if status}
  <section class="card">
    <div class="row head">
      <h2>État</h2>
      <span class="spacer"></span>
      {#if status.session}
        <span class="badge ok">Écran virtuel actif</span>
      {:else if status.driver.available}
        <span class="badge">En attente d'une connexion</span>
      {:else}
        <span class="badge danger">Pilote indisponible</span>
      {/if}
    </div>

    {#if !status.supported}
      <p class="muted">Écran virtuel simulé : cette console ne tourne pas sous Windows.</p>
    {/if}

    <dl class="kv">
      <dt>Pilote SudoVDA</dt>
      <dd>
        {#if status.driver.available}
          prêt · protocole {status.driver.info?.version}
          <span class="muted">· chien de garde {status.driver.info?.watchdog_seconds} s</span>
          <InfoTip label="Chien de garde">
            <strong>Le chien de garde, votre filet de sécurité</strong>
            <span>
              KyberNow envoie un signal de vie au pilote chaque seconde. S'il s'arrête (plantage, mise à jour), le pilote
              retire l'écran virtuel tout seul au bout de quelques secondes, et Windows rallume vos écrans.
            </span>
          </InfoTip>
        {:else}
          <span class="err">{status.driver.error ?? 'introuvable'}</span>
        {/if}
      </dd>
      {#if status.session}
        <dt>Mode</dt>
        <dd>
          {status.session.applied.width ?? status.session.mode.width} × {status.session.applied.height ?? status.session.mode.height}
          à {status.session.applied.refresh ?? status.session.mode.refresh} Hz
          {#if status.session.applied.gdi_name}<span class="muted mono">· {status.session.applied.gdi_name}</span>{/if}
        </dd>
        <dt>Origine</dt>
        <dd>{originLabel(status.session.origin, status.session.user)}</dd>
        <dt>Depuis</dt>
        <dd>
          {since(status.session.started_at)}
          {#if status.session.seconds_left !== null}<span class="muted">· retour automatique dans {status.session.seconds_left} s</span>{/if}
        </dd>
        <dt>Écrans éteints</dt>
        <dd>{status.session.applied.disabled_others ?? 0}</dd>
      {/if}
    </dl>

    {#if status.last_error}
      <div class="notice danger">{status.last_error}</div>
    {/if}

    {#if status.session}
      <div class="row actions">
        <button class="danger" disabled={busy} onclick={() => run(() => api.displayStop(), 'Écrans physiques restaurés.')}>
          Rallumer mes écrans maintenant
        </button>
      </div>
    {/if}
  </section>
{/if}

{#if form}
  <section class="card">
    <div class="row head">
      <h2>Quand créer l'écran virtuel ?</h2>
      <span class="spacer"></span>
      <button class="primary" disabled={!dirty || busy} onclick={save}>Enregistrer</button>
    </div>
    <div class="triggers" role="radiogroup" aria-label="Déclenchement de l'écran virtuel">
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

    <div class="default">
      <div>
        <div class="label">
          <strong>Mode par défaut</strong>
          <InfoTip label="Mode par défaut">
            <strong>Mode par défaut</strong>
            <span>
              Utilisé quand l'appareil n'annonce pas son écran : client web, client Kyber en ligne de commande, ou
              « Pour toute connexion ». Les apps KyberNow, elles, envoient toujours leur propre mode.
            </span>
            <span><strong class="accent">Conseil</strong> Choisissez la résolution de l'écran sur lequel vous regardez le plus souvent.</span>
          </InfoTip>
        </div>
        <p class="muted">Résolution et fréquence pour les appareils qui ne précisent pas les leurs.</p>
      </div>
      <div class="mode">
        <select aria-label="Mode par défaut" value={String(presetIndex)} onchange={(e) => choosePreset(Number(e.currentTarget.value))}>
          {#each PRESETS as p, i (p.label)}
            <option value={String(i)}>{p.label}</option>
          {/each}
          {#if presetIndex < 0}<option value="-1">Personnalisé</option>{/if}
        </select>
        <div class="custom">
          <input type="number" aria-label="Largeur" min="640" max="7680" bind:value={form.width} />
          <span>×</span>
          <input type="number" aria-label="Hauteur" min="480" max="4320" bind:value={form.height} />
          <span>à</span>
          <input type="number" aria-label="Fréquence" min="24" max="500" bind:value={form.refresh} />
          <span>Hz</span>
        </div>
      </div>
    </div>
  </section>
{/if}

<section class="card">
  <div class="row head">
    <h2>Tester sans risque</h2>
  </div>
  <p class="muted">
    Crée l'écran virtuel quelques secondes puis rallume vos écrans automatiquement. Lancez le test depuis un autre
    appareil (votre Mac par exemple) : l'écran de l'ordinateur va s'éteindre pendant le test. Si vous êtes connecté au stream, vous
    devez voir l'image passer au nouveau mode.
  </p>
  <div class="row test">
    <select aria-label="Mode du test" bind:value={testPreset}>
      {#each PRESETS as p, i (p.label)}
        <option value={i}>{p.label}</option>
      {/each}
    </select>
    <select aria-label="Durée du test" bind:value={testSeconds}>
      <option value={10}>10 secondes</option>
      <option value={20}>20 secondes</option>
      <option value={60}>1 minute</option>
    </select>
    <button
      class="primary"
      disabled={busy || !!status?.session || !status?.driver.available}
      onclick={() => run(() => api.displayTest(PRESETS[testPreset], testSeconds), 'Écran virtuel créé pour le test.')}
    >
      {busy ? 'Création…' : 'Lancer le test'}
    </button>
  </div>
</section>

<style>
  .intro {
    max-width: 720px;
    font-size: 0.92rem;
  }

  .phases {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }

  .phase {
    display: flex;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: 0.86rem;
    line-height: 1.5;
    color: var(--muted);
  }

  .phase strong {
    display: block;
    color: var(--text);
  }

  .num {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    font-family: var(--mono);
    font-size: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .head {
    margin-bottom: 4px;
  }

  .err {
    color: var(--danger);
  }

  .mono {
    font-family: var(--mono);
    font-size: 0.82rem;
  }

  .actions {
    margin-top: 14px;
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

  .default {
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
    display: grid;
    grid-template-columns: 1fr 380px;
    gap: 20px;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .default p {
    font-size: 0.86rem;
  }

  .accent {
    color: var(--accent);
    margin-right: 4px;
  }

  .mode {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .custom {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.86rem;
    color: var(--muted);
  }

  .custom input {
    width: 90px;
  }

  .test {
    margin-top: 12px;
  }

  @media (max-width: 860px) {
    .phases,
    .triggers {
      grid-template-columns: 1fr;
    }

    .default {
      grid-template-columns: 1fr;
    }
  }
</style>
