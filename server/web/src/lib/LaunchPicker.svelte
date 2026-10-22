<!--
  Sélecteur de l'app ouverte au début des sessions d'un appareil : les
  suggestions détectées (Steam Big Picture, Playnite…), tout le menu Démarrer
  de l'ordinateur avec ses icônes, et un explorateur limité à quelques
  dossiers. Rien à taper : on clique. Les options (fermer à la fin, pas en
  Bureau) sont sur la carte de l'appareil, pas ici.
-->
<script lang="ts">
  import { api, type AppEntry, type Browse, type Catalogue, type Device, type Launch, type Suggestion } from './api'
  import { errorText, notify } from './store.svelte'
  import { t } from './i18n/index.svelte'
  import Icon from './Icon.svelte'

  let { device, onclose, onsaved }: { device: Device; onclose: () => void; onsaved: (devices: Device[]) => void } = $props()

  let catalogue = $state<Catalogue | null>(null)
  let loading = $state(true)
  let refreshing = $state(false)
  let saving = $state(false)
  let search = $state('')
  // Le réglage en cours de l'appareil, au moment où le sélecteur s'ouvre.
  // svelte-ignore state_referenced_locally
  let draft = $state<Launch | null>(device.launch ? { ...device.launch } : null)
  let view = $state<'pick' | 'browse'>('pick')
  let browse = $state<Browse | null>(null)
  let browsing = $state(false)

  $effect(() => {
    api
      .apps()
      .then((c) => (catalogue = c))
      .catch((e) => notify('error', errorText(e)))
      .finally(() => (loading = false))
  })

  async function refresh() {
    refreshing = true
    try {
      catalogue = await api.appsRefresh()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      refreshing = false
    }
  }

  const query = $derived(search.trim().toLowerCase())
  const apps = $derived((catalogue?.apps ?? []).filter((a) => !query || a.name.toLowerCase().includes(query)))
  const suggestions = $derived((catalogue?.suggestions ?? []).filter((s) => !query || s.name.toLowerCase().includes(query)))

  function isSelected(kind: string, target: string): boolean {
    return draft?.kind === kind && draft?.target === target
  }

  function keepOptions(canClose: boolean, defaultClose: boolean) {
    return {
      close_at_end: draft ? draft.close_at_end && canClose : defaultClose,
      skip_desktop: draft?.skip_desktop ?? false,
    }
  }

  function pickApp(a: AppEntry) {
    draft = { kind: 'app', target: a.id, name: a.name, icon: a.icon ?? null, exe: a.exe ?? null, close: null, ...keepOptions(true, false) }
  }

  function pickSuggestion(s: Suggestion) {
    draft = { kind: s.kind, target: s.target, name: s.name, icon: s.icon ?? null, exe: s.exe ?? null, close: s.close ?? null, ...keepOptions(true, !!s.close) }
  }

  function pickFile(path: string, name: string) {
    const clean = name.replace(/\.(exe|lnk)$/i, '')
    draft = { kind: 'path', target: path, name: clean, icon: null, exe: /\.exe$/i.test(name) ? path : null, close: null, ...keepOptions(true, false) }
    view = 'pick'
  }

  /** L'app choisie sait-elle se fermer proprement ? (Même règle que le serveur.) */
  const canClose = $derived(!!draft && (!!draft.close || !!draft.exe || draft.kind !== 'uri'))

  async function open(path?: string) {
    browsing = true
    try {
      browse = await api.appsBrowse(path)
      view = 'browse'
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      browsing = false
    }
  }

  function rootLabel(id: string): string {
    const known: Record<string, string> = {
      programs: t('devices.launch.roots.programs'),
      programs86: t('devices.launch.roots.programs86'),
      desktop: t('devices.launch.roots.desktop'),
      userPrograms: t('devices.launch.roots.userPrograms'),
    }
    return known[id] ?? id
  }

  async function save(launch: Launch | null) {
    saving = true
    try {
      const res = await api.setLaunch(device.id, launch)
      onsaved(res.devices)
      notify('ok', launch ? t('devices.launch.saved', { app: launch.name, name: device.name }) : t('devices.launch.cleared', { name: device.name }))
      onclose()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      saving = false
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="launch-title">
    <header>
      <div>
        <span class="lbl">{t('devices.launch.eyebrow')}</span>
        <h2 id="launch-title" class="disp">{t('devices.launch.title', { name: device.name })}</h2>
        <p>{t('devices.launch.intro')}</p>
      </div>
      <button type="button" class="icon-button" aria-label={t('devices.launch.cancel')} onclick={onclose}><Icon name="x" size={16} /></button>
    </header>

    {#if view === 'browse'}
      <section class="browse">
        <div class="browse-head">
          {#if browse?.path}
            <button type="button" class="ghost small" disabled={browsing} onclick={() => open(browse?.parent ?? undefined)}><Icon name="back" size={14} />{t('devices.launch.up')}</button>
            <span class="mono path">{browse.path}</span>
          {:else}
            <button type="button" class="ghost small" onclick={() => (view = 'pick')}><Icon name="back" size={14} />{t('devices.launch.backToList')}</button>
            <span class="path">{t('devices.launch.otherHint')}</span>
          {/if}
        </div>
        <ul class="list">
          {#if browse?.roots}
            {#each browse.roots as r (r.id)}
              <li><button type="button" onclick={() => open(r.path)}><Icon name="folder" size={16} /><span>{rootLabel(r.id)}</span><span class="mono dim">{r.path}</span></button></li>
            {/each}
          {:else}
            {#each browse?.folders ?? [] as f (f.path)}
              <li><button type="button" onclick={() => open(f.path)}><Icon name="folder" size={16} /><span>{f.name}</span></button></li>
            {/each}
            {#each browse?.files ?? [] as f (f.path)}
              <li><button type="button" class="file" class:selected={isSelected('path', f.path)} onclick={() => pickFile(f.path, f.name)}><Icon name="app" size={16} /><span>{f.name}</span></button></li>
            {/each}
            {#if !(browse?.folders?.length || browse?.files?.length)}
              <li class="dim">{t('devices.launch.emptyFolder')}</li>
            {/if}
          {/if}
        </ul>
      </section>
    {:else}
      <div class="tools">
        <label class="search">
          <Icon name="search" size={16} />
          <input type="search" placeholder={t('devices.launch.search')} bind:value={search} autocomplete="off" />
        </label>
        <button type="button" class="ghost small" disabled={refreshing} onclick={refresh}><Icon name="restart" size={14} />{refreshing ? t('devices.launch.refreshing') : t('devices.launch.refresh')}</button>
        <button type="button" class="ghost small" disabled={browsing} onclick={() => open()}><Icon name="folder" size={14} />{t('devices.launch.other')}</button>
      </div>

      {#if loading}
        <p class="dim">{t('devices.launch.loading')}</p>
      {:else}
        {#if catalogue?.error}
          <div class="notice warn" role="status"><Icon name="warning" /><span>{t('devices.launch.readFailed', { error: catalogue.error })}</span></div>
        {/if}
        {#if suggestions.length}
          <h3 class="lbl">{t('devices.launch.suggestions')}</h3>
          <div class="chips">
            {#each suggestions as s (s.id)}
              <button type="button" class="chip" class:selected={isSelected(s.kind, s.target)} onclick={() => pickSuggestion(s)}>
                {#if s.icon}<img src={api.appIconUrl(s.icon)} alt="" width="22" height="22" />{:else}<Icon name="play" size={18} />{/if}
                <span>{s.name}</span>
              </button>
            {/each}
          </div>
        {/if}
        <h3 class="lbl">{catalogue?.user ? t('devices.launch.allOf', { user: catalogue.user }) : t('devices.launch.all')}</h3>
        {#if apps.length}
          <div class="grid">
            {#each apps as a (a.id)}
              <button type="button" class="tile" class:selected={isSelected('app', a.id)} title={a.name} onclick={() => pickApp(a)}>
                {#if a.icon}<img src={api.appIconUrl(a.icon)} alt="" width="40" height="40" />{:else}<span class="blank"><Icon name="app" size={22} /></span>{/if}
                <span class="name">{a.name}</span>
              </button>
            {/each}
          </div>
        {:else}
          <p class="dim">{catalogue?.apps.length ? t('devices.launch.noMatch') : t('devices.launch.emptyCatalogue')}</p>
        {/if}
      {/if}
    {/if}

    <footer>
      <div class="chosen">
        {#if draft}
          {#if draft.icon}<img src={api.appIconUrl(draft.icon)} alt="" width="28" height="28" />{:else}<Icon name="app" size={20} />{/if}
          <div>
            <strong>{draft.name}</strong>
            <span class="mono dim">{draft.target}</span>
            <span class="dim">{canClose ? t('devices.launch.closeHint') : t('devices.launch.noCloseHint')} {t('devices.launch.optionsHint')}</span>
          </div>
        {:else}
          <Icon name="other" size={20} />
          <div><strong>{t('devices.launch.none')}</strong><span class="dim">{t('devices.launch.noneHint')}</span></div>
        {/if}
      </div>
      <div class="actions">
        {#if device.launch}
          <button type="button" class="ghost danger" disabled={saving} onclick={() => save(null)}>{t('devices.launch.clear')}</button>
        {/if}
        <span class="spacer"></span>
        <button type="button" class="ghost" disabled={saving} onclick={onclose}>{t('devices.launch.cancel')}</button>
        <button type="button" class="primary" disabled={saving || !draft} onclick={() => draft && save(draft)}><Icon name="check" size={15} stroke={2} />{t('devices.launch.save')}</button>
      </div>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(5 7 10 / 0.6);
    backdrop-filter: blur(10px);
  }

  .dialog {
    width: min(760px, 100%);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    overflow: auto;
    border-radius: var(--r-card);
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2), 0 30px 80px rgb(0 0 0 / 0.6);
    animation: rise 0.35s var(--ease-out) both;
  }

  header {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    padding: 24px 24px 14px;
  }

  header > div {
    flex: 1;
    min-width: 0;
  }

  header h2 {
    margin-top: 6px;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  header p {
    margin-top: 8px;
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--text-2);
  }

  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    padding: 0 24px 14px;
  }

  .search {
    flex: 1 1 220px;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-radius: var(--r-control);
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line-2);
    color: var(--faint);
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    color: var(--text);
    font-size: 14px;
    outline: none;
  }

  h3.lbl {
    padding: 8px 24px 8px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 0 24px 10px;
  }

  .chip,
  .tile,
  .list button {
    border: 0;
    background: var(--ink-2);
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--line);
    transition: background-color 0.15s, box-shadow 0.15s;
    text-align: left;
  }

  .chip:hover,
  .tile:hover,
  .list button:hover {
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--line-3);
  }

  .selected,
  .chip.selected,
  .tile.selected,
  .list button.selected {
    background: rgb(109 91 255 / 0.18);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.65), 0 0 14px rgb(109 91 255 / 0.35);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 14px 0 10px;
    border-radius: 999px;
    font-size: 13.5px;
    font-weight: 600;
  }

  .chip img {
    border-radius: 5px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
    gap: 8px;
    padding: 0 24px 8px;
    overflow: auto;
    max-height: 42vh;
  }

  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 14px 8px 12px;
    border-radius: 12px;
  }

  .tile img,
  .blank {
    width: 40px;
    height: 40px;
    border-radius: 9px;
  }

  .blank {
    display: grid;
    place-items: center;
    background: var(--ink-3);
    color: var(--faint);
  }

  .tile .name {
    width: 100%;
    font-size: 12.5px;
    line-height: 1.3;
    text-align: center;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .browse {
    padding: 0 24px 8px;
  }

  .browse-head {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    margin-bottom: 10px;
  }

  .path {
    font-size: 12px;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 42vh;
    overflow: auto;
  }

  .list button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 0 12px;
    border-radius: 9px;
    font-size: 13.5px;
  }

  .list button span:first-of-type {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .list button .dim {
    font-size: 11px;
  }

  .list li.dim {
    padding: 10px 12px;
    font-size: 13px;
  }

  .dim {
    color: var(--faint);
  }

  p.dim {
    padding: 4px 24px 12px;
    font-size: 13.5px;
  }

  .notice {
    margin: 0 24px 12px;
  }

  footer {
    margin-top: auto;
    padding: 14px 24px 20px;
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .chosen {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .chosen img {
    border-radius: 7px;
  }

  .chosen > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .chosen strong {
    font-size: 14.5px;
  }

  .chosen span {
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }


  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
  }

  .spacer {
    flex: 1;
  }

  @media (max-width: 760px) {
    .overlay {
      padding: 0;
    }

    .dialog {
      width: 100%;
      max-height: 100vh;
      border-radius: 0;
    }

    /* Sur un téléphone, c'est le sélecteur entier qui défile, pied compris. */
    .grid,
    .list {
      max-height: none;
      overflow: visible;
    }
  }
</style>
