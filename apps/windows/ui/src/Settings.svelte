<!--
  Réglages d'un ordinateur (docs/link-ui.md) : son nom, le profil (ajuster un
  réglage le rend « Personnalisé »), puis six rubriques choisies dans la
  colonne de gauche : Image, Son, Commandes, Réseau, Statistiques, Station.
  Fenêtre étroite (moins de 1040 px) : les rubriques passent en rangée
  au-dessus de leur contenu.
-->
<script lang="ts">
  import Segmented from './lib/Segmented.svelte'
  import Switch from './lib/Switch.svelte'
  import Icon from './lib/Icon.svelte'
  import Statistics from './Statistics.svelte'
  import Station from './Station.svelte'
  import { onMount, untrack } from 'svelte'
  import type { BarPen, Battery, Computer, DisplaysInfo, MicSettings, PenMode, Settings, ThisPc } from './lib/api'
  import { chosenBy, invoke, isMacStation, listen, pyrowaveOffered, values } from './lib/api'
  import { beforeName, t } from './lib/i18n/index.svelte'

  let {
    computer,
    thisPc,
    battery,
    start = null,
    accountName = null,
    applied = null,
    inSession = false,
    onprofile,
    onautotest,
    onremoved,
  }: {
    computer: Computer
    thisPc: ThisPc | null
    battery: Battery
    /** Rubrique à ouvrir (notification, menu de l'icône) ; sinon Image. */
    start?: string | null
    /** Un ordinateur d'un compte KyberNow : le compte qui le donne (rubrique Station). */
    accountName?: string | null
    /** Profil Auto appliqué par le panneau du test : les réglages enregistrés, repris ici. */
    applied?: Settings | null
    /** L'ordinateur est en session : Auto choisi sans test, le panneau s'ouvrira à sa fin. */
    inSession?: boolean
    /** Auto choisi sans test gardé (comme l'accueil : `set_profile`). */
    onprofile: (id: string, p: Settings['profile']) => void
    /** « Refaire le test » : le panneau du test du profil Auto. */
    onautotest: (id: string) => void
    onremoved: () => void
  } = $props()

  // Copie de travail, enregistrée à chaque changement (l'écran est recréé
  // pour un autre ordinateur).
  let s = $state<Settings>(untrack(() => structuredClone($state.snapshot(computer.settings)) as Settings))
  let name = $state(untrack(() => computer.name))
  // Profil Auto appliqué (panneau du test) : ses réglages remplacent la copie de travail.
  $effect(() => {
    if (applied) s = structuredClone($state.snapshot(applied)) as Settings
  })
  /** Le test gardé pour cet ordinateur (profil Auto). */
  const auto = $derived(computer.auto ?? null)

  type Rubric = 'image' | 'sound' | 'controls' | 'network' | 'stats' | 'station'
  const rubrics: { id: Rubric; icon: string }[] = [
    { id: 'image', icon: 'monitor' },
    { id: 'sound', icon: 'speaker' },
    { id: 'controls', icon: 'gamepad' },
    { id: 'network', icon: 'wifi' },
    { id: 'stats', icon: 'chart' },
    { id: 'station', icon: 'station' },
  ]
  const known = (r: string | null): Rubric | null => rubrics.find((x) => x.id === r)?.id ?? null
  let rubric = $state<Rubric>(untrack(() => known(start) ?? 'image'))
  // Demandée de nouveau alors que l'écran est déjà ouvert.
  $effect(() => {
    const r = known(start)
    if (r) untrack(() => (rubric = r))
  })

  /** Ce PC a un micro : le réglage « Micro » de la rubrique Son (docs/link-ui.md « Micro »). */
  let hasMic = $state(false)
  onMount(() => {
    invoke<MicSettings>('mic_settings')
      .then((m) => (hasMic = m.devices.length > 0))
      .catch(() => (hasMic = false))
  })
  /** « Stylet » (docs/link-ui.md « Stylet ») : ses textes, ceux du cœur (`pen_texts`) ; le réglage est toujours montré. */
  let penTexts = $state<BarPen | null>(null)
  onMount(() => {
    invoke<BarPen>('pen_texts')
      .then((p) => (penTexts = p))
      .catch(() => (penTexts = null))
  })
  // « Stylet » changé dans la barre ou par Ctrl+Alt+Maj+P : la copie de travail le reprend.
  $effect(() => {
    const mode: PenMode = computer.settings.pen === 'mouse' ? 'mouse' : 'pressure'
    untrack(() => {
      if ((s.pen === 'mouse' ? 'mouse' : 'pressure') !== mode) s = { ...s, pen: mode }
    })
  })
  // « Micro » changé dans la barre ou par Ctrl+Alt+Maj+V : la copie de travail le reprend.
  $effect(() => {
    const on = !!computer.settings.microphone
    untrack(() => {
      if (!!s.microphone !== on) s = { ...s, microphone: on }
    })
  })

  // Enregistrements à la suite : le dernier état gagne toujours.
  let saving: Promise<unknown> = Promise.resolve()
  /**
   * Un autre format : en quittant PyroWave, un débit réglé à la main passe à celui de HEVC au plus (règle du cœur,
   * `values.hevc_after_pyrowave_mbps`, comme « Passer en HEVC ») ; un débit de PyroWave ne vaut rien ailleurs.
   */
  function codecChange(codec: Settings['codec']): Partial<Settings> {
    const leaving = s.codec === 'pyrowave' && codec !== 'pyrowave' && s.mbps != null
    return leaving ? { codec, mbps: Math.min(s.mbps!, values.hevc_after_pyrowave_mbps) } : { codec }
  }

  function save(patch: Partial<Settings>) {
    s = { ...s, ...patch }
    const settings = $state.snapshot(s)
    // Refusé (ordinateur retiré entre-temps…) : noté, la suite part quand même.
    saving = saving.then(() => invoke('save_settings', { id: computer.id, settings })).catch((e) => console.error('save_settings :', e))
  }

  // Le réglage « Écrans », « Personnalisé », les valeurs du profil et le débit de la session : ce que le cœur
  // décide (`displays_info`, le même calcul que la session). Revu à chaque réglage changé, et quand les écrans de
  // ce PC changent.
  let displaysInfo = $state<DisplaysInfo | null>(null)
  const custom = $derived(displaysInfo?.plan.custom ?? false)
  /** Valeurs du profil choisi sur ce PC (le cœur ; Auto : celles du test) ; `null` le temps de la première réponse. */
  const base = $derived(displaysInfo?.plan.base ?? null)
  /** Une Station Mac : « Ctrl agit comme ⌘ » et l'aide du clavier face à un Mac. */
  const mac = $derived(isMacStation(computer.info?.os || computer.os))
  const profiles = $derived((['jeu', 'bureau', 'max', 'auto'] as const).map((value) => ({ value, title: t(`profiles.${value}`), sub: t(`profiles.${value}_sub`) })))
  /** Le profil Auto est celui de l'ordinateur (sans retouche), avec son test. */
  const autoOn = $derived(s.profile === 'auto' && !custom && !!auto)


  const codec = $derived(s.codec ?? base?.codec ?? null)
  const yuv444 = $derived(s.yuv444 ?? base?.yuv444 ?? false)
  const reliable = $derived(s.reliable ?? base?.reliable ?? false)

  /**
   * PyroWave (docs/link-ui.md « PyroWave ») : la valeur de « Format vidéo » quand l'ordinateur le propose (ou
   * qu'il est déjà choisi), inactive avec sa raison ; « Auto » le prend seulement par câble. `wired` : ce PC
   * rejoint l'ordinateur par un câble Ethernet (demandé à l'ouverture des réglages).
   */
  const pyroOffered = $derived(pyrowaveOffered(computer.info))
  const pyroDecodes = $derived(!!thisPc?.pyrowave)
  const pyroShown = $derived(pyroOffered || s.codec === 'pyrowave')
  const pyroReason = $derived(
    !pyroOffered ? t('pyrowave.not_offered', { name: computer.name }) : !pyroDecodes ? t('pyrowave.cannot_decode', { device: t('pyrowave.this_device') }) : null,
  )
  let wired = $state<boolean | null>(null)
  // L'identifiant seul : `computer` est un objet neuf à chaque relevé.
  const computerId = $derived(computer.id)
  $effect(() => {
    const id = computerId
    invoke<boolean>('computer_wired', { id })
      .then((w) => (wired = w))
      .catch(() => (wired = null))
  })
  const relayed = $derived(!!(computer.via_relay || computer.away))
  /** L'aide sous « Format vidéo », et l'avertissement Wi-Fi ou relais quand PyroWave est choisi. */
  const formatHelp = $derived.by(() => {
    // Valeur inactive : sa raison.
    if (pyroShown && pyroReason) return pyroReason
    if (s.codec === 'pyrowave') return t('pyrowave.chosen_help')
    if (s.codec == null && pyroOffered && pyroDecodes) {
      const cable = wired && !relayed
      // PyroWave par câble : Max et Auto seulement (docs/link-ui.md « Profil Auto et débit adaptatif »).
      if (s.profile === 'jeu' || s.profile === 'bureau') return cable ? t('pyrowave.auto_profile_only') : t('pyrowave.auto_unwired')
      // Auto : PyroWave seulement si le test l'a choisi (son résumé dit pourquoi sinon).
      if (s.profile === 'auto' && auto?.codec !== 'pyrowave') return null
      return cable ? t('pyrowave.auto_wired') : t('pyrowave.auto_unwired')
    }
    return null
  })
  const formatWarn = $derived(
    s.codec !== 'pyrowave' || pyroReason ? null : relayed ? t('pyrowave.warn_relay') : wired === false ? t('pyrowave.warn_wifi') : null,
  )
  /** PyroWave : 4:4:4 possible (le décodeur le prend toujours). */
  const pyro444 = $derived(codec === 'pyrowave' && pyroDecodes)

  /** Débit réel d'une session : celui que le cœur demandera (`plan` de `displays_info`). */
  const realMbps = $derived(displaysInfo?.plan.mbps ?? s.mbps ?? 0)
  let screensEpoch = $state(0)
  let asked = 0
  // Ce que ce PC décode compte pour « Personnalisé » (règle du cœur). Une clé :
  // la demande ne repart que si un de ces éléments change vraiment.
  const displaysKey = $derived(JSON.stringify([computerId, $state.snapshot(s), screensEpoch, $state.snapshot(thisPc), battery]))
  $effect(() => {
    const [id, settings] = JSON.parse(displaysKey) as [string, unknown]
    const n = ++asked
    invoke<DisplaysInfo>('displays_info', { id, settings })
      .then((i) => {
        if (n === asked) displaysInfo = i
      })
      .catch(() => {})
  })
  onMount(() => {
    const off = listen<null>('displays', () => screensEpoch++)
    return () => off.then((f) => f())
  })
  /** Inactif avec une définition sur mesure : l'aide le dit. Sans écran virtuel
   *  utilisable (réglage coupé, ou Station qui n'en a pas), pas d'aide : la
   *  ligne grisée sur « Cet écran » se suffit (docs/link-ui.md, comme sur Mac et Linux). */
  const displaysCustom = $derived(!!s.custom)
  const displaysNoVirtual = $derived(displaysInfo ? !displaysInfo.virtual_display : false)
  /** HDR : la puce du cœur, et le HDR sera demandé (« Texte net (4:4:4) » inactif). */
  const hdr = $derived(displaysInfo?.hdr ?? null)
  const hdrOn = $derived(!!displaysInfo?.hdr_on)

  /**
   * Un profil : ses réglages, retouches effacées. Auto (docs/link-ui.md) : avec un test gardé, ses réglages tout de
   * suite (définition comprise, `autotest_apply`) ; sans test, le panneau du test (à la fin de la session si
   * l'ordinateur est en session).
   */
  async function pickProfile(p: Settings['profile']) {
    if (p !== 'auto') {
      save({ profile: p, codec: null, yuv444: null, mbps: null, reliable: null })
      return
    }
    if (!auto) {
      if (inSession) onprofile(computer.id, 'auto')
      else onautotest(computer.id)
      return
    }
    await saving
    try {
      s = await invoke<Settings>('autotest_apply', { id: computer.id, result: $state.snapshot(auto) })
    } catch {}
  }

  // Un réglage égal à celui du profil n'en est pas un écart.
  const over = <K extends 'yuv444' | 'reliable'>(k: K, v: Settings[K], b: Settings[K]) => save({ [k]: v === b ? null : v } as Partial<Settings>)

  let mbpsDraft = $state<number | null>(null)
  const sliderMbps = $derived(mbpsDraft ?? realMbps)

  const presets = [
    { label: '1080p', width: 1920, height: 1080 },
    { label: '1440p', width: 2560, height: 1440 },
    { label: '4K', width: 3840, height: 2160 },
    { label: '16:10', width: 1920, height: 1200 },
  ]
  const num = (e: Event) => Math.round(Number((e.currentTarget as HTMLInputElement).value))
  /** Taille paire, dans les limites de l'écran virtuel (7680 × 4320, 24 à 240 Hz). */
  function setCustom(patch: Partial<NonNullable<Settings['custom']>>) {
    const c = { ...(s.custom ?? { width: 1920, height: 1080, refresh: 60 }), ...patch }
    c.width = Math.min(7680, Math.max(640, c.width || 1920)) & ~1
    c.height = Math.min(4320, Math.max(360, c.height || 1080)) & ~1
    c.refresh = Math.min(240, Math.max(24, c.refresh || 60))
    save({ custom: c })
  }

  async function rename() {
    if (name.trim() && name.trim() !== computer.name) await invoke('rename_computer', { id: computer.id, name })
  }
</script>

<main>
  <div class="head">
    <div class="eyebrow">{beforeName(t('settings.settings_of'), name)}</div>
    <input class="name" aria-label={t('settings.computer_name')} bind:value={name} onblur={rename} onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()} />
  </div>

  <div class="eyebrow">{t('home.profile')}</div>
  <div class="profiles" role="radiogroup" aria-label={t('home.profile')}>
    {#each profiles as p (p.value)}
      {@const on = !custom && s.profile === p.value}
      <button role="radio" aria-checked={on} class:on onclick={() => pickProfile(p.value)}>
        <span class="t">{#if p.value === 'auto'}<Icon name="wand" size={15} stroke={1.8} />{/if}{p.title}</span><span class="s">{p.sub}</span>
      </button>
    {/each}
    <div class="custom" class:on={custom}>
      <span class="t">{t('profiles.custom')}</span><span class="s">{custom ? t('profiles.custom_sub') : t('settings.custom_when')}</span>
    </div>
  </div>

  <div class="body">
    <nav aria-label={t('settings.rubrics')}>
      {#each rubrics as r (r.id)}
        <button class:on={rubric === r.id} aria-current={rubric === r.id ? 'page' : undefined} onclick={() => (rubric = r.id)}>
          <Icon name={r.icon} size={18} stroke={1.6} />{t(`settings.rubric.${r.id}`)}
        </button>
      {/each}
    </nav>

    <div class="pane">
      <h2>{t(`settings.rubric.${rubric}`)}</h2>

      {#if rubric === 'image'}
        <section>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.resolution')}</span>
              <Segmented
                label={t('settings.resolution')}
                value={s.custom ? 'custom' : s.max_4k ? 'max4k' : 'match'}
                options={[
                  { value: 'match', label: t('settings.match') },
                  { value: 'max4k', label: t('settings.max4k') },
                  { value: 'custom', label: t('settings.custom') },
                ]}
                onchange={(v) =>
                  save(
                    v === 'custom'
                      ? { custom: { width: thisPc?.width ?? 1920, height: thisPc?.height ?? 1080, refresh: thisPc?.refresh ?? 60 } }
                      : { custom: null, max_4k: v === 'max4k' },
                  )}
              />
            </div>
            {#if s.custom}
              {@const c = s.custom}
              <div class="custom-mode">
                <label><span class="eyebrow">{t('settings.width')}</span><input class="field mono" type="number" min="640" max="7680" step="2" value={c.width} onchange={(e) => setCustom({ width: num(e) })} /></label>
                <span class="x">×</span>
                <label><span class="eyebrow">{t('settings.height')}</span><input class="field mono" type="number" min="360" max="4320" step="2" value={c.height} onchange={(e) => setCustom({ height: num(e) })} /></label>
                <label><span class="eyebrow">{t('settings.refresh')}</span><input class="field mono" type="number" min="24" max="240" value={c.refresh} onchange={(e) => setCustom({ refresh: num(e) })} /></label>
              </div>
              <div class="presets">
                {#each presets as p (p.label)}
                  <button class:on={c.width === p.width && c.height === p.height} onclick={() => setCustom({ width: p.width, height: p.height })}>{p.label}</button>
                {/each}
                {#each [60, 120, 144] as hz (hz)}
                  <button class:on={c.refresh === hz} onclick={() => setCustom({ refresh: hz })}>{hz} Hz</button>
                {/each}
              </div>
              <span class="help">{t('settings.custom_help')}</span>
            {:else}
              <span class="help">{t('settings.refresh_help')}</span>
            {/if}
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.displays')}</span>
              <Segmented
                label={t('settings.displays')}
                value={displaysInfo?.shown ?? s.displays}
                disabled={displaysInfo ? !displaysInfo.enabled : false}
                options={[
                  { value: 'auto', label: 'Auto' },
                  { value: 'this', label: t('settings.displays_this') },
                  { value: 'all', label: t('settings.displays_all') },
                ]}
                onchange={(v) => save({ displays: v })}
              />
            </div>
            {#if displaysCustom}
              <span class="help">{t('settings.displays_custom_help')}</span>
            {:else if !displaysNoVirtual}
              <span class="help">{t('settings.displays_help')}</span>
            {/if}
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.virtual')}</span>
              <Switch label={t('settings.virtual')} checked={s.virtual_display} onchange={(v) => save({ virtual_display: v })} />
            </div>
            <span class="help">{t('settings.virtual_help')}</span>
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.format')}</span>
              <Segmented
                label={t('settings.format')}
                value={s.codec ?? 'auto'}
                options={[
                  { value: 'auto' as const, label: 'Auto' },
                  ...(thisPc?.hevc !== false ? [{ value: 'hevc' as const, label: 'HEVC' }] : []),
                  { value: 'h264' as const, label: 'H.264' },
                  ...(thisPc?.av1 ? [{ value: 'av1' as const, label: 'AV1' }] : []),
                  // PyroWave sous son nom ; inactif (sa raison dans l'aide) s'il ne peut pas servir.
                  ...(pyroShown ? [{ value: 'pyrowave' as const, label: 'PyroWave', disabled: !!pyroReason && s.codec !== 'pyrowave' }] : []),
                ]}
                onchange={(v) => save(codecChange(v === 'auto' ? null : v))}
              />
            </div>
            {#if formatHelp}<span class="help">{formatHelp}</span>{/if}
            {#if formatWarn}<span class="help warn">{formatWarn}</span>{/if}
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.sharp_text')}</span>
              <Switch
                label={t('settings.sharp_text')}
                checked={yuv444 && ((codec === 'hevc' && !!thisPc?.hevc444) || pyro444) && !hdrOn}
                disabled={!((codec === 'hevc' && thisPc?.hevc444) || pyro444) || hdrOn}
                onchange={(v) => over('yuv444', v, base?.yuv444 ?? false)}
              />
            </div>
            <span class="help">{hdrOn ? t('hdr.no_444') : pyro444 ? t('pyrowave.sharp_text_help') : t('settings.sharp_text_help')}</span>
          </div>
          {#if hdr?.shown}
            <!-- HDR (docs/link-ui.md « HDR ») : sur un écran HDR seulement ; inactif avec la raison. -->
            <div class="line stack">
              <div class="lr">
                <span>HDR</span>
                <Switch label="HDR" checked={hdr.available && (s.hdr ?? true)} disabled={!hdr.available} onchange={(v) => save({ hdr: v })} />
              </div>
              <span class="help">{hdr.reason ?? t('hdr.settings_help')}</span>
            </div>
          {/if}
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.bitrate')}</span>
              <span class="mono val">{s.mbps == null && mbpsDraft == null ? t('settings.bitrate_auto', { mbps: sliderMbps }) : `${sliderMbps} Mbit/s`}</span>
            </div>
            <input
              type="range"
              min={values.manual_mbps.min}
              max={codec === 'pyrowave' ? values.manual_mbps.max_pyrowave : values.manual_mbps.max}
              step={values.manual_mbps.step}
              value={sliderMbps}
              aria-label={t('settings.bitrate')}
              oninput={(e) => (mbpsDraft = Number((e.currentTarget as HTMLInputElement).value))}
              onchange={() => {
                if (mbpsDraft != null) save({ mbps: mbpsDraft })
                mbpsDraft = null
              }}
            />
            {#if s.mbps != null}<button class="reset" onclick={() => save({ mbps: null })}>{t('settings.reset_bitrate')}</button>{/if}
            {#if sliderMbps > 300 && codec !== 'pyrowave'}<span class="help warn">{t('settings.wifi_warn')}</span>{/if}
            {#if autoOn && auto}
              <!-- Profil Auto : le test qui a choisi ces réglages, et « Refaire le test ». -->
              <span class="help chosen">{chosenBy(auto)} <button class="again" onclick={() => onautotest(computer.id)}>{t('autotest.again')}</button></span>
            {/if}
            <!-- « Débit adaptatif » (docs/link-ui.md) : hors du profil, il ne rend pas « Personnalisé ». -->
            <div class="lr adaptive">
              <span>{t('autotest.adaptive')}</span>
              <Switch label={t('autotest.adaptive')} checked={s.adaptive ?? true} onchange={(v) => save({ adaptive: v })} />
            </div>
            <span class="help">{t('autotest.adaptive_help')}{codec === 'pyrowave' ? ` ${t('autotest.adaptive_pyrowave')}` : ''}</span>
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.frame_display')}</span>
              <Segmented label={t('settings.frame_display')} value={s.vsync ? 'sync' : 'now'} options={[{ value: 'sync', label: t('settings.synced') }, { value: 'now', label: t('settings.immediate') }]} onchange={(v) => save({ vsync: v === 'sync' })} />
            </div>
            <span class="help">{t('settings.display_help')}</span>
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.keep_last')}</span>
              <Switch label={t('settings.keep_last')} checked={s.keep_last_image} onchange={(v) => save({ keep_last_image: v })} />
            </div>
            <span class="help">{t('settings.keep_help')}</span>
          </div>
        </section>
      {:else if rubric === 'sound'}
        <section>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.sound')}</span>
              <Segmented
                label={t('settings.sound')}
                value={s.audio}
                options={[
                  { value: 'auto', label: 'Auto' },
                  { value: '2', label: t('settings.stereo') },
                  { value: '6', label: '5.1' },
                  { value: '8', label: '7.1' },
                  { value: '0', label: t('settings.on_computer') },
                ]}
                onchange={(v) => save({ audio: v })}
              />
            </div>
            <span class="help">{t('settings.sound_help')}</span>
          </div>
          {#if hasMic}
            <!-- Le même interrupteur que « Micro » de la session, retenu pour cet ordinateur. -->
            <div class="line stack">
              <div class="lr">
                <span>{t('micro.setting')}</span>
                <Switch label={t('micro.setting')} checked={!!s.microphone} onchange={(v) => save({ microphone: v })} />
              </div>
              <span class="help">{t('micro.setting_help')}</span>
            </div>
          {/if}
        </section>
      {:else if rubric === 'controls'}
        <section>
          <div class="line">
            <span>{t('settings.gamepad')}</span>
            <Segmented label={t('settings.gamepad')} value={s.gamepad} options={[{ value: 'xbox360', label: 'Xbox 360' }, { value: 'fidele', label: t('settings.as_is') }]} onchange={(v) => save({ gamepad: v })} />
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.mouse')}</span>
              <Segmented label={t('settings.mouse')} value={s.mouse} options={[{ value: 'auto', label: 'Auto' }, { value: 'bureau', label: t('settings.mouse_desktop') }, { value: 'jeu', label: t('settings.mouse_game') }]} onchange={(v) => save({ mouse: v })} />
            </div>
            <span class="help">{t('settings.mouse_help')}</span>
          </div>
          {#if penTexts}
            <!-- « Stylet : Pression / Souris », retenu pour cet ordinateur ; montré même sans stylet branché. -->
            <div class="line stack">
              <div class="lr">
                <span>{penTexts.title}</span>
                <Segmented
                  label={penTexts.title}
                  value={s.pen === 'mouse' ? 'mouse' : 'pressure'}
                  options={penTexts.choices.map((c) => ({ value: c.key, label: c.label }))}
                  onchange={(v) => save({ pen: v === 'mouse' ? 'mouse' : 'pressure' })}
                />
              </div>
              <span class="help">{penTexts.help}</span>
            </div>
          {/if}
          <div class="line stack">
            <span>{t('settings.keyboard')}</span>
            <span class="help">{t(mac ? 'settings.keyboard_help_mac' : 'settings.keyboard_help')}</span>
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.immersive')}</span>
              <Segmented label={t('settings.immersive')} value={s.immersive ?? 'fullscreen'} options={[{ value: 'fullscreen', label: t('settings.immersive_fullscreen') }, { value: 'always', label: t('settings.immersive_always') }, { value: 'never', label: t('settings.immersive_never') }]} onchange={(v) => save({ immersive: v })} />
            </div>
            <span class="help">{t('settings.immersive_help')}</span>
          </div>
          {#if mac}
            <!-- Face à une Station Mac seulement : allumé tant qu'on ne l'a pas éteint. -->
            <div class="line stack">
              <div class="lr">
                <span>{t('settings.ctrl_command')}</span>
                <Switch label={t('settings.ctrl_command')} checked={s.ctrl_command ?? true} onchange={(v) => save({ ctrl_command: v })} />
              </div>
              <span class="help">{t('settings.ctrl_command_help')}</span>
            </div>
          {/if}
          <div class="line">
            <span>{t('settings.clipboard')}</span>
            <Switch label={t('settings.clipboard')} checked={s.clipboard} onchange={(v) => save({ clipboard: v })} />
          </div>
          {#if computer.camera_offered}
            <!-- La caméra de ce PC (docs/webcam.md) : une fois que l'ordinateur l'a proposée. -->
            <div class="line stack">
              <div class="lr">
                <span>{t('camera.resolution')}</span>
                <Segmented label={t('camera.resolution')} value={s.camera_1080 ? '1080p' : '720p'} options={[{ value: '720p', label: '720p' }, { value: '1080p', label: '1080p' }]} onchange={(v) => save({ camera_1080: v === '1080p' })} />
              </div>
              <span class="help">{t('camera.resolution_help')}</span>
            </div>
          {/if}
        </section>
      {:else if rubric === 'network'}
        <section>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.transport')}</span>
              <Segmented label={t('settings.transport')} value={reliable ? 'fiable' : 'rapide'} options={[{ value: 'rapide', label: t('settings.fast') }, { value: 'fiable', label: t('settings.reliable') }]} onchange={(v) => over('reliable', v === 'fiable', base?.reliable ?? false)} />
            </div>
            <span class="help">{t('settings.transport_help')}</span>
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.reconnect')}</span>
              <Switch label={t('settings.reconnect')} checked={s.auto_reconnect} onchange={(v) => save({ auto_reconnect: v })} />
            </div>
            <span class="help">{t('settings.reconnect_help')}</span>
          </div>
          <div class="line stack">
            <div class="lr">
              <span>{t('settings.wake')}</span>
              {#if computer.wake?.length}
                <span class="wake"><span class="ready">{t('settings.wake_ready')}</span><span class="mono">{computer.wake[0].mac}</span></span>
              {:else}
                <span class="wake none">{t('settings.wake_none')}</span>
              {/if}
            </div>
            <span class="help">{t('settings.wake_help')}</span>
          </div>
        </section>
      {:else if rubric === 'stats'}
        <Statistics {computer} />
      {:else}
        <Station {computer} {accountName} {onremoved} />
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
    margin-bottom: 8px;
  }

  .name {
    border: 0;
    background: none;
    padding: 0;
    font-family: var(--display);
    font-size: 40px;
    font-weight: 640;
    font-stretch: 125%;
    width: 640px;
    max-width: 100%;
    user-select: text;
  }

  .name:focus {
    outline: none;
    box-shadow: 0 2px 0 var(--signal);
  }

  .profiles {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 12px;
    margin-bottom: 14px;
  }

  .profiles button,
  .custom {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 16px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--ink-1);
    text-align: left;
  }

  .custom {
    border-style: dashed;
    border-color: var(--line-2);
    background: none;
  }

  .t {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 16px;
    font-weight: 600;
  }

  .s {
    font-size: 12px;
    line-height: 1.4;
    color: var(--faint);
  }

  .profiles .on {
    border-color: var(--signal);
    border-style: solid;
    background: var(--ink-3);
    box-shadow: 0 0 14px rgb(109 91 255 / 0.22);
  }

  .on .s {
    color: var(--muted);
  }

  /* Rubriques à gauche, leur contenu à droite (comme les réglages de Windows). */
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

  section {
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-1);
    padding: 4px 22px 6px;
  }

  /* Libellé et choix sur une ligne quand il y a la place, sinon le choix
     passe dessous (fenêtre étroite). */
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

  .warn {
    color: var(--warn);
  }

  .val {
    font-size: 12px;
    color: var(--muted);
  }

  .wake {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 13px;
  }

  .wake .ready {
    color: var(--ok);
  }

  .wake .mono {
    font-size: 12px;
    color: var(--muted);
  }

  .wake.none {
    color: var(--muted);
  }

  .custom-mode {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) minmax(0, 0.7fr);
    align-items: end;
    gap: 8px;
    max-width: 520px;
  }

  .custom-mode label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .custom-mode .field {
    min-width: 0;
    height: 38px;
    font-size: 13px;
  }

  .x {
    padding-bottom: 10px;
    color: var(--faint);
  }

  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .presets button {
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--line-2);
    border-radius: 7px;
    background: none;
    color: var(--muted);
    font-size: 12px;
  }

  .presets button.on {
    border-color: var(--signal);
    color: var(--text);
    background: var(--signal-soft);
  }

  input[type='range'] {
    width: 100%;
    accent-color: var(--signal-strong);
  }

  .reset {
    align-self: flex-start;
    border: 0;
    background: none;
    padding: 0;
    font-size: 12px;
    color: var(--signal);
  }

  /* Profil Auto : « Choisi par le test du … » et « Refaire le test », puis l'interrupteur « Débit adaptatif ». */
  .again {
    border: 0;
    background: none;
    padding: 0;
    font-size: inherit;
    color: var(--signal);
  }

  .again:hover {
    text-decoration: underline;
  }

  .adaptive {
    margin-top: 10px;
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

    .profiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
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
