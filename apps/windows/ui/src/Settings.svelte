<!--
  Réglages d'un ordinateur : profils (ajuster un réglage rend le profil
  « Personnalisé »), image, entrées et son, et la carte « Ce PC » (ce que la
  carte graphique décode, vérifié au lancement).
-->
<script lang="ts">
  import Segmented from './lib/Segmented.svelte'
  import Switch from './lib/Switch.svelte'
  import Icon from './lib/Icon.svelte'
  import { untrack } from 'svelte'
  import type { Computer, Settings, ThisPc } from './lib/api'
  import { invoke, isCustom } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let { computer, thisPc, onremoved }: { computer: Computer; thisPc: ThisPc | null; onremoved: () => void } = $props()

  // Copie de travail, enregistrée à chaque changement (l'écran est recréé
  // pour un autre ordinateur).
  let s = $state<Settings>(untrack(() => structuredClone($state.snapshot(computer.settings)) as Settings))
  let confirmRemove = $state(false)
  let name = $state(untrack(() => computer.name))

  // Enregistrements à la suite : le dernier état gagne toujours.
  let saving: Promise<unknown> = Promise.resolve()
  function save(patch: Partial<Settings>) {
    s = { ...s, ...patch }
    const settings = $state.snapshot(s)
    saving = saving.then(() => invoke('save_settings', { id: computer.id, settings })).catch(() => {})
  }

  const custom = $derived(isCustom(s))
  const profiles: { value: Settings['profile']; title: string; sub: string }[] = $derived(
    (['jeu', 'bureau', 'max'] as const).map((value) => ({ value, title: t(`profiles.${value}`), sub: t(`settings.${value}_sub`) })),
  )

  /** Valeurs du profil choisi, comme le cœur les calcule (profile.rs). */
  const base = $derived.by(() => {
    const hevc = thisPc?.hevc ?? true
    const codec: 'h264' | 'hevc' = hevc ? 'hevc' : 'h264'
    let [yuv444, mbps, reliable] = { jeu: [false, 60, false], bureau: [true, 90, true], max: [true, 300, false] }[s.profile] as [boolean, number, boolean]
    if (yuv444 && !(codec === 'hevc' && thisPc?.hevc444)) {
      yuv444 = false
      mbps += Math.floor(mbps / 3)
    }
    if (codec === 'h264') mbps += Math.floor(mbps / 2)
    return { codec, yuv444, mbps, reliable }
  })

  const codec = $derived(s.codec ?? base.codec)
  const yuv444 = $derived(s.yuv444 ?? base.yuv444)
  const reliable = $derived(s.reliable ?? base.reliable)

  /** Débit réel d'une session à l'écran de ce PC. */
  const realMbps = $derived.by(() => {
    if (s.mbps != null) return s.mbps
    if (!thisPc && !s.custom) return base.mbps
    const mode = s.custom ?? thisPc!
    const hz = s.battery === 'alleger' && thisPc?.on_battery ? Math.min(30, mode.refresh) : mode.refresh
    // Même ajustement de l'écran que plan.rs (au plus 4K).
    let { width: w, height: h } = mode
    if (!s.custom && s.max_4k && (w > 3840 || h > 2160)) {
      const k = Math.min(3840 / w, 2160 / h)
      w = Math.floor(w * k)
      h = Math.floor(h * k)
    }
    const ratio = (w * h * hz) / (3840 * 2160 * 60)
    const k = Math.min(3, Math.max(0.4, Math.pow(ratio, 0.75)))
    return Math.round(base.mbps * k)
  })

  function pickProfile(p: Settings['profile']) {
    save({ profile: p, codec: null, yuv444: null, mbps: null, reliable: null })
  }

  // Un réglage égal à celui du profil n'en est pas un écart.
  const over = <K extends 'codec' | 'yuv444' | 'reliable'>(k: K, v: Settings[K], b: Settings[K]) => save({ [k]: v === b ? null : v } as Partial<Settings>)

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

  async function remove() {
    await invoke('remove_computer', { id: computer.id })
    onremoved()
  }

  const decodes = $derived(
    thisPc
      ? [
          { k: 'H.264', v: thisPc.h264 },
          { k: 'HEVC', v: thisPc.hevc },
          { k: 'HEVC 4:4:4', v: thisPc.hevc444 },
          { k: 'AV1', v: thisPc.av1 },
        ]
      : [],
  )
</script>

<main>
  <div class="head">
    <div class="title">
      <div class="eyebrow">{t('settings.settings_of')}</div>
      <input class="name" aria-label={t('settings.computer_name')} bind:value={name} onblur={rename} onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()} />
      <div class="mono addr">{computer.host}:{computer.port} · {t('settings.account', { user: computer.user })}</div>
    </div>
    <div class="mono real">{t('settings.real_bitrate', { mbps: realMbps })}</div>
  </div>

  <div class="profiles" role="radiogroup" aria-label={t('home.profile')}>
    {#each profiles as p (p.value)}
      {@const on = !custom && s.profile === p.value}
      <button role="radio" aria-checked={on} class:on onclick={() => pickProfile(p.value)}>
        <span class="t">{p.title}</span><span class="s">{p.sub}</span>
      </button>
    {/each}
    <div class="custom" class:on={custom}>
      <span class="t">{t('profiles.custom')}</span><span class="s">{custom ? t('settings.custom_based', { profile: t(`profiles.${s.profile}`) }) : t('settings.custom_when')}</span>
    </div>
  </div>

  <div class="cols">
    <section>
      <h2 class="eyebrow">{t('settings.image')}</h2>
      <div class="line stack">
        <div class="lr">
          <span>{t('settings.computer_display')}</span>
          <Segmented
            label={t('settings.computer_display')}
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
            <label><span class="eyebrow">Hz</span><input class="field mono" type="number" min="24" max="240" value={c.refresh} onchange={(e) => setCustom({ refresh: num(e) })} /></label>
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
        {/if}
      </div>
      <div class="line">
        <span>{t('settings.format')}</span>
        <Segmented
          label={t('settings.format')}
          value={codec}
          options={[
            { value: 'h264', label: 'H.264' },
            ...(thisPc?.hevc !== false ? [{ value: 'hevc' as const, label: 'HEVC' }] : []),
            ...(thisPc?.av1 ? [{ value: 'av1' as const, label: 'AV1' }] : []),
          ]}
          onchange={(v) => over('codec', v, base.codec)}
        />
      </div>
      <div class="line">
        <span>{t('settings.full_color')}</span>
        <Switch label={t('settings.full_color_short')} checked={yuv444 && codec === 'hevc'} disabled={codec !== 'hevc' || !thisPc?.hevc444} onchange={(v) => over('yuv444', v, base.yuv444)} />
      </div>
      <div class="line stack">
        <div class="lr">
          <span>{t('settings.display_mode')}</span>
          <Segmented label={t('settings.display_short')} value={s.vsync ? 'sync' : 'now'} options={[{ value: 'sync', label: t('settings.synced') }, { value: 'now', label: t('settings.immediate') }]} onchange={(v) => save({ vsync: v === 'sync' })} />
        </div>
        <span class="help">{t('settings.display_help')}</span>
      </div>
      <div class="line stack">
        <div class="lr">
          <span>{t('settings.bitrate')}</span>
          <span class="mono val">{sliderMbps} Mbit/s {s.mbps == null ? t('settings.profile_suffix') : ''}</span>
        </div>
        <input
          type="range"
          min="5"
          max="500"
          step="5"
          value={sliderMbps}
          aria-label={t('settings.bitrate')}
          oninput={(e) => (mbpsDraft = Number((e.currentTarget as HTMLInputElement).value))}
          onchange={() => {
            if (mbpsDraft != null) save({ mbps: mbpsDraft })
            mbpsDraft = null
          }}
        />
        {#if s.mbps != null}<button class="reset" onclick={() => save({ mbps: null })}>{t('settings.reset_bitrate')}</button>{/if}
        {#if sliderMbps > 300}<span class="help warn">{t('settings.wifi_warn')}</span>{/if}
      </div>
    </section>

    <section>
      <h2 class="eyebrow">{t('settings.inputs')}</h2>
      <div class="line">
        <span>{t('settings.mouse')}</span>
        <Segmented label={t('settings.mouse')} value={s.mouse} options={[{ value: 'auto', label: 'Auto' }, { value: 'bureau', label: t('settings.mouse_desktop') }, { value: 'jeu', label: t('settings.mouse_game') }]} onchange={(v) => save({ mouse: v })} />
      </div>
      <div class="line">
        <span>{t('settings.gamepad')}</span>
        <Segmented label={t('settings.gamepad_short')} value={s.gamepad} options={[{ value: 'xbox360', label: 'Xbox 360' }, { value: 'fidele', label: t('settings.faithful') }]} onchange={(v) => save({ gamepad: v })} />
      </div>
      <div class="line">
        <span>{t('settings.clipboard')}</span>
        <Switch label={t('settings.clipboard')} checked={s.clipboard} onchange={(v) => save({ clipboard: v })} />
      </div>
      <div class="line">
        <span>{t('settings.sound')}</span>
        <Segmented
          label={t('settings.sound')}
          value={s.audio}
          options={[
            { value: 'auto', label: 'Auto' },
            { value: '2', label: t('settings.stereo') },
            { value: '6', label: '5.1' },
            { value: '8', label: '7.1' },
            { value: '0', label: t('settings.none') },
          ]}
          onchange={(v) => save({ audio: v })}
        />
      </div>
      <div class="line">
        <span>{t('settings.transport')}</span>
        <Segmented label={t('settings.transport')} value={reliable ? 'fiable' : 'rapide'} options={[{ value: 'rapide', label: t('settings.fast') }, { value: 'fiable', label: t('settings.reliable') }]} onchange={(v) => over('reliable', v === 'fiable', base.reliable)} />
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
          <span>{t('settings.keep_last')}</span>
          <Switch label={t('settings.keep_last')} checked={s.keep_last_image} onchange={(v) => save({ keep_last_image: v })} />
        </div>
        <span class="help">{t('settings.keep_help')}</span>
      </div>
    </section>

    <section class="pc">
      <h2 class="eyebrow">{t('settings.this_pc')}</h2>
      {#if thisPc}
        <div class="pcname">{thisPc.name}</div>
        <div class="mono gpu">{thisPc.gpu || t('settings.unknown_gpu')}</div>
        <div class="table mono">
          {#each decodes as d (d.k)}
            <div class="tr"><span>{d.k}</span><span class:ok={d.v} class:no={!d.v}>{d.v ? t('settings.hardware') : d.k === 'AV1' ? t('settings.no') : t('settings.processor')}</span></div>
          {/each}
          <div class="tr"><span>{t('settings.display')}</span><span>{thisPc.width} × {thisPc.height} · {thisPc.refresh} Hz</span></div>
          <div class="tr"><span>{t('settings.sound')}</span><span>{thisPc.audio_channels === 8 ? '7.1' : thisPc.audio_channels === 6 ? '5.1' : t('settings.stereo_lower')}</span></div>
        </div>
        <p class="help">{t('settings.probe_help')}</p>
        <div class="line stack battery">
          <div class="lr">
            <span><Icon name="battery" size={16} /> {t('settings.battery')}</span>
            <Segmented label={t('settings.battery')} value={s.battery} options={[{ value: 'alleger', label: t('settings.lighten') }, { value: 'garder', label: t('settings.keep') }]} onchange={(v) => save({ battery: v })} />
          </div>
          <span class="help">{t('settings.battery_help')}{thisPc.on_battery ? t('settings.on_battery_now') : ''}</span>
        </div>
      {:else}
        <p class="help">{t('settings.probing')}</p>
      {/if}
    </section>
  </div>

  <div class="foot">
    {#if confirmRemove}
      <span>{t('settings.remove_confirm', { name: computer.name })}</span>
      <button class="danger" onclick={remove}>{t('settings.remove')}</button>
      <button class="ghost" onclick={() => (confirmRemove = false)}>{t('common.cancel')}</button>
    {:else}
      <button class="ghost" onclick={() => (confirmRemove = true)}><Icon name="trash" size={16} /> {t('settings.remove_computer')}</button>
    {/if}
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
    gap: 22px;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 20px;
  }

  .title {
    display: flex;
    flex-direction: column;
    gap: 6px;
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

  .addr,
  .real {
    font-size: 12px;
    color: var(--muted);
  }

  .profiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
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
    font-size: 16px;
    font-weight: 600;
  }

  .s {
    font-size: 12px;
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

  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 340px;
    gap: 18px;
    align-items: start;
  }

  section {
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--ink-1);
    padding: 6px 20px 12px;
  }

  h2 {
    margin: 14px 0 6px;
    font-weight: 500;
  }

  /* Libellé et choix sur une ligne quand il y a la place, sinon le choix
     passe dessous (fenêtre étroite). */
  .line {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 8px 12px;
    padding: 12px 0;
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

  .lr span:first-child {
    display: flex;
    align-items: center;
    gap: 8px;
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

  .custom-mode {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) minmax(0, 0.7fr);
    align-items: end;
    gap: 8px;
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

  .pc {
    background: var(--ink-2);
    padding: 6px 20px 16px;
  }

  .pcname {
    font-size: 18px;
    font-weight: 600;
    margin-top: 4px;
  }

  .gpu {
    font-size: 11px;
    color: var(--muted);
    margin: 4px 0 10px;
    line-height: 1.6;
  }

  .table {
    font-size: 12px;
  }

  .tr {
    display: flex;
    justify-content: space-between;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
  }

  .tr span:last-child {
    color: var(--text);
  }

  .tr span.ok {
    color: var(--ok);
  }

  .tr span.no {
    color: var(--faint);
  }

  .battery {
    margin-top: 6px;
    border-top: 1px solid var(--line);
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
    color: var(--muted);
  }

  .foot .ghost {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .danger {
    height: 40px;
    padding: 0 16px;
    border: 0;
    border-radius: 8px;
    background: #c42b1c;
    color: #fff;
    font-weight: 600;
  }

  @media (max-width: 1280px) {
    .cols {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }

    .pc {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 1040px) {
    main {
      padding: 20px 32px 32px;
    }

    .cols {
      grid-template-columns: minmax(0, 1fr);
    }

    .profiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
