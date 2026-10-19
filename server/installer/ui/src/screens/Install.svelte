<script lang="ts">
  import StatusIcon from '../lib/StatusIcon.svelte'
  import { app, go, startInstall, TASK_ORDER } from '../lib/state.svelte'

  let showLog = $state(false)
  let logBox: HTMLPreElement | undefined = $state()

  const s = $derived(app.ports.stream)
  const c = $derived(app.ports.console)
  const rows = $derived<Record<string, { label: string; meta: string; title: string; text: string }>>({
    files: { label: 'Fichiers du serveur de streaming', meta: 'Kyber 0.29', title: 'Copie des fichiers', text: 'Nous copions le moteur Kyber, ses bibliothèques vidéo (VLC, FFmpeg) et le client web dans C:\\Program Files\\KyberNow.' },
    cert: { label: 'Certificat de sécurité du serveur', meta: '', title: 'Certificat de sécurité', text: 'Un certificat unique est créé pour ce PC. Il chiffre tout ce qui circule entre vos appareils et lui.' },
    service: { label: 'Service Windows « kyber »', meta: '', title: 'Service Windows', text: 'Le serveur est enregistré comme service : il démarre seul, même avant que vous ouvriez votre session Windows.' },
    firewall: { label: `Pare-feu : ports ${s} et ${c}`, meta: '', title: 'Ouverture du pare-feu', text: `Windows bloque par défaut les connexions entrantes. Nous autorisons seulement le stream (port ${s}) et la console (port ${c}), sur votre réseau privé.` },
    vdd: { label: "Pilote d'écran virtuel", meta: 'SudoVDA', title: 'Écran virtuel', text: "Le pilote SudoVDA est ajouté. Il reste invisible tant qu'aucun appareil n'est connecté." },
    gamepad: { label: 'Pilote de manettes', meta: 'ViGEmBus', title: 'Manettes', text: 'Le pilote ViGEmBus permet au PC de voir votre manette comme si elle était branchée en USB.' },
    audio: { label: 'Haut-parleurs virtuels', meta: 'Steam', title: 'Son à distance', text: "Pendant le stream, le son du PC part vers ces haut-parleurs invisibles : le PC se tait, et votre appareil reçoit le son en stéréo, 5.1 ou 7.1." },
    admin: { label: "Console d'administration", meta: `port ${c}`, title: "Console d'administration", text: `La console démarre sur le port ${c}. Vous pourrez y régler la qualité d'image, les comptes et l'écran virtuel.` },
    power: { label: "Réglages d'alimentation", meta: '', title: 'Alimentation', text: 'Le mode Performances élevées empêche le processeur et le GPU de ralentir pendant le stream.' },
  })

  const finished = $derived(!!app.result)
  const currentId = $derived(TASK_ORDER.find((id) => app.tasks[id] === 'running'))
  const explain = $derived(
    app.failure
      ? { title: "L'installation s'est arrêtée", text: app.failure.message }
      : finished
        ? { title: 'Tout est en place', text: "Le serveur, la console et les pilotes sont installés. Il ne reste plus qu'à connecter un appareil." }
        : currentId
          ? rows[currentId]
          : app.tasks.finish === 'running'
            ? { title: 'Derniers réglages', text: 'Inscription dans les applications installées de Windows, raccourci vers la console et démarrage du serveur.' }
            : { title: 'Préparation', text: 'Arrêt des éventuelles versions précédentes et préparation du dossier.' },
  )

  const done = (id: string) => app.tasks[id] === 'done' || app.tasks[id] === 'skipped'
  const nodes = $derived([
    { label: 'Ce PC', on: done('service') },
    { label: 'Votre réseau', on: done('firewall') },
    { label: 'Vos appareils', on: done('admin') },
  ])

  $effect(() => {
    app.log.length
    if (logBox) logBox.scrollTop = logBox.scrollHeight
  })
</script>

<span class="eyebrow kn-enter">Étape 5 sur 6</span>
<div class="head kn-enter kn-d1">
  <h1>{app.failure ? 'Installation interrompue' : finished ? 'Installation terminée.' : 'Installation en cours…'}</h1>
  <span class="percent">{Math.round(app.percent)} %</span>
</div>
<div class="bar kn-enter kn-d1" role="progressbar" aria-valuenow={Math.round(app.percent)} aria-valuemin={0} aria-valuemax={100} aria-label="Progression de l'installation">
  <div class="fill" class:failed={!!app.failure} style="width: {app.percent}%">
    {#if !finished && !app.failure}<div class="sheen"></div>{/if}
  </div>
</div>

<div class="nodes kn-enter kn-d2">
  {#each nodes as node, i (node.label)}
    <div class="node" class:on={node.on}><span></span>{node.label}</div>
    {#if i < nodes.length - 1}
      <div class="link" class:kn-flow={node.on} class:on={node.on}></div>
    {/if}
  {/each}
</div>

<div class="body kn-enter kn-d3">
  <ol>
    {#each TASK_ORDER as id (id)}
      {@const status = app.tasks[id]}
      <li class:current={status === 'running'} class:pending={status === 'pending'}>
        <StatusIcon {status} size={22} />
        <span class="label">{rows[id].label}</span>
        <span class="meta">{status === 'skipped' ? 'non installé' : rows[id].meta}</span>
      </li>
    {/each}
  </ol>

  <aside class="aside" class:error={!!app.failure}>
    <span class="label-mono">{app.failure ? 'Problème' : 'En ce moment'}</span>
    {#key explain.title}
      <div class="explain">
        <strong>{explain.title}</strong>
        <p>{explain.text}</p>
      </div>
    {/key}
    <div class="spacer"></div>
    <span class="label-mono">Le saviez-vous ?</span>
    <p>L'image, le son et vos commandes voyagent ensemble dans une seule connexion chiffrée, sur le protocole QUIC.</p>
  </aside>
</div>

{#if showLog || app.failure}
  <pre bind:this={logBox}>{app.log.join('\n') || 'Préparation…'}</pre>
{/if}

<div class="footer">
  <button class="btn" onclick={() => (showLog = !showLog)}>{showLog ? 'Masquer le détail' : 'Afficher le détail'}</button>
  <div class="spacer"></div>
  {#if app.failure}
    <button class="btn" onclick={() => go('options')}>Modifier les options</button>
    <button class="btn primary" onclick={startInstall}>Réessayer</button>
  {:else}
    <button class="btn primary" class:kn-pop={finished} disabled={!finished} onclick={() => go('done')}>
      Continuer
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
  {/if}
</div>

<style>
  .head {
    margin-top: 8px;
    display: flex;
    align-items: flex-end;
    gap: 16px;
  }

  .head h1 {
    flex: 1;
    margin: 0;
    font-size: 32px;
  }

  .percent {
    font-family: var(--display);
    font-weight: 700;
    font-size: 32px;
    font-variant-numeric: tabular-nums;
    color: var(--accent);
  }

  .bar {
    margin-top: 14px;
    height: 8px;
    border-radius: 4px;
    background: #1e2733;
    overflow: hidden;
  }

  .fill {
    position: relative;
    height: 8px;
    border-radius: 4px;
    background: var(--accent);
    overflow: hidden;
    transition: width 0.15s linear;
  }

  .fill.failed {
    background: var(--danger);
  }

  .sheen {
    position: absolute;
    inset: 0 auto 0 0;
    width: 40%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.55), transparent);
    animation: kn-sheen 1.6s ease-in-out infinite;
  }

  .nodes {
    margin-top: 16px;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .node {
    height: 34px;
    padding: 0 12px;
    border-radius: 9px;
    background: rgba(24, 32, 43, 0.95);
    border: 1px solid var(--line-2);
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--faint);
    white-space: nowrap;
    transition: border-color 0.4s, color 0.4s;
  }

  .node span {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #3c4a5e;
    transition: background-color 0.4s;
  }

  .node.on {
    border-color: var(--accent);
    color: var(--text);
  }

  .node.on span {
    background: var(--accent);
  }

  .link {
    flex: 1;
    height: 2px;
    background-image: repeating-linear-gradient(90deg, var(--line-2) 0 10px, transparent 10px 16px);
    background-size: 32px 2px;
  }

  .link.on {
    background-image: repeating-linear-gradient(90deg, var(--accent) 0 10px, transparent 10px 16px);
  }

  .body {
    margin-top: 16px;
    display: flex;
    gap: 20px;
    flex: 1;
    min-height: 0;
  }

  ol {
    flex: 1;
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 12px;
    border-radius: 9px;
    transition: background-color 0.3s;
  }

  li.current {
    background: rgba(24, 32, 43, 0.95);
  }

  .label {
    flex: 1;
    font-size: 14px;
    color: var(--text);
    transition: color 0.3s;
  }

  li.pending .label {
    color: var(--faint);
  }

  .meta {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--faint);
  }

  aside {
    width: 270px;
    flex-shrink: 0;
  }

  aside.error {
    border-color: #5a2420;
    background: rgba(42, 20, 20, 0.94);
  }

  .explain {
    display: flex;
    flex-direction: column;
    gap: 8px;
    animation: kn-enter 0.45s ease-out both;
  }

  .explain strong {
    font-size: 15px;
  }

  .explain p {
    word-break: break-word;
  }

  pre {
    margin: 12px 0 0;
    height: 84px;
    overflow: auto;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--bar);
    border: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    line-height: 1.6;
    color: var(--muted);
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
  }

  .footer {
    margin-top: 14px;
    padding-top: 16px;
  }
</style>
