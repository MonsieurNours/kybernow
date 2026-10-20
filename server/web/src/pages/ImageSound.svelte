<!--
  Image et son : ce que les appareils reçoivent pendant une session (écran
  virtuel, son) et ce qui part d'eux (clavier, souris, manettes). Chaque
  choix s'applique tout de suite.
-->
<script lang="ts">
  import { api, type AudioChannels, type AudioStatus, type DisplayStatus, type Trigger } from '../lib/api'
  import { app, errorText, field, kyber, loadKyber, notify, setValues, value } from '../lib/store.svelte'
  import { mode } from '../lib/format'
  import Icon from '../lib/Icon.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'

  const PRESETS = [
    { width: 1920, height: 1080, refresh: 60 },
    { width: 1920, height: 1080, refresh: 120 },
    { width: 2560, height: 1440, refresh: 60 },
    { width: 2560, height: 1440, refresh: 144 },
    { width: 2560, height: 1600, refresh: 120 },
    { width: 3024, height: 1964, refresh: 120 },
    { width: 3456, height: 2234, refresh: 120 },
    { width: 3840, height: 2160, refresh: 60 },
    { width: 3840, height: 2160, refresh: 120 },
  ]

  const TRIGGERS: { value: Trigger; label: string }[] = [
    { value: 'apps', label: 'Avec les apps' },
    { value: 'all', label: 'Toujours' },
    { value: 'off', label: 'Jamais' },
  ]

  const CHANNELS: { value: AudioChannels; label: string; hint: string }[] = [
    { value: 'auto', label: 'Auto', hint: 'Auto suit ce que la sortie de l’appareil accepte. Un appareil qui en a moins réduit lui-même.' },
    { value: 'stereo', label: 'Stéréo', hint: 'Toujours deux canaux, quel que soit l’appareil.' },
    { value: '5.1', label: '5.1', hint: 'Six canaux, même si l’appareil ne demande rien : les jeux et les films en surround placent chaque son.' },
    { value: '7.1', label: '7.1', hint: 'Huit canaux, même si l’appareil ne demande rien.' },
  ]

  const INPUTS = ['input.clipboard_allow_copy', 'input.clipboard_allow_paste', 'input.gamepad_force_xbox360', 'input.keyboard_key_repeat']
  // Depuis KyberNow 0.2.6, les apps Mac et Apple TV choisissent par ordinateur
  // la manette vue par l'ordinateur (Xbox 360 ou fidèle) et l'imposent à la
  // session : l'option de la console n'est plus qu'un défaut pour les autres clients.
  const INPUT_NOTES: Record<string, string> = {
    'input.gamepad_force_xbox360':
      'Les apps Mac et Apple TV choisissent elles-mêmes, par ordinateur (réglage « Manette vue par l’ordinateur ») : ici, seulement le défaut pour les autres clients.',
  }

  let display = $state<DisplayStatus | null>(null)
  let audio = $state<AudioStatus | null>(null)
  let busy = $state(false)
  let custom = $state<{ width: number; height: number; refresh: number } | null>(null)

  async function load() {
    try {
      ;[display, audio] = await Promise.all([api.display(), api.audio()])
    } catch {
      // 401 gérée globalement
    }
  }

  $effect(() => {
    load()
    loadKyber()
    const timer = setInterval(load, 2500)
    return () => clearInterval(timer)
  })

  async function run<T>(fn: () => Promise<T>, ok?: string): Promise<T | null> {
    busy = true
    try {
      const r = await fn()
      if (ok) notify('ok', ok)
      return r
    } catch (e) {
      notify('error', errorText(e))
      return null
    } finally {
      busy = false
    }
  }

  async function setDisplay(patch: Partial<DisplayStatus['settings']>, ok: string) {
    if (!display) return
    const r = await run(() => api.displaySettings({ ...display!.settings, ...patch }), ok)
    if (r) display = r
  }

  async function setAudio(patch: Partial<AudioStatus['settings']>, ok: string) {
    if (!audio) return
    const r = await run(() => api.audioSettings({ ...audio!.settings, ...patch }), ok)
    if (r) audio = r
  }

  const presetIndex = $derived(
    display ? PRESETS.findIndex((p) => p.width === display!.settings.width && p.height === display!.settings.height && p.refresh === display!.settings.refresh) : -1,
  )

  function choosePreset(raw: string) {
    if (raw === 'custom') {
      custom = { width: display!.settings.width, height: display!.settings.height, refresh: display!.settings.refresh }
      return
    }
    custom = null
    setDisplay(PRESETS[Number(raw)], 'Mode par défaut enregistré.')
  }

  async function saveCustom(event: SubmitEvent) {
    event.preventDefault()
    if (!custom) return
    await setDisplay(custom, 'Mode par défaut enregistré.')
    custom = null
  }

  async function tryDisplay() {
    if (!display) return
    const r = await run(() => api.displayTest({ width: display!.settings.width, height: display!.settings.height, refresh: display!.settings.refresh }, 15))
    if (r) display = r
  }

  async function stopDisplay() {
    const r = await run(() => api.displayStop(), 'Vos écrans reviennent.')
    if (r) display = r
  }

  const displayPill = $derived.by(() => {
    if (!display) return null
    if (!display.supported) return { kind: 'warn', text: 'Simulé : cette console ne tourne pas sous Windows' }
    if (display.session) return { kind: 'signal', text: `${display.session.origin === 'test' ? 'Essai' : 'Actif'} · ${mode(display.session.mode)}` }
    if (!display.driver.available) return { kind: 'warn', text: 'Pilote indisponible' }
    return { kind: 'ok', text: `Prêt · pilote SudoVDA${display.driver.info ? ` ${display.driver.info.version}` : ''}` }
  })

  const audioPill = $derived.by(() => {
    if (!audio) return null
    if (!audio.supported) return { kind: 'warn', text: 'Simulé : cette console ne tourne pas sous Windows' }
    if (audio.session) return { kind: 'signal', text: `Envoyé · ${audio.session.channels === 8 ? '7.1' : audio.session.channels === 6 ? '5.1' : 'stéréo'}` }
    if (!audio.devices.speakers) return { kind: 'warn', text: 'Haut-parleurs virtuels absents' }
    return { kind: 'ok', text: 'Prêt · haut-parleurs de Steam' }
  })

  const quality = $derived(field('audio.quality'))
  const qualityValue = $derived(value<string>('audio.quality'))
  const qualityHint = $derived(quality?.kind.type === 'enum' ? (quality.kind.options.find((o) => o.value === qualityValue)?.hint ?? '') : '')
  const watchdog = $derived(display?.driver.info?.watchdog_seconds)
</script>

<header class="space-head with-tabs">
  <div>
    <span class="lbl">Pendant une session</span>
    <h1>Image et son</h1>
    <p>Ce que vos appareils reçoivent, et ce que devient cet ordinateur le temps d'une session. Les apps KyberNow règlent elles-mêmes la qualité de l'image : il n'y a rien à régler ici pour bien démarrer.</p>
    <nav class="tabs" aria-label="Sections">
      <a href="#/image-son/ecran" class:current={app.section === 'ecran' || !app.section}>Écran</a>
      <a href="#/image-son/son" class:current={app.section === 'son'}>Son</a>
      <a href="#/image-son/commandes" class:current={app.section === 'commandes'}>Clavier, souris et manettes</a>
    </nav>
  </div>
</header>

<div class="space-body">
  <!-- Écran -->
  <section id="ecran" aria-labelledby="t-ecran" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-ecran">Écran virtuel</h2>
        <span class="sub">Un écran à la taille exacte de l'appareil qui se connecte.</span>
      </div>
      {#if displayPill}<span class="pill {displayPill.kind}"><Icon name={displayPill.kind === 'warn' ? 'warning' : 'check'} size={13} stroke={2.6} />{displayPill.text}</span>{/if}
    </div>

    <div class="steps">
      <div><span class="lbl">1 · À la connexion</span><span>L'écran virtuel prend la définition et la fréquence de l'appareil.</span></div>
      <div><span class="lbl">2 · Pendant la session</span><span>Les écrans branchés s'éteignent : personne ne voit ce que vous faites.</span></div>
      <div><span class="lbl">3 · À la fin</span><span>Vos écrans reviennent, avec leur disposition habituelle.</span></div>
    </div>

    {#if display?.last_error}
      <div class="notice warn" role="alert"><Icon name="warning" /><span>{display.last_error}</span></div>
    {/if}
    {#if display && !display.driver.available && display.driver.error}
      <div class="notice warn" role="alert"><Icon name="warning" /><span>{display.driver.error}</span></div>
    {/if}

    {#if display}
      <div class="set">
        <div><h3>Quand créer l'écran virtuel</h3><p>Les apps KyberNow annoncent leur écran ; un navigateur ou un client en ligne de commande ne le fait pas, il reçoit alors le mode par défaut.</p></div>
        <Segmented label="Quand créer l'écran virtuel" options={TRIGGERS} value={display.settings.trigger} disabled={busy} onchange={(t) => setDisplay({ trigger: t }, t === 'off' ? 'Écran virtuel désactivé.' : 'Enregistré.')} />
      </div>
      <div class="set">
        <div><label class="title" for="mode">Mode par défaut</label><p>Pour les connexions qui n'annoncent pas leur écran.</p></div>
        <select id="mode" value={custom ? 'custom' : presetIndex >= 0 ? String(presetIndex) : 'custom'} disabled={busy} onchange={(e) => choosePreset(e.currentTarget.value)}>
          {#each PRESETS as p, i}<option value={String(i)}>{mode(p)}</option>{/each}
          <option value="custom">{presetIndex < 0 && !custom ? `${mode(display.settings)} (autre)` : 'Autre…'}</option>
        </select>
        {#if custom}
          <form class="custom" onsubmit={saveCustom}>
            <div class="field"><label for="w">Largeur</label><input id="w" type="number" min="640" max="7680" bind:value={custom.width} /></div>
            <div class="field"><label for="h">Hauteur</label><input id="h" type="number" min="480" max="4320" bind:value={custom.height} /></div>
            <div class="field"><label for="r">Fréquence (Hz)</label><input id="r" type="number" min="24" max="240" bind:value={custom.refresh} /></div>
            <button type="submit" class="ghost" disabled={busy}>Enregistrer</button>
          </form>
        {/if}
      </div>
      <div class="set">
        <div>
          <h3>Si quelque chose tourne mal</h3>
          <p>Le pilote retire l'écran virtuel tout seul s'il ne reçoit plus de signe de vie{watchdog ? ` pendant ${watchdog} s` : ''}, et Windows rallume vos écrans. Vous pouvez aussi le faire tout de suite.</p>
        </div>
        <div class="buttons">
          {#if display.session?.origin === 'test'}
            <button type="button" class="ghost" disabled={busy} onclick={stopDisplay}>Arrêter l'essai{display.session.seconds_left ? ` (${display.session.seconds_left} s)` : ''}</button>
          {:else}
            <button type="button" class="ghost" disabled={busy || !!display.session || !display.driver.available} onclick={tryDisplay}>Essayer pendant 15 s</button>
          {/if}
          <button type="button" class="ghost" disabled={busy || !display.session} onclick={stopDisplay}>Rallumer mes écrans</button>
        </div>
      </div>
    {/if}
  </section>

  <!-- Son -->
  <section id="son" aria-labelledby="t-son" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-son">Son</h2>
        <span class="sub">L'ordinateur se tait, votre appareil reçoit le son, de la stéréo au 7.1.</span>
      </div>
      {#if audioPill}<span class="pill {audioPill.kind}"><Icon name={audioPill.kind === 'warn' ? 'warning' : 'check'} size={13} stroke={2.6} />{audioPill.text}</span>{/if}
    </div>

    {#if audio && audio.supported && !audio.devices.speakers}
      <div class="notice warn"><Icon name="info" /><span>Installez Steam sur cet ordinateur, puis relancez l'installateur KyberNow : il ajoutera les haut-parleurs virtuels. Sans eux, le son sort aussi des haut-parleurs de l'ordinateur.</span></div>
    {/if}
    {#if audio?.last_error}
      <div class="notice warn" role="alert"><Icon name="warning" /><span>{audio.last_error}</span></div>
    {/if}

    {#if audio}
      <div class="set">
        <div><h3>Quand envoyer le son</h3><p>Sinon, le son sort des haut-parleurs de l'ordinateur, comme d'habitude.</p></div>
        <Segmented label="Quand envoyer le son" options={TRIGGERS} value={audio.settings.trigger} disabled={busy} onchange={(t) => setAudio({ trigger: t }, 'Enregistré.')} />
      </div>
      <div class="set">
        <div><h3>Canaux</h3><p>{CHANNELS.find((c) => c.value === audio!.settings.channels)?.hint}</p></div>
        <Segmented label="Canaux" options={CHANNELS} value={audio.settings.channels} disabled={busy} onchange={(c) => setAudio({ channels: c }, 'Enregistré.')} />
      </div>
    {/if}
    {#if quality?.kind.type === 'enum'}
      <div class="set">
        <div><h3>Qualité</h3><p>{qualityHint}</p></div>
        <Segmented label="Qualité du son" options={quality.kind.options} value={qualityValue} onchange={(v) => setValues({ 'audio.quality': v })} />
      </div>
    {/if}
  </section>

  <!-- Commandes -->
  <section id="commandes" aria-labelledby="t-cmd" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-cmd">Clavier, souris et manettes</h2>
        <span class="sub">Ce qui part de votre appareil vers cet ordinateur.</span>
      </div>
    </div>
    {#if kyber.loaded}
      {#each INPUTS as id (id)}
        {@const f = field(id)}
        {#if f}
          <div class="set">
            <div><h3>{f.label}</h3><p>{f.help}{f.caution ? ` ${f.caution}` : ''}{INPUT_NOTES[id] ? ` ${INPUT_NOTES[id]}` : ''}</p></div>
            <Switch label={f.label} checked={!!value(id)} onchange={(v) => setValues({ [id]: v })} />
          </div>
        {/if}
      {/each}
    {/if}
  </section>

  <p class="aside">L'encodeur, le moteur de capture et le débit minimum sont dans <a href="#/systeme/avance">Système › Avancé</a>.</p>
</div>

<style>
  .steps {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(220px, 100%), 1fr));
    gap: 10px;
    margin: 8px 0 14px;
  }

  .steps > div {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px;
    border-radius: 12px;
    background: var(--ink-2);
    font-size: 13.5px;
    line-height: 1.45;
    color: var(--text-2);
  }

  .steps .lbl {
    color: var(--signal-light);
  }

  .notice {
    margin-bottom: 14px;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .custom {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(140px, 100%), 1fr));
    gap: 12px;
    align-items: end;
  }

  .aside {
    font-size: 13px;
    color: var(--faint);
  }
</style>
