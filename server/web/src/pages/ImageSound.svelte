<!--
  Image et son : ce que les appareils reçoivent pendant une session (écran
  virtuel, son) et ce qui part d'eux (clavier, souris, manettes). Chaque
  choix s'applique tout de suite.
-->
<script lang="ts">
  import { api, type AudioChannels, type AudioStatus, type DisplayStatus, type ModePolicy, type Trigger } from '../lib/api'
  import { app, errorText, field, kyber, loadKyber, notify, setValues, value } from '../lib/store.svelte'
  import { mode } from '../lib/format'
  import Icon from '../lib/Icon.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'
  import { t } from '../lib/i18n/index.svelte'

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

  const TRIGGERS: { value: Trigger; label: string }[] = $derived([
    { value: 'apps', label: t('imageSound.triggers.apps') },
    { value: 'all', label: t('imageSound.triggers.all') },
    { value: 'off', label: t('imageSound.triggers.off') },
  ])

  // Un jeu qui change la définition de l'écran virtuel pendant la session.
  const MODE_POLICIES: { value: ModePolicy; label: string }[] = $derived([
    { value: 'follow', label: t('imageSound.modePolicy.follow') },
    { value: 'restore', label: t('imageSound.modePolicy.restore') },
  ])

  const CHANNELS: { value: AudioChannels; label: string; hint: string }[] = $derived([
    { value: 'auto', label: t('imageSound.channels.auto'), hint: t('imageSound.channels.autoHint') },
    { value: 'stereo', label: t('imageSound.channels.stereo'), hint: t('imageSound.channels.stereoHint') },
    { value: '5.1', label: '5.1', hint: t('imageSound.channels.surround51Hint') },
    { value: '7.1', label: '7.1', hint: t('imageSound.channels.surround71Hint') },
  ])

  const INPUTS = ['input.clipboard_allow_copy', 'input.clipboard_allow_paste', 'input.gamepad_force_xbox360', 'input.keyboard_key_repeat']
  // Depuis KyberNow 0.2.6, les apps Mac et Apple TV choisissent par ordinateur
  // la manette vue par l'ordinateur (Xbox 360 ou fidèle) et l'imposent à la
  // session : l'option de la console n'est plus qu'un défaut pour les autres clients.
  const INPUT_NOTES: Record<string, string> = $derived({
    'input.gamepad_force_xbox360': t('imageSound.inputNotes.gamepad'),
  })

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
    setDisplay(PRESETS[Number(raw)], t('imageSound.toast.defaultMode'))
  }

  async function saveCustom(event: SubmitEvent) {
    event.preventDefault()
    if (!custom) return
    await setDisplay(custom, t('imageSound.toast.defaultMode'))
    custom = null
  }

  async function tryDisplay() {
    if (!display) return
    const r = await run(() => api.displayTest({ width: display!.settings.width, height: display!.settings.height, refresh: display!.settings.refresh }, 15))
    if (r) display = r
  }

  async function stopDisplay() {
    const r = await run(() => api.displayStop(), t('imageSound.toast.displaysBack'))
    if (r) display = r
  }

  const displayPill = $derived.by(() => {
    if (!display) return null
    if (!display.supported) return { kind: 'warn', text: t('imageSound.pill.simulated') }
    if (display.session) return { kind: 'signal', text: t(display.session.origin === 'test' ? 'imageSound.pill.test' : 'imageSound.pill.active', { mode: mode(display.session.mode) }) }
    if (!display.driver.available) return { kind: 'warn', text: t('imageSound.pill.driverUnavailable') }
    return { kind: 'ok', text: display.driver.info ? t('imageSound.pill.readyVersion', { version: display.driver.info.version }) : t('imageSound.pill.ready') }
  })

  const audioPill = $derived.by(() => {
    if (!audio) return null
    if (!audio.supported) return { kind: 'warn', text: t('imageSound.pill.simulated') }
    if (audio.session) return { kind: 'signal', text: t('imageSound.pill.sent', { channels: audio.session.channels === 8 ? '7.1' : audio.session.channels === 6 ? '5.1' : t('imageSound.pill.stereo') }) }
    if (!audio.devices.speakers) return { kind: 'warn', text: t('imageSound.pill.noSpeakers') }
    return { kind: 'ok', text: t('imageSound.pill.readySound') }
  })

  const quality = $derived(field('audio.quality'))
  const qualityValue = $derived(value<string>('audio.quality'))
  const qualityHint = $derived(quality?.kind.type === 'enum' ? (quality.kind.options.find((o) => o.value === qualityValue)?.hint ?? '') : '')
  const watchdog = $derived(display?.driver.info?.watchdog_seconds)
</script>

<header class="space-head with-tabs">
  <div>
    <span class="lbl">{t('imageSound.kicker')}</span>
    <h1>{t('imageSound.title')}</h1>
    <p>{t('imageSound.intro')}</p>
    <nav class="tabs" aria-label={t('imageSound.sections')}>
      <a href="#/image-son/ecran" class:current={app.section === 'ecran' || !app.section}>{t('imageSound.tabs.display')}</a>
      <a href="#/image-son/son" class:current={app.section === 'son'}>{t('imageSound.tabs.sound')}</a>
      <a href="#/image-son/commandes" class:current={app.section === 'commandes'}>{t('imageSound.tabs.inputs')}</a>
    </nav>
  </div>
</header>

<div class="space-body">
  <!-- Écran -->
  <section id="ecran" aria-labelledby="t-ecran" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-ecran">{t('imageSound.display.title')}</h2>
        <span class="sub">{t('imageSound.display.sub')}</span>
      </div>
      {#if displayPill}<span class="pill {displayPill.kind}"><Icon name={displayPill.kind === 'warn' ? 'warning' : 'check'} size={13} stroke={2.6} />{displayPill.text}</span>{/if}
    </div>

    <div class="steps">
      <div><span class="lbl">{t('imageSound.display.step1')}</span><span>{t('imageSound.display.step1Text')}</span></div>
      <div><span class="lbl">{t('imageSound.display.step2')}</span><span>{t('imageSound.display.step2Text')}</span></div>
      <div><span class="lbl">{t('imageSound.display.step3')}</span><span>{t('imageSound.display.step3Text')}</span></div>
    </div>

    {#if display?.last_error}
      <div class="notice warn" role="alert"><Icon name="warning" /><span>{display.last_error}</span></div>
    {/if}
    {#if display && !display.driver.available && display.driver.error}
      <div class="notice warn" role="alert"><Icon name="warning" /><span>{display.driver.error}</span></div>
    {/if}

    {#if display}
      <div class="set">
        <div><h3>{t('imageSound.display.trigger')}</h3><p>{t('imageSound.display.triggerHelp')}</p></div>
        <Segmented label={t('imageSound.display.trigger')} options={TRIGGERS} value={display.settings.trigger} disabled={busy} onchange={(v) => setDisplay({ trigger: v }, v === 'off' ? t('imageSound.toast.displayOff') : t('imageSound.toast.saved'))} />
      </div>
      <div class="set">
        <div><label class="title" for="mode">{t('imageSound.display.defaultMode')}</label><p>{t('imageSound.display.defaultModeHelp')}</p></div>
        <select id="mode" value={custom ? 'custom' : presetIndex >= 0 ? String(presetIndex) : 'custom'} disabled={busy} onchange={(e) => choosePreset(e.currentTarget.value)}>
          {#each PRESETS as p, i}<option value={String(i)}>{mode(p)}</option>{/each}
          <option value="custom">{presetIndex < 0 && !custom ? t('imageSound.display.otherCurrent', { mode: mode(display.settings) }) : t('imageSound.display.other')}</option>
        </select>
        {#if custom}
          <form class="custom" onsubmit={saveCustom}>
            <div class="field"><label for="w">{t('imageSound.display.width')}</label><input id="w" type="number" min="640" max="7680" bind:value={custom.width} /></div>
            <div class="field"><label for="h">{t('imageSound.display.height')}</label><input id="h" type="number" min="480" max="4320" bind:value={custom.height} /></div>
            <div class="field"><label for="r">{t('imageSound.display.refresh')}</label><input id="r" type="number" min="24" max="240" bind:value={custom.refresh} /></div>
            <button type="submit" class="ghost" disabled={busy}>{t('imageSound.display.save')}</button>
          </form>
        {/if}
      </div>
      <div class="set">
        <div>
          <h3>{t('imageSound.display.modeChange')}</h3>
          <p>{t('imageSound.display.modeChangeHelp')}</p>
          {#if display.session && display.mode && (display.mode.width !== display.session.mode.width || display.mode.height !== display.session.mode.height)}
            <p>{t('imageSound.display.modeNow', { current: mode(display.mode), requested: mode(display.session.mode) })}</p>
          {/if}
        </div>
        <Segmented label={t('imageSound.display.modeChange')} options={MODE_POLICIES} value={display.settings.on_mode_change ?? 'follow'} disabled={busy} onchange={(p) => setDisplay({ on_mode_change: p }, t('imageSound.toast.saved'))} />
      </div>
      <div class="set">
        <div>
          <h3>{t('imageSound.display.failsafe')}</h3>
          <p>{watchdog ? t('imageSound.display.failsafeHelpSeconds', { s: watchdog }) : t('imageSound.display.failsafeHelp')}</p>
        </div>
        <div class="buttons">
          {#if display.session?.origin === 'test'}
            <button type="button" class="ghost" disabled={busy} onclick={stopDisplay}>{display.session.seconds_left ? t('imageSound.display.stopTestSeconds', { s: display.session.seconds_left }) : t('imageSound.display.stopTest')}</button>
          {:else}
            <button type="button" class="ghost" disabled={busy || !!display.session || !display.driver.available} onclick={tryDisplay}>{t('imageSound.display.tryIt')}</button>
          {/if}
          <button type="button" class="ghost" disabled={busy || !display.session} onclick={stopDisplay}>{t('imageSound.display.restore')}</button>
        </div>
      </div>
    {/if}
  </section>

  <!-- Son -->
  <section id="son" aria-labelledby="t-son" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-son">{t('imageSound.sound.title')}</h2>
        <span class="sub">{t('imageSound.sound.sub')}</span>
      </div>
      {#if audioPill}<span class="pill {audioPill.kind}"><Icon name={audioPill.kind === 'warn' ? 'warning' : 'check'} size={13} stroke={2.6} />{audioPill.text}</span>{/if}
    </div>

    {#if audio && audio.supported && !audio.devices.speakers}
      <div class="notice warn"><Icon name="info" /><span>{t('imageSound.sound.installSteam')}</span></div>
    {/if}
    {#if audio?.last_error}
      <div class="notice warn" role="alert"><Icon name="warning" /><span>{audio.last_error}</span></div>
    {/if}

    {#if audio}
      <div class="set">
        <div><h3>{t('imageSound.sound.trigger')}</h3><p>{t('imageSound.sound.triggerHelp')}</p></div>
        <Segmented label={t('imageSound.sound.trigger')} options={TRIGGERS} value={audio.settings.trigger} disabled={busy} onchange={(v) => setAudio({ trigger: v }, t('imageSound.toast.saved'))} />
      </div>
      <div class="set">
        <div><h3>{t('imageSound.sound.channels')}</h3><p>{CHANNELS.find((c) => c.value === audio!.settings.channels)?.hint}</p></div>
        <Segmented label={t('imageSound.sound.channels')} options={CHANNELS} value={audio.settings.channels} disabled={busy} onchange={(c) => setAudio({ channels: c }, t('imageSound.toast.saved'))} />
      </div>
    {/if}
    {#if quality?.kind.type === 'enum'}
      <div class="set">
        <div><h3>{t('imageSound.sound.quality')}</h3><p>{qualityHint}</p></div>
        <Segmented label={t('imageSound.sound.qualityLabel')} options={quality.kind.options} value={qualityValue} onchange={(v) => setValues({ 'audio.quality': v })} />
      </div>
    {/if}
  </section>

  <!-- Commandes -->
  <section id="commandes" aria-labelledby="t-cmd" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-cmd">{t('imageSound.inputs.title')}</h2>
        <span class="sub">{t('imageSound.inputs.sub')}</span>
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

  <p class="aside">{t('imageSound.aside.before')}<a href="#/systeme/avance">{t('imageSound.aside.link')}</a>{t('imageSound.aside.after')}</p>
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
