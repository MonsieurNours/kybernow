<!--
  Étape 1 · Bienvenue. L'analyse de l'ordinateur tourne déjà à côté : rien
  ne change sur l'ordinateur avant le clic sur Installer.
-->
<script lang="ts">
  import StatusIcon from '../lib/StatusIcon.svelte'
  import { app, go, rerunChecks, validatePorts } from '../lib/state.svelte'

  let editing = $state(false)
  let stream = $state('')
  let consolePort = $state('')

  const running = $derived(!app.checksDone)
  const problems = $derived(app.checks.filter((c) => c.status === 'error').length)

  function edit() {
    stream = String(app.ports.stream)
    consolePort = String(app.ports.console)
    app.portErrors = { stream: null, console: null }
    editing = true
  }

  async function confirmPorts() {
    if (await validatePorts(Number(stream), Number(consolePort))) editing = false
  }

  const digits = (v: string) => v.replace(/[^0-9]/g, '').slice(0, 5)

  function detail(id: string, text: string) {
    // Le détail du contrôle des ports suit les valeurs modifiées à la main.
    return id === 'ports' && app.checksDone ? text.replace(/stream \d+ · console \d+/, `flux ${app.ports.stream} · console ${app.ports.console}`) : text
  }
</script>

<div class="welcome">
  <div class="intro">
    <span class="lbl signal kn-enter">Bienvenue</span>
    <h1 class="title kn-enter kn-d1">Cet ordinateur, sur tous vos écrans.</h1>
    <p class="lead kn-enter kn-d2">
      Depuis un téléphone, une tablette, une télé ou un autre ordinateur, vous retrouvez cet écran comme s'il était devant vous : l'image en quelques millisecondes, le
      clavier, la souris et la manette qui répondent tout de suite.
    </p>
    <p class="note kn-enter kn-d3">Trois minutes. Rien ne change sur cet ordinateur avant que vous cliquiez sur Installer.</p>
  </div>

  <div class="panel checks kn-enter kn-d2">
    <div class="checks-head">
      <span class="lbl">{running ? 'Nous vérifions…' : 'Ce que nous avons vérifié'}</span>
      {#if !running}<button type="button" class="text-button again" onclick={rerunChecks}>Relancer</button>{/if}
    </div>
    <ul>
      {#each app.checks as check (check.id)}
        <li class:pending={check.status === 'pending'}>
          <StatusIcon status={check.status} />
          <div class="text">
            <span class="name">
              {check.title}
              {#if check.id === 'ports' && check.status === 'ok'}
                <button type="button" class="ghost small" onclick={editing ? confirmPorts : edit} disabled={app.portsPending}>{editing ? 'Valider' : 'Modifier'}</button>
              {/if}
            </span>
            {#if check.status !== 'pending' && check.detail}
              <span class="detail" class:mono={['gpu', 'driver', 'display', 'ports'].includes(check.id)} class:warn={check.status === 'warn'} class:bad={check.status === 'error'}>{detail(check.id, check.detail)}</span>
            {/if}
            {#if check.id === 'ports' && editing}
              <div class="ports">
                <label>Flux <input class="plain mono" bind:value={stream} oninput={() => (stream = digits(stream))} inputmode="numeric" aria-invalid={!!app.portErrors.stream} /></label>
                <label>Console <input class="plain mono" bind:value={consolePort} oninput={() => (consolePort = digits(consolePort))} inputmode="numeric" aria-invalid={!!app.portErrors.console} /></label>
              </div>
              {#if app.portErrors.stream || app.portErrors.console}<span class="detail bad">{app.portErrors.stream ?? app.portErrors.console}</span>{/if}
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  </div>
</div>

<div class="footer">
  {#if problems}<span class="note bad-note">{problems === 1 ? 'Un point empêche' : `${problems} points empêchent`} un streaming correct : voyez le détail ci-dessus.</span>{/if}
  <div class="spacer"></div>
  <button type="button" class="primary" disabled={running || editing} onclick={() => go(app.existing ? 'reinstall' : 'account')}>
    {running ? 'Analyse en cours…' : 'Continuer'}
  </button>
</div>

<style>
  .welcome {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    gap: 40px;
    align-items: start;
    padding-top: 24px;
  }

  .intro {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 20px;
  }

  .intro h1 {
    font-size: 38px;
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
    padding: 11px 0;
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
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    font-size: 14px;
  }

  .detail {
    font-size: 12px;
    line-height: 1.45;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .detail.mono {
    font-size: 11px;
  }

  .detail.warn {
    color: var(--warn-text);
  }

  .detail.bad {
    color: var(--bad-text);
  }

  .ports {
    display: flex;
    gap: 10px;
    margin-top: 6px;
  }

  .ports label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
    color: var(--faint);
  }

  .ports input {
    width: 100px;
    height: 34px;
    font-size: 13px;
  }

  .bad-note {
    color: var(--bad-text);
  }
</style>
