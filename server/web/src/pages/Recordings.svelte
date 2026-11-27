<!--
  Enregistrements (docs/enregistrement.md, docs/link-ui.md « Enregistrement de la session ») :
  la session en cours (lancer, arrêter, la pastille et son temps), les réglages (permis, dossier,
  durée de garde) et les fichiers (lire dans la page, télécharger, supprimer).
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { Generation, poll } from '../lib/poll'
  import { api, type RecordingFile, type RecordingView } from '../lib/api'
  import { errorText, notify } from '../lib/store.svelte'
  import { elapsedNow, fileDate, fileDuration, fileSize, freeSpace, recordingClock } from '../lib/recording'
  import Icon from '../lib/Icon.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let view = $state<RecordingView | null>(null)
  /** Quand la vue est arrivée (ms de cette page) : le temps de la pastille en part. */
  let receivedAt = $state(Date.now())
  let now = $state(Date.now())
  let busy = $state(false)
  let editing = $state(false)
  let folder = $state('')
  /** Le fichier ouvert dans le lecteur. */
  let playing = $state<string | null>(null)

  const KEEP = $derived([
    { value: '7', label: t('recordings.days7') },
    { value: '30', label: t('recordings.days30') },
    { value: '90', label: t('recordings.days90') },
    { value: '0', label: t('recordings.forever') },
  ])

  const rec = $derived(view?.state ?? null)
  const active = $derived(!!rec?.active)
  const clock = $derived(recordingClock(elapsedNow(rec, receivedAt, now)))

  function show(v: RecordingView) {
    view = v
    receivedAt = Date.now()
    now = receivedAt
  }

  async function load() {
    try {
      show(await api.recording())
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  onMount(() => {
    load()
  })

  // Le temps chaque seconde pendant l'enregistrement.
  $effect(() => {
    if (!active) return
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  // La vue relue : toutes les 3 s pendant un enregistrement (taille du fichier, fin), sinon toutes les 10 s
  // (une session qui commence).
  // Un enregistrement des réglages rend périmé le relevé parti avant lui (WEB-04 : `busy` n'était testé qu'au départ).
  const writes = new Generation()
  $effect(() => {
    const ms = active ? 3000 : 10_000
    return untrack(() =>
      poll(
        async () => {
          if (busy) return
          const at = writes.current()
          const v = await api.recording()
          if (!busy && writes.fresh(at)) show(v)
        },
        ms,
        { immediate: false },
      ),
    )
  })

  async function save(changes: { allowed?: boolean; folder?: string; keep_days?: number }) {
    busy = true
    writes.bump()
    try {
      show(await api.setRecording(changes))
      notify('ok', t('store.saved'))
      return true
    } catch (e) {
      notify('error', errorText(e))
      return false
    } finally {
      busy = false
    }
  }

  function editFolder() {
    folder = view?.settings.folder ?? ''
    editing = true
  }

  async function saveFolder(event: SubmitEvent) {
    event.preventDefault()
    if (await save({ folder: folder.trim() })) editing = false
  }

  async function defaultFolder() {
    if (await save({ folder: '' })) editing = false
  }

  async function record(on: boolean) {
    busy = true
    try {
      const next = await api.record(on)
      if (view) show({ ...view, state: next })
      await load()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  async function remove(f: RecordingFile) {
    if (!confirm(t('recordings.confirmDelete'))) return
    busy = true
    try {
      if (playing === f.name) playing = null
      show(await api.deleteRecording(f.name))
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  function details(f: RecordingFile): string {
    return [f.duration_ms != null ? fileDuration(f.duration_ms) : '', fileSize(f.size)].filter(Boolean).join(' · ')
  }
</script>

<header class="space-head">
  <div>
    <span class="lbl">{t('recordings.eyebrow')}</span>
    <h1>{t('recordings.title')}</h1>
    <p>{t('recordings.intro')}</p>
  </div>
</header>

<div class="space-body">
  {#if view}
    {#if view.session || active}
      <!-- La session en cours : lancer ou arrêter. -->
      <section aria-labelledby="t-session" class="card session">
        <div class="card-head">
          <div><h2 id="t-session">{t('recordings.session')}</h2></div>
        </div>
        <div class="set">
          <div>
            {#if active}
              <span class="rec" role="status"><span class="beat" aria-hidden="true"></span>{t('recording.pill', { time: clock })}</span>
            {:else}
              <p>{view.settings.allowed ? view.session : t('recording.disabled')}</p>
            {/if}
          </div>
          {#if active}
            <button type="button" class="danger" disabled={busy} onclick={() => record(false)}>{t('recordings.stop')}</button>
          {:else}
            <button type="button" class="primary" disabled={busy || !view.settings.allowed} title={view.settings.allowed ? '' : t('recording.disabled')} onclick={() => record(true)}>
              <span class="dot-rec" aria-hidden="true"></span>{t('recordings.recordDevice', { device: view.session ?? '' })}
            </button>
          {/if}
        </div>
      </section>
    {/if}

    <section aria-labelledby="t-settings" class="card">
      <div class="card-head">
        <div><h2 id="t-settings">{t('recordings.settings')}</h2></div>
      </div>
      <div class="set">
        <div><h3>{t('recordings.allow')}</h3><p>{t('recordings.allowHelp')}</p></div>
        <Switch label={t('recordings.allow')} checked={view.settings.allowed} disabled={busy} onchange={(v) => save({ allowed: v })} />
      </div>
      <div class="set">
        <div class="folder">
          <h3>{t('recordings.folder')}</h3>
          {#if editing}
            <form class="inline" onsubmit={saveFolder}>
              <input id="rec-folder" class="mono" aria-label={t('recordings.folder')} autocomplete="off" spellcheck="false" required bind:value={folder} />
              <div class="row">
                <button type="submit" class="ghost small" disabled={busy || !folder.trim()}>{t('recordings.save')}</button>
                <button type="button" class="ghost small" disabled={busy} onclick={() => (editing = false)}>{t('recordings.cancel')}</button>
                {#if view.settings.custom_folder}
                  <button type="button" class="ghost small" disabled={busy} title={view.settings.default_folder} onclick={defaultFolder}>{t('recordings.defaultFolder')}</button>
                {/if}
              </div>
            </form>
          {:else}
            <p class="mono path">{view.settings.folder}</p>
          {/if}
          {#if view.free != null}<p>{freeSpace(view.free)}</p>{/if}
        </div>
        {#if !editing}
          <button type="button" class="ghost small" disabled={busy} onclick={editFolder}><Icon name="folder" size={15} />{t('recordings.change')}</button>
        {/if}
      </div>
      <div class="set">
        <div><h3>{t('recordings.keep')}</h3><p>{t('recordings.keepHelp')}</p></div>
        <Segmented label={t('recordings.keep')} options={KEEP} value={String(view.settings.keep_days)} disabled={busy} onchange={(v) => save({ keep_days: Number(v) })} />
      </div>
    </section>

    <section aria-labelledby="t-files" class="card files">
      <div class="card-head">
        <div><h2 id="t-files">{t('recordings.files')}</h2></div>
      </div>
      {#if !view.files.length}
        <p class="empty">{t('recordings.none')}</p>
      {/if}
      {#each view.files as f (f.name)}
        <div class="file">
          <span class="icon"><Icon name="video" size={20} stroke={1.6} /></span>
          <div class="who">
            <span class="name">
              {f.modified != null ? fileDate(f.modified) : f.name}
              {#if f.in_progress}<span class="rec small"><span class="beat" aria-hidden="true"></span>{t('recordings.inProgress')}</span>{/if}
            </span>
            <span class="sub">{details(f)}</span>
          </div>
          <div class="actions">
            <button type="button" class="ghost small" aria-expanded={playing === f.name} onclick={() => (playing = playing === f.name ? null : f.name)}>
              <Icon name={playing === f.name ? 'x' : 'play'} size={14} stroke={2} />{playing === f.name ? t('recordings.close') : t('recordings.play')}
            </button>
            <a class="ghost small" href={api.recordingFileUrl(f.name, true)} download={f.name}><Icon name="download" size={15} />{t('recordings.download')}</a>
            <button type="button" class="icon-button" aria-label={`${t('recordings.delete')} · ${f.name}`} title={t('recordings.delete')} disabled={busy || f.in_progress} onclick={() => remove(f)}><Icon name="trash" size={16} /></button>
          </div>
        </div>
        {#if playing === f.name}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video class="player" controls autoplay preload="metadata" src={api.recordingFileUrl(f.name)}></video>
        {/if}
      {/each}
    </section>
  {/if}
</div>

<style>
  /* La pastille rouge, son rond qui bat une fois par seconde (fixe quand on demande moins d'animations). */
  .rec {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
    font-size: 13.5px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .rec.small {
    padding: 2px 9px;
    font-size: 11.5px;
  }

  .beat,
  .dot-rec {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--bad);
    flex: none;
  }

  .beat {
    animation: beat 1s ease-in-out infinite;
  }

  .rec.small .beat {
    width: 7px;
    height: 7px;
  }

  .dot-rec {
    background: #fff;
  }

  @keyframes beat {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .beat {
      animation: none;
    }
  }

  .folder {
    min-width: 0;
  }

  .path {
    font-size: 12.5px;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }

  .inline {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 4px;
  }

  .inline input {
    width: 100%;
    font-size: 13px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .files {
    padding-bottom: 14px;
  }

  .empty {
    padding: 16px 0 8px;
    border-top: 1px solid var(--line);
    font-size: 13.5px;
    color: var(--faint);
  }

  .file {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    gap: 12px 18px;
    align-items: center;
    padding: 14px 0;
    border-top: 1px solid var(--line);
  }

  .icon {
    width: 44px;
    height: 44px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
    color: var(--text-2);
  }

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 15px;
    font-weight: 600;
  }

  .sub {
    font-size: 13px;
    color: var(--faint);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .player {
    display: block;
    width: 100%;
    max-height: 70vh;
    margin: 0 0 14px;
    border-radius: 12px;
    background: #000;
  }

  @media (max-width: 760px) {
    .file {
      grid-template-columns: 44px minmax(0, 1fr);
    }

    .actions {
      grid-column: 1 / -1;
      flex-wrap: wrap;
    }
  }
</style>
