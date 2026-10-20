<!--
  Étape 4 · Installation. Chaque tâche apparaît à son tour ; la couture du
  symbole, dans la colonne, se remplit avec la progression.
-->
<script lang="ts">
  import StatusIcon from '../lib/StatusIcon.svelte'
  import { app, go, startInstall, TASK_ORDER } from '../lib/state.svelte'

  let showLog = $state(false)
  let logBox: HTMLPreElement | undefined = $state()

  const s = $derived(app.ports.stream)
  const c = $derived(app.ports.console)
  const rows = $derived<Record<string, { title: string; text: string; fact?: string }>>({
    files: { title: 'Moteur de streaming Kyber 0.29', text: 'Le moteur, ses bibliothèques vidéo et le client web se posent dans C:\\Program Files\\KyberNow.' },
    cert: { title: 'Certificat de cet ordinateur', text: 'Il chiffre tout ce qui circule entre vos appareils et lui.' },
    service: { title: 'Démarrage avec l’ordinateur', text: 'Le serveur devient un service : il démarre seul, avant même votre session Windows.' },
    firewall: { title: 'Réseau', text: `Ouverture des ports ${s} (flux) et ${c} (console), sur votre réseau privé seulement.`, fact: `flux ${s} · console ${c}` },
    vdd: { title: 'Écran virtuel', text: 'Il prendra la taille de chaque appareil qui se connecte.', fact: 'SudoVDA' },
    gamepad: { title: 'Manettes', text: 'Pour jouer depuis n’importe quel appareil.', fact: 'ViGEmBus' },
    audio: { title: 'Son à distance', text: 'Les haut-parleurs virtuels de Steam : l’ordinateur se tait, l’appareil reçoit le son.' },
    admin: { title: 'Console d’administration', text: `Elle démarre sur le port ${c}.`, fact: `port ${c}` },
    power: { title: 'Alimentation', text: 'Le mode Performances élevées évite les ralentissements pendant une session.' },
  })

  const finished = $derived(!!app.result)
  const visible = $derived(TASK_ORDER.filter((id) => app.tasks[id] !== 'skipped'))
  const lastLine = $derived(app.log.at(-1)?.replace(/^\s*/, '› ') ?? '› préparation')
  const remaining = $derived(
    finished || app.failure ? '' : app.percent < 30 ? 'Encore moins d’une minute' : app.percent < 75 ? 'Encore une trentaine de secondes' : 'Encore quelques secondes',
  )

  $effect(() => {
    void app.log.length
    if (logBox) logBox.scrollTop = logBox.scrollHeight
  })
</script>

<div class="install">
  <div class="head">
    <span class="lbl signal">
      {app.failure ? 'Installation interrompue' : finished ? 'Installation terminée' : `Installation · ${Math.round(app.percent)} %`}
    </span>
    <h1 class="title">{app.failure ? 'Quelque chose a bloqué.' : finished ? 'Tout est en place.' : 'Cet ordinateur, bientôt sur tous vos écrans.'}</h1>
    <p class="lead">
      {app.failure
        ? app.failure.message
        : finished
          ? 'Le serveur, la console et les pilotes sont installés.'
          : 'KyberNow Server se met en place. Rien à faire : on vous montre ce qui se passe, au fur et à mesure.'}
    </p>
  </div>

  {#if showLog || app.failure}
    <pre bind:this={logBox} class="mono">{app.log.join('\n') || 'Préparation…'}</pre>
  {:else}
    <ol>
      {#each visible as id (id)}
        {@const status = app.tasks[id]}
        <li class:current={status === 'running'} class:todo={status === 'pending'} class:failed={status === 'failed'}>
          <StatusIcon {status} />
          <div>
            <span class="t">{rows[id].title}</span>
            {#if status === 'running'}
              <span class="d">{rows[id].text}</span>
              <span class="bar" aria-hidden="true"><span></span></span>
            {:else if (status === 'done' && rows[id].fact) || status === 'failed'}
              <span class="mono f">{status === 'failed' ? 'échec' : rows[id].fact}</span>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<div class="footer">
  <span class="mono log">{lastLine}</span>
  <div class="spacer"></div>
  {#if remaining}<span class="remaining">{remaining}</span>{/if}
  {#if app.failure}
    <button type="button" class="ghost" onclick={() => go('options')}>Modifier les options</button>
    <button type="button" class="primary" onclick={startInstall}>Réessayer</button>
  {:else if finished}
    <button type="button" class="primary lit" onclick={() => go('done')}>Continuer</button>
  {:else}
    <button type="button" class="ghost" onclick={() => (showLog = !showLog)}>{showLog ? 'Masquer le détail' : 'Voir le détail'}</button>
  {/if}
</div>

<style>
  .install {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding-top: 18px;
    max-width: 680px;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .head h1 {
    font-size: 36px;
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0 0 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  li {
    display: grid;
    grid-template-columns: 20px 1fr;
    gap: 12px;
    align-items: start;
  }

  li > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .t {
    font-size: 14.5px;
  }

  li.current {
    margin: 2px -16px;
    padding: 12px 16px 14px;
    border-radius: 12px;
    background: rgb(109 91 255 / 0.1);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.45);
    animation: kn-enter 0.45s var(--ease-out) both;
  }

  li.current .t {
    font-weight: 600;
  }

  li.todo {
    opacity: 0.5;
  }

  li.failed .t,
  li.failed .f {
    color: var(--bad-text);
  }

  .d {
    font-size: 13px;
    color: var(--text-2);
  }

  .f {
    font-size: 11px;
    color: var(--faint);
  }

  .bar {
    margin-top: 6px;
    height: 3px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.1);
    overflow: hidden;
  }

  .bar span {
    display: block;
    width: 35%;
    height: 100%;
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
    animation: slide 1.4s var(--ease-out) infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }

  pre {
    flex: 1;
    min-height: 0;
    margin: 0 0 16px;
    padding: 14px 16px;
    overflow: auto;
    border-radius: 12px;
    background: var(--ink-1);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: 11px;
    line-height: 1.7;
    color: var(--muted);
    white-space: pre-wrap;
    user-select: text;
  }

  .log {
    font-size: 11px;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 0 1 auto;
  }

  .remaining {
    font-size: 13px;
    color: var(--muted);
    white-space: nowrap;
  }
</style>
