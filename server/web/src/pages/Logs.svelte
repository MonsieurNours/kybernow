<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { api, type LogFile } from '../lib/api'
  import { errorText } from '../lib/store.svelte'

  let files = $state<LogFile[]>([])
  let selected = $state('')
  let lines = $state(500)
  let text = $state('')
  let filter = $state('')
  let follow = $state(true)
  let error = $state('')
  let pre: HTMLPreElement | undefined = $state()

  const shown = $derived(
    filter ? text.split('\n').filter((l) => l.toLowerCase().includes(filter.toLowerCase())).join('\n') : text,
  )

  async function loadFiles() {
    files = (await api.logs()).files
    if (!selected && files.length) selected = files[0].name
  }

  async function loadText() {
    if (!selected) return
    try {
      const atBottom = !pre || pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 20
      text = (await api.logTail(selected, lines)).text
      error = ''
      if (atBottom) {
        await tick()
        pre?.scrollTo({ top: pre.scrollHeight })
      }
    } catch (e) {
      error = errorText(e)
    }
  }

  onMount(() => {
    loadFiles().catch((e) => (error = errorText(e)))
  })

  $effect(() => {
    selected
    lines
    loadText()
  })

  $effect(() => {
    if (!follow) return
    const timer = setInterval(loadText, 2000)
    return () => clearInterval(timer)
  })

  const size = (b: number) => (b > 1_048_576 ? `${(b / 1_048_576).toFixed(1)} Mo` : `${Math.ceil(b / 1024)} Ko`)
</script>

<header>
  <h1>Journaux</h1>
  <p class="muted">Fichiers du dossier <code>log</code> de Kyber, du plus récent au plus ancien.</p>
</header>

<section class="card stack">
  <div class="row">
    <select bind:value={selected} disabled={!files.length}>
      {#each files as f (f.name)}
        <option value={f.name}>{f.name} ({size(f.size)})</option>
      {:else}
        <option value="">Aucun journal</option>
      {/each}
    </select>
    <select bind:value={lines}>
      <option value={200}>200 lignes</option>
      <option value={500}>500 lignes</option>
      <option value={2000}>2000 lignes</option>
    </select>
    <input class="filter" placeholder="Filtrer (ex. ERROR)" bind:value={filter} />
    <span class="spacer"></span>
    <label class="row follow"><input type="checkbox" bind:checked={follow} /> Suivre</label>
    <button onclick={() => loadFiles().then(loadText)}>Actualiser</button>
  </div>
  {#if error}
    <div class="notice danger">{error}</div>
  {/if}
  <pre bind:this={pre}>{shown || ' '}</pre>
</section>

<style>
  .filter {
    min-width: 180px;
  }

  .follow {
    gap: 6px;
    font-size: 0.9rem;
  }

  pre {
    margin: 0;
    height: 60vh;
    overflow: auto;
    padding: 12px 14px;
    background: var(--surface-2);
    border-radius: 8px;
    white-space: pre-wrap;
    word-break: break-word;
    line-height: 1.45;
  }
</style>
