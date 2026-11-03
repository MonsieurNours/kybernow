<!--
  Réglages d'une console PlayStation (docs/link-ui.md « Consoles PlayStation », « Réglages de la
  console ») : présentés comme ceux d'un ordinateur (Settings.svelte), sans profils. En tête son
  nom, son modèle et « Auto » ou « Personnalisé » ; à gauche trois rubriques :
  - Image : définition, images par seconde, débit (Auto, ou de 2 à 100 Mbit/s), format (inactif sur
    une PS4, qui n'envoie que du H.264), « Ouvrir la partie en plein écran ». Pas d'agrandissement
    (appareils Apple seulement) ni de HDR (pas sur Windows) : gardés tels quels ;
  - Commandes : la touche de chaque action de la manette (un clic : « Appuyez sur une touche ou
    cliquez… », Échap annule, Effacement retire la touche ; une touche déjà prise ailleurs y est
    retirée), la souris qui tient le stick droit, « Disposition par défaut » ;
  - Manette : vibrations et gâchettes adaptatives de la DualSense, de 0 à 100 % par pas de 10.
  Chaque changement est gardé tout de suite (`ps_save_settings`, `ps_save_layout`) et vaut pour la
  partie suivante ; le clavier, la souris et les intensités changent aussi pendant une partie.
  En bas, « Retirer cette console », confirmé.
-->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import Segmented from './lib/Segmented.svelte'
  import Switch from './lib/Switch.svelte'
  import Icon from './lib/Icon.svelte'
  import type { PsConsole, PsInput, PsLayout, PsSettings, PsSettingsView } from './lib/api'
  import { codeToUsage, invoke, ms, psActions, psAutoBitrate, psBind, psCurves, psInputsOf, psIsCustom, psMouseButton, psResolutionLabel, psSameInput, psUnbind } from './lib/api'
  import { beforeName, t } from './lib/i18n/index.svelte'

  let {
    console: target,
    onremoved,
  }: {
    console: PsConsole
    onremoved: () => void
  } = $props()

  const id = untrack(() => target.id)
  let view = $state<PsSettingsView | null>(null)
  let error = $state<string | null>(null)
  invoke<PsSettingsView>('ps_settings', { id })
    .then((v) => (view = v))
    .catch((e) => (error = String(e)))

  type Rubric = 'picture' | 'controls' | 'controller'
  const rubrics: { id: Rubric; icon: string }[] = [
    { id: 'picture', icon: 'monitor' },
    { id: 'controls', icon: 'keyboard' },
    { id: 'controller', icon: 'gamepad' },
  ]
  let rubric = $state<Rubric>('picture')

  const s = $derived(view?.settings ?? null)
  const l = $derived(view?.layout ?? null)
  /** La PS4 n'envoie que du H.264. */
  const ps4 = $derived(target.model === 'PS4')
  const custom = $derived(s ? psIsCustom(s) : target.custom)

  // Enregistrements à la suite : le dernier état gagne toujours.
  let saving: Promise<unknown> = Promise.resolve()
  function save(patch: Partial<PsSettings>) {
    if (!view) return
    view.settings = { ...view.settings, ...patch }
    const settings = $state.snapshot(view.settings)
    saving = saving.then(() => invoke('ps_save_settings', { id, settings })).catch((e) => (error = String(e)))
  }
  function saveLayout(layout: PsLayout) {
    if (!view) return
    view.layout = layout
    const snapshot = $state.snapshot(layout)
    saving = saving.then(() => invoke('ps_save_layout', { id, layout: snapshot })).catch((e) => (error = String(e)))
  }

  async function defaultLayout() {
    stopCatch()
    try {
      const layout = await invoke<PsLayout>('ps_default_layout', { id })
      if (view) view.layout = layout
    } catch (e) {
      error = String(e)
    }
  }

  // Curseurs : la valeur suit le doigt, l'enregistrement se fait au lâcher.
  let draft = $state<{ key: string; value: number } | null>(null)
  const shown = (key: string, value: number) => (draft?.key === key ? draft.value : value)
  const num = (e: Event) => Number((e.currentTarget as HTMLInputElement).value)

  const mbps = $derived(s ? Math.round((s.bitrate_kbps ?? psAutoBitrate(s.resolution)) / 1000) : 15)
  /** La courbe la plus proche de celle gardée (Vive par défaut). */
  const curve = $derived(l ? psCurves.reduce((best, c, i) => (Math.abs(c - l.curve) < Math.abs(psCurves[best] - l.curve) ? i : best), 2) : 2)

  // MARK: - Touches

  /** Le nom d'une touche (celui de Windows, dans la disposition du clavier de ce PC) ou d'un bouton de la souris. */
  function inputName(i: PsInput) {
    if ('mouse' in i) return i.mouse === 0 ? t('ps.click_left') : i.mouse === 1 ? t('ps.click_right') : i.mouse === 2 ? t('ps.click_middle') : t('ps.mouse_button', { n: i.mouse + 1 })
    return view?.key_names[String(i.key)] ?? t('ps.key_n', { n: i.key })
  }

  /** L'action dont on attend la touche, et la touche qu'on change (nulle : une de plus). */
  let waitingFor = $state<{ action: string; replacing: PsInput | null } | null>(null)
  const isWaiting = (action: string, replacing: PsInput | null) =>
    !!waitingFor && waitingFor.action === action && (waitingFor.replacing && replacing ? psSameInput(waitingFor.replacing, replacing) : waitingFor.replacing === replacing)

  function listenFor(action: string, replacing: PsInput | null) {
    stopCatch()
    waitingFor = { action, replacing }
    // Après ce clic : le suivant est la touche cherchée.
    setTimeout(() => {
      if (!waitingFor) return
      window.addEventListener('keydown', onKey, true)
      window.addEventListener('mousedown', onMouse, true)
      window.addEventListener('contextmenu', swallow, true)
      window.addEventListener('auxclick', swallow, true)
    })
  }

  function stopCatch() {
    waitingFor = null
    window.removeEventListener('keydown', onKey, true)
    window.removeEventListener('mousedown', onMouse, true)
    // Le clic droit ou du milieu qui finit la saisie ne doit pas ouvrir de menu.
    setTimeout(() => {
      window.removeEventListener('contextmenu', swallow, true)
      window.removeEventListener('auxclick', swallow, true)
    }, 300)
  }
  onDestroy(stopCatch)

  function swallow(e: Event) {
    e.preventDefault()
    e.stopPropagation()
  }

  function caught(input: PsInput | 'remove' | 'cancel') {
    const w = waitingFor
    stopCatch()
    if (!w || !l || input === 'cancel') return
    if (input === 'remove') {
      if (w.replacing) saveLayout(psUnbind(l, w.replacing, w.action))
      return
    }
    saveLayout(psBind(l, input, w.action, w.replacing))
  }

  function onKey(e: KeyboardEvent) {
    swallow(e)
    if (e.repeat) return
    // Échap annule, Effacement retire la touche qu'on changeait.
    if (e.code === 'Escape') return caught('cancel')
    if (e.code === 'Backspace') return caught('remove')
    const usage = codeToUsage[e.code]
    if (usage != null) caught({ key: usage })
  }

  function onMouse(e: MouseEvent) {
    swallow(e)
    const b = psMouseButton(e.button)
    if (b != null) caught({ mouse: b })
  }

  // MARK: - Retrait

  let confirmRemove = $state(false)
  async function remove() {
    try {
      await invoke('ps_remove', { id })
      onremoved()
    } catch (e) {
      error = String(e)
    }
  }
</script>

<main>
  <div class="head">
    <div class="eyebrow">{beforeName(t('settings.settings_of'), target.name)}</div>
    <div class="title">
      <h1>{target.name}</h1>
      <span class="eyebrow">{target.model}</span>
      <span class="chip"><Icon name="sliders" size={16} stroke={1.6} />{custom ? t('ps.custom') : t('ps.auto')}</span>
    </div>
  </div>

  <div class="body">
    <nav aria-label={t('settings.rubrics')}>
      {#each rubrics as r (r.id)}
        <button class:on={rubric === r.id} aria-current={rubric === r.id ? 'page' : undefined} onclick={() => ((rubric = r.id), stopCatch())}>
          <Icon name={r.icon} size={18} stroke={1.6} />{t(`ps.${r.id}`)}
        </button>
      {/each}
    </nav>

    <div class="pane">
      <h2>{t(`ps.${rubric}`)}</h2>
      {#if error}<div class="error" role="alert">{error}</div>{/if}

      {#if s && l}
        {#if rubric === 'picture'}
          <section>
            <div class="line stack">
              <div class="lr">
                <span>{t('ps.resolution')}</span>
                <Segmented
                  label={t('ps.resolution')}
                  value={s.resolution}
                  options={(['auto', 'p1080', 'p720', 'p540', 'p360'] as const).map((r) => ({ value: r, label: psResolutionLabel(r) }))}
                  onchange={(v) => save({ resolution: v })}
                />
              </div>
              {#if s.resolution === 'auto'}<span class="help">{t('ps.resolution_auto_help')}</span>{/if}
            </div>
            <div class="line">
              <span>{t('ps.frame_rate')}</span>
              <Segmented
                label={t('ps.frame_rate')}
                value={s.fps === 30 ? '30' : '60'}
                options={[
                  { value: '60', label: t('ps.fps', { n: 60 }) },
                  { value: '30', label: t('ps.fps', { n: 30 }) },
                ]}
                onchange={(v) => save({ fps: Number(v) })}
              />
            </div>
            <div class="line stack">
              <div class="lr">
                <span>{t('ps.format')}</span>
                <Segmented
                  label={t('ps.format')}
                  value={ps4 ? 'h264' : s.codec}
                  disabled={ps4}
                  options={[
                    { value: 'auto' as const, label: t('ps.auto') },
                    { value: 'h264' as const, label: 'H.264' },
                    { value: 'h265' as const, label: 'H.265' },
                  ]}
                  onchange={(v) => save({ codec: v })}
                />
              </div>
              {#if ps4}
                <span class="help">{t('ps.format_ps4_help')}</span>
              {:else if s.codec === 'auto'}
                <span class="help">{t('ps.format_auto_help')}</span>
              {:else if s.codec === 'h264'}
                <span class="help">{t('ps.format_h264_help')}</span>
              {/if}
            </div>
            <div class="line stack">
              <div class="lr">
                <span>{t('ps.bitrate')}</span>
                <span class="auto">
                  <span class="mono val">{s.bitrate_kbps == null ? t('ps.auto') : t('ps.mbps', { n: shown('mbps', mbps) })}</span>
                  <Switch label={t('ps.bitrate') + ' · ' + t('ps.auto')} checked={s.bitrate_kbps == null} onchange={(on) => save({ bitrate_kbps: on ? null : psAutoBitrate(s.resolution) })} />
                </span>
              </div>
              {#if s.bitrate_kbps == null}
                <span class="help">{t('ps.bitrate_auto_help')}</span>
              {:else}
                <input
                  type="range"
                  min="2"
                  max="100"
                  step="1"
                  value={shown('mbps', mbps)}
                  aria-label={t('ps.bitrate')}
                  aria-valuetext={t('ps.mbps', { n: shown('mbps', mbps) })}
                  oninput={(e) => (draft = { key: 'mbps', value: num(e) })}
                  onchange={(e) => ((draft = null), save({ bitrate_kbps: Math.min(100, Math.max(2, num(e))) * 1000 }))}
                />
                <div class="ends mono"><span>2</span><span>100</span></div>
              {/if}
            </div>
            <div class="line">
              <span>{t('ps.fullscreen')}</span>
              <Switch label={t('ps.fullscreen')} checked={s.fullscreen} onchange={(v) => save({ fullscreen: v })} />
            </div>
          </section>
        {:else if rubric === 'controls'}
          <p class="help lead">{t('ps.controls_help')}</p>
          <section>
            {#each psActions as action (action)}
              {@const inputs = psInputsOf(l, action)}
              <div class="line keyrow">
                <span class="action">{t(`ps.action.${action}`)}</span>
                <span class="keys">
                  {#if inputs.length === 0 && !isWaiting(action, null)}<span class="none">{t('ps.none')}</span>{/if}
                  {#each inputs as input, i (i)}
                    {#if isWaiting(action, input)}
                      <span class="key waiting" role="status">{t('ps.press_key')}</span>
                    {:else}
                      <button class="key" onclick={() => listenFor(action, input)}>{inputName(input)}</button>
                    {/if}
                  {/each}
                  {#if isWaiting(action, null)}
                    <span class="key waiting" role="status">{t('ps.press_key')}</span>
                  {:else}
                    <button class="key plus" aria-label={t('ps.add_key', { action: t(`ps.action.${action}`) })} title={t('ps.add_key', { action: t(`ps.action.${action}`) })} onclick={() => listenFor(action, null)}>
                      <Icon name="plus" size={13} stroke={2} />
                    </button>
                  {/if}
                </span>
              </div>
            {/each}
          </section>

          <h3>{t('ps.mouse')}</h3>
          <section>
            <div class="line">
              <span>{t('ps.mouse_stick')}</span>
              <Switch label={t('ps.mouse_stick')} checked={l.mouse_stick} onchange={(v) => saveLayout({ ...l, mouse_stick: v })} />
            </div>
            <div class="line stack" class:inert={!l.mouse_stick}>
              <div class="lr">
                <span>{t('ps.sensitivity')}</span>
                <span class="mono val">{ms(shown('sens', l.sensitivity), 1)}</span>
              </div>
              <input
                type="range"
                min="0.2"
                max="5"
                step="0.1"
                value={shown('sens', l.sensitivity)}
                disabled={!l.mouse_stick}
                aria-label={t('ps.sensitivity')}
                oninput={(e) => (draft = { key: 'sens', value: num(e) })}
                onchange={(e) => ((draft = null), saveLayout({ ...l, sensitivity: Math.round(num(e) * 10) / 10 }))}
              />
            </div>
            <div class="line" class:inert={!l.mouse_stick}>
              <span>{t('ps.starting_response')}</span>
              <Segmented
                label={t('ps.starting_response')}
                value={String(curve)}
                disabled={!l.mouse_stick}
                options={['soft', 'normal', 'brisk'].map((k, i) => ({ value: String(i), label: t(`ps.curve_${k}`) }))}
                onchange={(v) => saveLayout({ ...l, curve: psCurves[Number(v)] })}
              />
            </div>
            <div class="line stack" class:inert={!l.mouse_stick}>
              <div class="lr">
                <span>{t('ps.minimum_tilt')}</span>
                <span class="mono val">{t('ps.percent', { n: Math.round(shown('dz', l.deadzone) * 100) })}</span>
              </div>
              <input
                type="range"
                min="0"
                max="0.5"
                step="0.01"
                value={shown('dz', l.deadzone)}
                disabled={!l.mouse_stick}
                aria-label={t('ps.minimum_tilt')}
                oninput={(e) => (draft = { key: 'dz', value: num(e) })}
                onchange={(e) => ((draft = null), saveLayout({ ...l, deadzone: Math.round(num(e) * 100) / 100 }))}
              />
              <span class="help">{t('ps.minimum_tilt_help')}</span>
            </div>
            <div class="line" class:inert={!l.mouse_stick}>
              <span>{t('ps.invert_y')}</span>
              <Switch label={t('ps.invert_y')} checked={l.invert_y} disabled={!l.mouse_stick} onchange={(v) => saveLayout({ ...l, invert_y: v })} />
            </div>
          </section>
          <div><button class="ghost reset" onclick={defaultLayout}><Icon name="restart" size={15} stroke={1.8} />{t('ps.default_layout')}</button></div>
        {:else}
          <section>
            {#each [['rumble', s.rumble], ['triggers', s.triggers]] as [key, value] (key)}
              <div class="line stack">
                <div class="lr">
                  <span>{t(`ps.${key}`)}</span>
                  <span class="mono val">{t('ps.percent', { n: shown(String(key), Number(value)) })}</span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="100"
                  step="10"
                  value={shown(String(key), Number(value))}
                  aria-label={t(`ps.${key}`)}
                  oninput={(e) => (draft = { key: String(key), value: num(e) })}
                  onchange={(e) => ((draft = null), save({ [String(key)]: Math.round(num(e) / 10) * 10 }))}
                />
              </div>
            {/each}
            <div class="line"><span class="help">{t('ps.controller_help')}</span></div>
          </section>
        {/if}

        <section class="remove">
          <div class="line">
            {#if confirmRemove}
              <span>{t('ps.remove_console_confirm', { name: target.name })}</span>
              <span class="acts">
                <button class="danger" onclick={remove}>{t('ps.remove')}</button>
                <button class="ghost" onclick={() => (confirmRemove = false)}>{t('common.cancel')}</button>
              </span>
            {:else}
              <button class="ghost" onclick={() => (confirmRemove = true)}><Icon name="trash" size={16} /> {t('ps.remove_console')}</button>
            {/if}
          </div>
        </section>
      {/if}
    </div>
  </div>
</main>

<style>
  main {
    position: absolute;
    inset: 40px 0 0 0;
    overflow-y: auto;
    padding: 20px 56px 40px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 22px;
  }

  .title {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 18px;
  }

  h1 {
    margin: 0;
    font-family: var(--display);
    font-size: 40px;
    font-weight: 640;
    font-stretch: 125%;
    overflow-wrap: anywhere;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 14px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: var(--ink-1);
    font-size: 14px;
    font-weight: 600;
    color: var(--muted);
  }

  .chip :global(svg) {
    color: var(--signal);
  }

  /* Rubriques à gauche, leur contenu à droite (comme les réglages d'un ordinateur). */
  .body {
    display: grid;
    grid-template-columns: 220px minmax(0, 820px);
    gap: 32px;
    align-items: start;
  }

  nav {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  nav button {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 14px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--muted);
    font-size: 14px;
    text-align: left;
  }

  nav button:hover {
    background: rgb(255 255 255 / 0.04);
    color: var(--text);
  }

  nav button.on {
    background: var(--ink-3);
    box-shadow: inset 3px 0 0 var(--signal);
    color: var(--text);
    font-weight: 600;
  }

  nav button.on :global(svg) {
    color: var(--signal);
  }

  .pane {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  h2 {
    margin: 6px 0 0;
    font-size: 22px;
    font-weight: 600;
  }

  h3 {
    margin: 8px 0 0;
    font-size: 16px;
    font-weight: 600;
  }

  section {
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-1);
    padding: 4px 22px 6px;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 8px 12px;
    padding: 14px 0;
    border-bottom: 1px solid var(--ink-3);
    font-size: 14px;
  }

  .line:last-child {
    border-bottom: 0;
  }

  .line.stack {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .line.inert {
    opacity: 0.45;
  }

  .lr {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 8px 12px;
  }

  .help {
    font-size: 12px;
    line-height: 1.45;
    color: var(--faint);
  }

  .help.lead {
    margin: 0;
    font-size: 13px;
  }

  .val {
    font-size: 12px;
    color: var(--muted);
  }

  .auto {
    display: inline-flex;
    align-items: center;
    gap: 12px;
  }

  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: var(--faint);
  }

  input[type='range'] {
    width: 100%;
    accent-color: var(--signal-strong);
  }

  /* Une action de la manette : son nom, ses touches, « + ». */
  .keyrow {
    padding: 8px 0;
    flex-wrap: nowrap;
  }

  .action {
    color: var(--muted);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .keys {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }

  .none {
    font-size: 12.5px;
    color: var(--faint);
  }

  .key {
    min-width: 28px;
    height: 28px;
    padding: 0 10px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--line-2);
    border-radius: 7px;
    background: var(--ink-2);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
  }

  button.key:hover {
    border-color: var(--faint);
  }

  .key.plus {
    padding: 0;
    width: 28px;
    background: none;
    color: var(--muted);
  }

  .key.waiting {
    border-color: rgb(154 140 255 / 0.6);
    background: var(--signal-soft);
    color: var(--signal);
  }

  .ghost.reset {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .remove {
    margin-top: 10px;
  }

  .acts {
    display: flex;
    gap: 8px;
  }

  .remove .ghost {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .danger {
    height: 40px;
    padding: 0 16px;
    border: 0;
    border-radius: 8px;
    background: var(--bad);
    color: #1a0705;
    font-size: 14px;
    font-weight: 600;
  }

  @media (max-width: 1280px) {
    .body {
      grid-template-columns: 200px minmax(0, 1fr);
      gap: 24px;
    }
  }

  /* Fenêtre étroite : une colonne, les rubriques en rangée. */
  @media (max-width: 1040px) {
    main {
      padding: 20px 32px 32px;
    }

    .body {
      grid-template-columns: minmax(0, 1fr);
      gap: 18px;
    }

    nav {
      position: static;
      flex-direction: row;
      flex-wrap: wrap;
      gap: 6px;
    }

    nav button {
      height: 38px;
      padding: 0 12px;
      gap: 8px;
      border: 1px solid var(--line);
    }

    nav button.on {
      border-color: var(--signal);
      box-shadow: none;
    }

    h2 {
      display: none;
    }
  }
</style>
