<!--
  Système : mises à jour, le serveur et la carte graphique, les journaux, et
  les réglages fins de Kyber (Avancé).
-->
<script lang="ts">
  import { tick } from 'svelte'
  import { api, type GpuVendor, type LogFile, type UpdateStatus } from '../lib/api'
  import { app, errorText, field, kyber, loadKyber, markRestartPending, notify, refreshStatus, setValues, value } from '../lib/store.svelte'
  import Icon from '../lib/Icon.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import Switch from '../lib/Switch.svelte'

  const status = $derived(app.status)

  // Mises à jour

  let update = $state<UpdateStatus | null>(null)
  let checking = $state(false)
  let installing = $state<string | null>(null)

  async function loadUpdate() {
    try {
      update = await api.update()
      if (update.checked_seconds_ago === null) await check()
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function check() {
    checking = true
    try {
      update = await api.updateCheck()
      if (update.error) notify('error', `GitHub : ${update.error}`)
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      checking = false
    }
  }

  async function install() {
    if (!update?.latest) return
    try {
      const r = await api.updateInstall()
      installing = r.version
      waitForRestart(r.version)
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  // La console s'arrête, remplace ses fichiers et repart : on la guette.
  function waitForRestart(version: string) {
    const started = Date.now()
    const timer = setInterval(async () => {
      try {
        const s = await api.update()
        if (s.install_error) {
          clearInterval(timer)
          installing = null
          update = s
          notify('error', s.install_error.split('\n')[0])
          return
        }
        if (s.current === version) {
          clearInterval(timer)
          installing = null
          update = s
          notify('ok', `KyberNow ${version} est installé.`)
          refreshStatus()
        }
      } catch {
        // console en cours de redémarrage
      }
      if (Date.now() - started > 300_000) {
        clearInterval(timer)
        installing = null
        notify('error', "La mise à jour n'a pas abouti en 5 minutes : voyez C:\\ProgramData\\KyberNow\\update\\<version>\\update.log.")
      }
    }, 3000)
  }

  const notes = $derived(
    (update?.notes ?? '')
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.startsWith('- '))
      .map((l) => l.slice(2).replace(/\*\*/g, ''))
      .slice(0, 6),
  )
  const published = $derived(update?.published ? new Date(update.published) : null)
  const publishedLabel = $derived(
    !published
      ? ''
      : published.toDateString() === new Date().toDateString()
        ? 'publiée aujourd’hui'
        : `publiée le ${published.toLocaleDateString('fr-FR', { day: 'numeric', month: 'long' })}`,
  )

  // Serveur

  let serviceBusy = $state(false)

  async function service(action: 'start' | 'stop' | 'restart') {
    if (action === 'stop' && !confirm('Arrêter le serveur ? Plus aucun appareil ne pourra se connecter jusqu’à son redémarrage.')) return
    serviceBusy = true
    try {
      await api.service(action)
      notify('ok', action === 'stop' ? 'Serveur arrêté.' : action === 'start' ? 'Serveur démarré.' : 'Serveur redémarré.')
      await refreshStatus()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      serviceBusy = false
    }
  }

  // La carte qui encode : celle de l'encodeur réglé, sinon la première
  // (NVIDIA, puis AMD, puis Intel, comme à l'installation).
  const ENCODERS: Record<string, { vendor?: GpuVendor; name: string }> = {
    nvenc: { vendor: 'nvidia', name: 'NVENC' },
    amf: { vendor: 'amd', name: 'AMF' },
    qsv: { vendor: 'intel', name: 'Quick Sync' },
    x264: { name: 'x264' },
  }
  const VENDOR_ENCODER: Record<GpuVendor, string> = { nvidia: 'nvenc', amd: 'amf', intel: 'qsv' }
  const encoderValue = $derived(kyber.loaded ? value<string>('video.encoder') : null)
  const gpu = $derived.by(() => {
    const devices = status?.gpu?.devices ?? []
    const vendor = encoderValue ? ENCODERS[encoderValue]?.vendor : undefined
    return devices.find((d) => d.vendor === vendor) ?? devices[0] ?? null
  })
  const encoderLine = $derived.by(() => {
    const id = encoderValue ?? (gpu ? VENDOR_ENCODER[gpu.vendor] : null)
    if (!id) return null
    const f = kyber.loaded ? field('video.encoder') : undefined
    const label = f?.kind.type === 'enum' ? f.kind.options.find((o) => o.value === id)?.label : undefined
    const name = ENCODERS[id]?.name
    if (!name || !gpu || ENCODERS[id].vendor !== gpu.vendor) return label ?? name ?? id
    // NVIDIA : sessions, images par seconde et latence de NVENC.
    if (gpu.encoder_sessions != null) {
      const fps = gpu.encoder_fps ? `, ${gpu.encoder_fps} i/s` : ''
      const latency = gpu.encoder_latency_us ? ` · ${(gpu.encoder_latency_us / 1000).toFixed(1).replace('.', ',')} ms par image` : ''
      return `${name}${gpu.encoder_sessions ? ` · ${gpu.encoder_sessions} flux${fps}` : ' · au repos'}${latency}`
    }
    const load = gpu.encoding && gpu.encoder_utilization ? `, occupé à ${gpu.encoder_utilization} %` : ''
    return `${name} · ${gpu.encoding ? `flux en cours${load}` : 'au repos'}`
  })
  const PSTATES: Record<number, string> = { 0: 'Puissance maximale', 2: 'Performances élevées', 5: 'Puissance réduite', 8: 'Au repos' }
  const serviceLine = $derived.by(() => {
    switch (status?.service.state) {
      case 'running':
        return { dot: 'ok', text: status.controller.reachable ? 'En marche' : 'En marche, le flux démarre' }
      case 'starting':
        return { dot: 'warn', text: 'Démarrage…' }
      case 'stopping':
        return { dot: 'warn', text: 'Arrêt…' }
      case 'stopped':
        return { dot: 'bad', text: 'Arrêté' }
      default:
        return { dot: 'bad', text: 'Introuvable : réinstallez KyberNow' }
    }
  })

  // Journaux

  const SOURCES = [
    { value: 'connexions', label: 'Connexions', match: (n: string) => n.startsWith('kyber/') && /controller/i.test(n) },
    { value: 'image', label: 'Image et son', match: (n: string) => n.startsWith('kyber/') && /(avserver|kyav|txproto)/i.test(n) },
    { value: 'commandes', label: 'Commandes', match: (n: string) => n.startsWith('kyber/') && /(input|kynput)/i.test(n) },
    { value: 'console', label: 'Console', match: (n: string) => n.startsWith('kybernow/') },
  ]

  let files = $state<LogFile[]>([])
  let source = $state('connexions')
  let selected = $state('')
  let text = $state('')
  let filter = $state('')
  let follow = $state(true)
  let logError = $state('')
  let pre: HTMLPreElement | undefined = $state()

  const groups = $derived.by(() => {
    const known = SOURCES.map((s) => ({ ...s, files: files.filter((f) => s.match(f.name)) }))
    const rest = files.filter((f) => !SOURCES.some((s) => s.match(f.name)))
    return rest.length ? [...known, { value: 'autres', label: 'Autres', match: () => false, files: rest }] : known
  })
  const groupFiles = $derived(groups.find((g) => g.value === source)?.files ?? [])
  const shown = $derived(filter ? text.split('\n').filter((l) => l.toLowerCase().includes(filter.toLowerCase())).join('\n') : text)

  async function loadFiles() {
    try {
      files = (await api.logs()).files
      if (!groups.find((g) => g.value === source)?.files.length) source = groups.find((g) => g.files.length)?.value ?? source
    } catch (e) {
      logError = errorText(e)
    }
  }

  $effect(() => {
    // Le fichier le plus récent de la source choisie.
    if (!groupFiles.some((f) => f.name === selected)) selected = groupFiles[0]?.name ?? ''
  })

  async function loadText() {
    if (!selected) {
      text = ''
      return
    }
    try {
      const atBottom = !pre || pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 20
      text = (await api.logTail(selected, 500)).text
      logError = ''
      if (atBottom) {
        await tick()
        pre?.scrollTo({ top: pre.scrollHeight })
      }
    } catch (e) {
      logError = errorText(e)
    }
  }

  $effect(() => {
    void selected
    loadText()
  })

  $effect(() => {
    if (!follow) return
    const timer = setInterval(loadText, 2000)
    return () => clearInterval(timer)
  })

  async function download() {
    if (!selected) return
    try {
      const full = (await api.logTail(selected, 5000)).text
      const url = URL.createObjectURL(new Blob([full], { type: 'text/plain' }))
      const a = document.createElement('a')
      a.href = url
      a.download = selected.split('/').pop() ?? 'journal.log'
      a.click()
      URL.revokeObjectURL(url)
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  function lineClass(line: string): string {
    return /\b(ERROR|ERR|FATAL)\b/.test(line) ? 'err' : /\bWARN(ING)?\b/.test(line) ? 'warn' : ''
  }

  const LEVELS = [
    { value: 'error', label: 'Erreurs' },
    { value: 'info', label: 'Normal' },
    { value: 'debug', label: 'Détaillé' },
  ]
  const LOG_FIELDS = ['logs.controller', 'logs.avserver', 'logs.inputserver']
  const level = $derived.by(() => {
    const v = LOG_FIELDS.map((id) => value<string>(id))
    return v.every((x) => x === v[0]) ? v[0] : ''
  })

  // Avancé

  let raw = $state<string | null>(null)
  let rawOriginal = $state('')
  let rawError = $state('')
  let rawBusy = $state(false)
  let passwordOpen = $state(false)
  let current = $state('')
  let next = $state('')
  let confirmNext = $state('')

  async function openRaw() {
    if (raw !== null) {
      raw = null
      return
    }
    try {
      rawOriginal = (await api.rawConfig()).text
      raw = rawOriginal
      rawError = ''
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function saveRaw() {
    if (raw === null) return
    rawBusy = true
    try {
      await api.updateRawConfig(raw)
      rawOriginal = raw
      rawError = ''
      markRestartPending()
      kyber.loaded = false
      await loadKyber()
      notify('ok', 'kyber_config.toml enregistré.')
    } catch (e) {
      rawError = errorText(e)
    } finally {
      rawBusy = false
    }
  }

  async function changePassword(event: SubmitEvent) {
    event.preventDefault()
    if (next !== confirmNext) {
      notify('error', 'Les deux mots de passe ne correspondent pas.')
      return
    }
    try {
      await api.changePassword(current, next)
      current = next = confirmNext = ''
      passwordOpen = false
      notify('ok', 'Mot de passe de la console changé. Les autres sessions ont été fermées.')
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  let minBitrate = $state<number | null>(null)
  let port = $state<number | null>(null)
  $effect(() => {
    if (kyber.loaded && minBitrate === null) minBitrate = Number(value('video.minimum_bitrate')) / 1_000_000
    if (kyber.loaded && port === null) port = Number(value('network.port'))
  })

  function choiceHint(id: string): string {
    const f = field(id)
    return f?.kind.type === 'enum' ? (f.kind.options.find((o) => o.value === value(id))?.hint ?? f.help) : (f?.help ?? '')
  }

  $effect(() => {
    loadUpdate()
    loadFiles()
    loadKyber()
    const timer = setInterval(loadFiles, 15000)
    return () => clearInterval(timer)
  })
</script>

<header class="space-head with-tabs">
  <div>
    <span class="lbl">KyberNow Server {status?.admin_version ?? ''} · Kyber 0.29</span>
    <h1>Système</h1>
    <p>Mises à jour, le serveur et la carte graphique, les journaux, et les réglages fins de Kyber.</p>
    <nav class="tabs" aria-label="Sections">
      <a href="#/systeme/maj" class:current={app.section === 'maj' || !app.section}>Mises à jour</a>
      <a href="#/systeme/serveur" class:current={app.section === 'serveur'}>Serveur</a>
      <a href="#/systeme/journaux" class:current={app.section === 'journaux'}>Journaux</a>
      <a href="#/systeme/avance" class:current={app.section === 'avance'}>Avancé</a>
    </nav>
  </div>
</header>

<div class="space-body">
  <!-- Mises à jour -->
  {#if installing}
    <section id="maj" class="card feature update" aria-live="polite">
      <div class="update-text">
        <span class="lbl signal">Installation en cours</span>
        <h2 class="disp big">KyberNow {installing}</h2>
        <p class="lead">La console s'arrête, remplace ses fichiers et revient d'elle-même, dans environ une minute. Cette page se recharge seule.</p>
      </div>
      <div class="progress" aria-hidden="true"><span></span></div>
    </section>
  {:else if update?.available && update.latest}
    <section id="maj" aria-labelledby="t-maj" class="card feature update">
      <div class="update-text">
        <span class="lbl signal">Nouvelle version · {publishedLabel}</span>
        <h2 id="t-maj" class="disp big">KyberNow {update.latest}</h2>
        {#if notes.length}
          <ul>{#each notes as n}<li>{n}</li>{/each}</ul>
        {/if}
        {#if update.page}<a href={update.page} target="_blank" rel="noreferrer">Toutes les notes de version</a>{/if}
      </div>
      <div class="update-action">
        {#if update.installable}
          <button type="button" class="primary lit" onclick={install}>Installer maintenant</button>
          <span>Environ une minute. Une session en cours sera coupée. Vous avez {update.current}.</span>
          {#if update.install_error}
            <pre class="install-error">{update.install_error}</pre>
          {/if}
        {:else}
          <a class="ghost" href={update.page ?? '#'} target="_blank" rel="noreferrer">Télécharger KyberNow-Setup.exe</a>
          <span>Cette version s'installe avec l'installateur. Vous avez {update.current}.</span>
        {/if}
      </div>
    </section>
  {:else}
    <section id="maj" aria-labelledby="t-maj" class="card uptodate">
      <div class="card-head">
        <div>
          <h2 id="t-maj">Mises à jour</h2>
          <span class="sub">
            {#if update?.error}GitHub ne répond pas : {update.error}{:else}KyberNow {update?.current ?? status?.admin_version ?? ''} est la dernière version{update?.checked_seconds_ago != null ? ` · vérifié il y a ${update.checked_seconds_ago < 120 ? 'un instant' : `${Math.round(update.checked_seconds_ago / 60)} min`}` : ''}.{/if}
          </span>
        </div>
        <button type="button" class="ghost" disabled={checking} onclick={check}>{checking ? 'Vérification…' : 'Vérifier maintenant'}</button>
      </div>
      {#if update?.install_error}
        <!-- Échec signalé après un retour à la version précédente (update.ps1). -->
        <pre class="install-error">{update.install_error}</pre>
      {/if}
    </section>
  {/if}

  <!-- Serveur -->
  <section id="serveur" aria-labelledby="t-srv" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-srv">Serveur</h2>
        <span class="state"><span class="dot {serviceLine.dot}"></span>{serviceLine.text}{status?.service.simulated ? ' (simulé hors Windows)' : ''}</span>
      </div>
      <div class="buttons">
        {#if status?.service.state === 'stopped'}
          <button type="button" class="ghost" disabled={serviceBusy} onclick={() => service('start')}><Icon name="play" size={15} />Démarrer</button>
        {:else}
          <button type="button" class="ghost" disabled={serviceBusy} onclick={() => service('restart')}><Icon name="restart" size={15} />Redémarrer</button>
          <button type="button" class="ghost" disabled={serviceBusy} onclick={() => service('stop')}><Icon name="stop" size={15} />Arrêter</button>
        {/if}
      </div>
    </div>
    {#if kyber.loaded && field('network.tray')}
      {@const f = field('network.tray')!}
      <div class="set">
        <div><h3>Icône de Kyber près de l'horloge</h3><p>{f.help} L'icône KyberNow, elle, se règle dans son menu (démarrage avec l'ordinateur).</p></div>
        <Switch label={f.label} checked={!!value('network.tray')} onchange={(v) => setValues({ 'network.tray': v })} />
      </div>
    {/if}
    <div class="kv-grid">
      <div>
        <div class="kv"><span>Carte graphique</span><span>{gpu ? `${gpu.name}${(gpu.driver ?? status?.gpu?.driver) ? ` · pilote ${gpu.driver ?? status?.gpu?.driver}` : ''}` : status ? 'Aucune carte graphique compatible' : '—'}</span></div>
        {#if encoderLine}
          <div class="kv"><span>Encodeur</span><span>{encoderLine}</span></div>
        {/if}
        {#if gpu && (gpu.pstate != null || gpu.temperature_c != null || gpu.power_w != null)}
          <div class="kv"><span>Alimentation</span><span>{gpu.pstate != null ? (PSTATES[gpu.pstate] ?? `P${gpu.pstate}`) : '—'}{gpu.temperature_c != null ? ` · ${gpu.temperature_c} °C` : ''}{gpu.power_w != null ? ` · ${Math.round(gpu.power_w)} W` : ''}</span></div>
        {:else if gpu?.gpu_utilization != null}
          <div class="kv"><span>Charge</span><span>{gpu.gpu_utilization} %</span></div>
        {/if}
      </div>
      <div>
        <div class="kv"><span>Adresse</span><span class="mono small">{status?.lan_ip ?? '—'}</span></div>
        <div class="kv"><span>Ports</span><span class="mono small">flux {status?.controller.port ?? '—'} · console {location.port || '443'}</span></div>
        <div class="kv"><span>Système</span><span>{status?.os ?? '—'}</span></div>
      </div>
    </div>
  </section>

  <!-- Journaux -->
  <section id="journaux" aria-labelledby="t-log" class="card logs">
    <div class="card-head">
      <h2 id="t-log">Journaux</h2>
      <Segmented label="Journal" options={groups.map((g) => ({ value: g.value, label: g.label }))} value={source} onchange={(v) => (source = v)} />
    </div>
    <div class="log-tools">
      <label for="filter" class="sr-only">Filtrer</label>
      <input id="filter" type="search" placeholder="Filtrer, ex. ERROR" bind:value={filter} />
      {#if groupFiles.length > 1}
        <label for="file" class="sr-only">Fichier</label>
        <select id="file" bind:value={selected}>{#each groupFiles as f (f.name)}<option value={f.name}>{f.name.split('/').pop()}</option>{/each}</select>
      {/if}
      <button type="button" class="ghost follow" class:on={follow} aria-pressed={follow} onclick={() => (follow = !follow)}><span class="dot" class:live={follow}></span>Suivre</button>
      <button type="button" class="ghost" disabled={!selected} onclick={download}><Icon name="download" size={15} />Télécharger</button>
    </div>
    <pre class="mono" bind:this={pre}>{#if logError}<span class="err">{logError}</span>{:else if !selected}<span class="muted">Aucun journal pour cette source.</span>{:else}{#each shown.split('\n') as line}<span class={lineClass(line)}>{line}
</span>{/each}{/if}</pre>
    {#if kyber.loaded}
      <div class="set">
        <div><h3>Niveau de détail</h3><p>Normal suffit au quotidien. Détaillé le temps de comprendre un problème : les journaux grossissent vite.</p></div>
        <Segmented label="Niveau de détail" options={LEVELS} value={level} onchange={(v) => setValues(Object.fromEntries(LOG_FIELDS.map((id) => [id, v])))} />
      </div>
    {/if}
  </section>

  <!-- Avancé -->
  <section id="avance" aria-labelledby="t-adv" class="card">
    <div class="card-head">
      <div>
        <h2 id="t-adv">Avancé</h2>
        <span class="sub wide">Les réglages de Kyber lui-même. Les valeurs par défaut conviennent presque toujours ; un mauvais réglage peut empêcher les appareils de se connecter.</span>
      </div>
    </div>
    {#if kyber.loaded}
      {@const encoder = field('video.encoder')}
      {@const backend = field('video.backend')}
      {@const intra = field('video.intra_refresh')}
      {@const listen = field('network.listen_mode')}
      {#if encoder?.kind.type === 'enum'}
        <div class="set">
          <div><label class="title" for="enc">Encodeur</label><p>{choiceHint('video.encoder')}</p></div>
          <select id="enc" value={value<string>('video.encoder')} onchange={(e) => setValues({ 'video.encoder': e.currentTarget.value })}>
            {#each encoder.kind.options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
          </select>
        </div>
      {/if}
      {#if backend?.kind.type === 'enum'}
        <div class="set">
          <div><h3>Moteur de capture</h3><p>{choiceHint('video.backend')}</p></div>
          <Segmented label="Moteur de capture" options={backend.kind.options.map((o) => ({ value: o.value, label: o.label.replace(/\s*\(.*\)/, '') }))} value={value<string>('video.backend')} onchange={(v) => setValues({ 'video.backend': v })} />
        </div>
      {/if}
      {#if intra}
        <div class="set">
          <div><h3>{intra.label}</h3><p>{intra.help}</p></div>
          <Switch label={intra.label} checked={!!value('video.intra_refresh')} onchange={(v) => setValues({ 'video.intra_refresh': v })} />
        </div>
      {/if}
      <div class="set">
        <div><label class="title" for="minbr">Débit minimum</label><p>{field('video.minimum_bitrate')?.help}</p></div>
        <form class="inline-number" onsubmit={(e) => (e.preventDefault(), setValues({ 'video.minimum_bitrate': Math.round((minBitrate ?? 0) * 1_000_000) }))}>
          <input id="minbr" type="number" min="1" max="1000" step="1" bind:value={minBitrate} /><span class="unit">Mbit/s</span>
          <button type="submit" class="ghost small">Enregistrer</button>
        </form>
      </div>
      <div class="set">
        <div><label class="title" for="port">Port du flux</label><p>Tiré au hasard à l'installation. Le changer oblige à réajouter l'ordinateur sur vos appareils, et à ouvrir le nouveau port dans le pare-feu.</p></div>
        <form class="inline-number" onsubmit={(e) => (e.preventDefault(), confirm('Changer le port du flux ? Vos appareils devront réajouter cet ordinateur.') && setValues({ 'network.port': port }))}>
          <input id="port" type="number" min="1024" max="65535" bind:value={port} />
          <button type="submit" class="ghost small">Enregistrer</button>
        </form>
      </div>
      {#if listen?.kind.type === 'enum'}
        <div class="set">
          <div><h3>{listen.label}</h3><p>{choiceHint('network.listen_mode')}</p></div>
          <Segmented label={listen.label} options={listen.kind.options} value={value<string>('network.listen_mode')} onchange={(v) => setValues({ 'network.listen_mode': v })} />
        </div>
      {/if}
      {#each ['auth.basic_enabled', 'auth.jwt_enabled'] as id (id)}
        {@const f = field(id)}
        {#if f}
          <div class="set">
            <div><h3>{f.label}</h3><p>{f.help}{f.caution ? ` ${f.caution}` : ''}</p></div>
            <Switch label={f.label} checked={!!value(id)} onchange={(v) => setValues({ [id]: v })} />
          </div>
        {/if}
      {/each}
    {/if}
    <div class="set">
      <div><h3>Fichier de configuration</h3><p>kyber_config.toml, en direct. La syntaxe est vérifiée avant l'enregistrement, pas le sens des valeurs.</p></div>
      <button type="button" class="ghost" aria-expanded={raw !== null} onclick={openRaw}>{raw === null ? "Ouvrir l'éditeur" : "Fermer l'éditeur"}</button>
      {#if raw !== null}
        <div class="editor">
          <label for="raw" class="sr-only">Contenu de kyber_config.toml</label>
          <textarea id="raw" rows="18" spellcheck="false" bind:value={raw}></textarea>
          {#if rawError}<div class="notice bad" role="alert"><Icon name="warning" /><span>{rawError}</span></div>{/if}
          <div class="buttons">
            <button type="button" class="ghost" disabled={raw === rawOriginal} onclick={() => (raw = rawOriginal)}>Annuler les changements</button>
            <button type="button" class="ghost" disabled={rawBusy || raw === rawOriginal} onclick={saveRaw}>{rawBusy ? 'Enregistrement…' : 'Enregistrer'}</button>
          </div>
        </div>
      {/if}
    </div>
    <div class="set">
      <div><h3>Mot de passe de la console</h3><p>Celui choisi à l'installation. Le changer ferme les autres sessions ouvertes.</p></div>
      <button type="button" class="ghost" aria-expanded={passwordOpen} onclick={() => (passwordOpen = !passwordOpen)}>{passwordOpen ? 'Annuler' : 'Changer'}</button>
      {#if passwordOpen}
        <form class="password" onsubmit={changePassword}>
          <div class="field"><label for="cur">Mot de passe actuel</label><input id="cur" type="password" required autocomplete="current-password" bind:value={current} /></div>
          <div class="field"><label for="nx">Nouveau (10 caractères au moins)</label><input id="nx" type="password" minlength="10" required autocomplete="new-password" bind:value={next} /></div>
          <div class="field"><label for="cf">Confirmation</label><input id="cf" type="password" minlength="10" required autocomplete="new-password" bind:value={confirmNext} /></div>
          <button type="submit" class="ghost">Changer le mot de passe</button>
        </form>
      {/if}
    </div>
  </section>
</div>

<style>
  .update {
    display: flex;
    flex-wrap: wrap;
    gap: 24px 36px;
  }

  .update-text {
    flex: 1 1 380px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .lbl.signal {
    color: var(--signal-light);
  }

  .big {
    font-size: 30px;
    font-weight: 650;
    letter-spacing: -0.03em;
  }

  .update ul {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 14px;
    line-height: 1.5;
    color: var(--text-2);
  }

  .update a {
    font-size: 13px;
  }

  .lead {
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--text-2);
    max-width: 56ch;
  }

  .update-action {
    flex: 1 1 260px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    justify-content: center;
    align-items: flex-start;
  }

  .install-error {
    margin: 0;
    max-width: 100%;
    max-height: 14rem;
    overflow: auto;
    padding: 0.75rem;
    border-radius: 8px;
    background: var(--bad-bg);
    border: 1px solid var(--bad-line);
    color: var(--bad-text);
    font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, monospace;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .update-action span {
    font-size: 13px;
    line-height: 1.5;
    color: var(--faint);
    max-width: 34ch;
  }

  .progress {
    flex: 1 1 100%;
    height: 3px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.1);
    overflow: hidden;
  }

  .progress span {
    display: block;
    width: 30%;
    height: 100%;
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
    animation: slide 1.6s var(--ease-out) infinite;
  }

  @keyframes slide {
    from {
      translate: -100% 0;
    }
    to {
      translate: 340% 0;
    }
  }

  .uptodate {
    padding-bottom: 12px;
  }

  .card-head .state {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13.5px;
    color: var(--text-2);
  }

  .card-head .wide {
    max-width: 70ch;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .kv-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(300px, 100%), 1fr));
    gap: 0 36px;
    margin: 4px 0 12px;
  }

  .small {
    font-size: 12.5px;
  }

  .logs {
    padding-bottom: 12px;
  }

  .logs .card-head {
    align-items: center;
  }

  .log-tools {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    margin-bottom: 12px;
  }

  .log-tools input {
    flex: 1 1 240px;
    height: 36px;
    font-size: 13px;
  }

  .log-tools select {
    height: 36px;
    font-size: 13px;
  }

  .follow.on {
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.55);
    color: var(--signal-light);
  }

  pre {
    height: 260px;
    margin: 0 0 6px;
    overflow: auto;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--ink-0);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 11.5px;
    line-height: 1.75;
    color: var(--muted);
    white-space: pre;
  }

  pre .warn {
    color: var(--warn-text);
  }

  pre .err {
    color: var(--bad-text);
  }

  .inline-number {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .inline-number input {
    width: 110px;
    height: 36px;
  }

  .unit {
    font-size: 13px;
    color: var(--faint);
  }

  .editor,
  .password {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .editor textarea {
    width: 100%;
    resize: vertical;
  }

  .editor .buttons {
    justify-content: flex-end;
  }

  .password {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(200px, 100%), 1fr));
    align-items: end;
  }
</style>
