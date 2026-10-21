<!--
  Étape 1 · Bienvenue. L'analyse du PC tourne déjà à côté : rien ne change
  avant le clic sur Installer.
-->
<script lang="ts">
  import StatusIcon from '../lib/StatusIcon.svelte'
  import Toggle from '../lib/Toggle.svelte'
  import { app, rerunChecks, startInstall } from '../lib/state.svelte'

  const running = $derived(!app.checksDone)
  const update = $derived(!!app.existing)
</script>

<div class="welcome">
  <div class="intro">
    {#if update}
      <span class="lbl signal kn-enter">Mise à jour</span>
      <h1 class="title kn-enter kn-d1">Une nouvelle version de KyberNow.</h1>
      <p class="lead kn-enter kn-d2">
        Vous passez de la version {app.existing} à la {__APP_VERSION__}. Vos ordinateurs, leurs mots de passe et vos réglages restent où ils sont.
      </p>
    {:else}
      <span class="lbl signal kn-enter">Bienvenue</span>
      <h1 class="title kn-enter kn-d1">Votre ordinateur, sur ce PC.</h1>
      <p class="lead kn-enter kn-d2">
        Retrouvez l'écran de votre ordinateur ici, comme s'il était devant vous : l'image en quelques millisecondes, le clavier, la souris et la manette qui
        répondent tout de suite.
      </p>
    {/if}
    <p class="note kn-enter kn-d3">Une minute, pour vous seul, sans droits d'administrateur. Rien ne change sur ce PC avant que vous cliquiez sur {update ? 'Mettre à jour' : 'Installer'}.</p>

    <div class="option kn-enter kn-d3">
      <div>
        <span class="name">Raccourci sur le Bureau</span>
        <span class="note">KyberNow est toujours dans le menu Démarrer.</span>
      </div>
      <Toggle bind:checked={app.desktop} label="Raccourci sur le Bureau" />
    </div>
    {#if app.dir}<p class="mono where kn-enter kn-d3">{app.dir}</p>{/if}
  </div>

  <div class="panel checks kn-enter kn-d2">
    <div class="checks-head">
      <span class="lbl">{running ? 'Nous regardons ce PC…' : 'Ce que nous avons vu'}</span>
      {#if !running}<button type="button" class="text-button again" onclick={rerunChecks}>Relancer</button>{/if}
    </div>
    <ul>
      {#each app.checks as check (check.id)}
        <li class:pending={check.status === 'pending'}>
          <StatusIcon status={check.status} />
          <div class="text">
            <span class="name">{check.title}</span>
            {#if check.status !== 'pending' && check.detail}
              <span class="detail" class:warn={check.status === 'warn'} class:bad={check.status === 'error'}>{check.detail}</span>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  </div>
</div>

<div class="footer">
  {#if app.running}<span class="note warn-note">KyberNow sera fermée pendant l'installation.</span>{/if}
  <div class="spacer"></div>
  <button type="button" class="primary" disabled={running} onclick={startInstall}>
    {running ? 'Analyse en cours…' : update ? 'Mettre à jour' : 'Installer'}
  </button>
</div>

<style>
  .welcome {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 36px;
    align-items: start;
    padding-top: 16px;
  }

  .intro {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 16px;
  }

  .intro h1 {
    font-size: 36px;
  }

  .option {
    margin-top: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--ink-1);
    box-shadow: inset 0 0 0 1px var(--line);
  }

  .option > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .option .note {
    font-size: 12px;
  }

  .where {
    margin: -6px 0 0;
    font-size: 11px;
    color: var(--ghost);
    overflow-wrap: anywhere;
  }

  .checks {
    padding: 16px 18px 6px;
    max-height: 100%;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .checks-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 6px;
  }

  .again {
    font-size: 12px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: 12px;
    align-items: start;
    padding: 10px 0;
    border-top: 1px solid var(--line);
    transition: opacity 0.3s;
  }

  li.pending {
    opacity: 0.45;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .name {
    font-size: 14px;
  }

  .detail {
    font-size: 12px;
    line-height: 1.45;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .detail.warn,
  .warn-note {
    color: var(--warn-text);
  }

  .detail.bad {
    color: var(--bad-text);
  }
</style>
