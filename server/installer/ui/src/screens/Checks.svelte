<script lang="ts">
  import StatusIcon from '../lib/StatusIcon.svelte'
  import { app, go, rerunChecks, validatePorts } from '../lib/state.svelte'

  let editing = $state(false)
  let stream = $state('')
  let consolePort = $state('')

  const running = $derived(!app.checksDone)

  function edit() {
    stream = String(app.ports.stream)
    consolePort = String(app.ports.console)
    app.portErrors = { stream: null, console: null }
    editing = true
  }

  async function confirmPorts() {
    const ok = await validatePorts(Number(stream), Number(consolePort))
    if (ok) editing = false
  }

  const digits = (v: string) => v.replace(/[^0-9]/g, '').slice(0, 5)

  function portsDetail(detail: string) {
    // Le détail du contrôle des ports suit les valeurs modifiées à la main.
    return detail.replace(/stream \d+ · console \d+/, `stream ${app.ports.stream} · console ${app.ports.console}`)
  }
</script>

<span class="eyebrow kn-enter">Étape 2 sur 6</span>
<h1 class="kn-enter kn-d1" style="font-size: 32px">
  {running ? 'Analyse de votre ordinateur…' : 'Votre ordinateur est prêt pour le streaming.'}
</h1>
<p class="lead kn-enter kn-d2" style="margin-top: 6px">
  Nous vérifions ce qui compte pour une image fluide et une latence basse. Rien n'est modifié à cette étape.
</p>

<div class="body kn-enter kn-d3">
  <ul>
    {#each app.checks as check (check.id)}
      <li class={check.status}>
        <StatusIcon status={check.status} />
        <div class="text">
          <div class="row">
            <strong>{check.title}</strong>
            {#if check.id === 'ports' && check.status === 'ok'}
              <button class="mini" onclick={editing ? confirmPorts : edit} disabled={app.portsPending}>
                {editing ? 'Valider' : 'Modifier'}
              </button>
            {/if}
          </div>
          <span class="detail" class:mono={['gpu', 'driver', 'display'].includes(check.id)}>
            {check.status === 'pending' ? 'En attente…' : check.id === 'ports' ? portsDetail(check.detail) : check.detail}
          </span>
          {#if check.id === 'ports' && editing}
            <div class="ports">
              <label>
                Stream
                <input bind:value={stream} oninput={() => (stream = digits(stream))} inputmode="numeric" aria-invalid={!!app.portErrors.stream} />
              </label>
              <label>
                Console
                <input bind:value={consolePort} oninput={() => (consolePort = digits(consolePort))} inputmode="numeric" aria-invalid={!!app.portErrors.console} />
              </label>
            </div>
            {#if app.portErrors.stream || app.portErrors.console}
              <span class="error">{app.portErrors.stream ?? app.portErrors.console}</span>
            {/if}
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  <aside class="aside">
    <span class="label-mono">Pourquoi ces points ?</span>
    <p>Chaque image est compressée par la puce <strong>NVENC</strong> de la carte graphique, sans ralentir vos jeux.</p>
    <p>Elle traverse ensuite votre réseau : un <strong>câble Ethernet</strong> évite les micro-coupures du Wi-Fi.</p>
    <p>
      Le <strong>port</strong> est tiré au hasard puis vérifié libre en TCP et en UDP, hors des plages que Windows se
      réserve. Vos appareils le trouvent seuls sur le réseau.
    </p>
  </aside>
</div>

<div class="footer">
  <button class="btn" onclick={() => go('welcome')}>Retour</button>
  <button class="btn ghost" onclick={rerunChecks} disabled={running}>Relancer l'analyse</button>
  <div class="spacer"></div>
  <button class="btn primary" disabled={running || editing} onclick={() => go('account')}>
    Continuer
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
</div>

<style>
  .body {
    margin: 16px 0 16px;
    flex: 1;
    display: flex;
    gap: 20px;
    min-height: 0;
  }

  ul {
    flex: 1;
    list-style: none;
    margin: 0;
    padding: 0 4px 0 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--line-2) transparent;
  }

  li {
    flex-shrink: 0;
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 8px 14px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--line);
    transition: opacity 0.3s, background-color 0.3s;
  }

  li.pending {
    opacity: 0.45;
  }

  li.warn {
    background: var(--warn-bg);
    border-color: var(--warn-line);
  }

  li.error {
    background: var(--danger-bg);
    border-color: #5a2420;
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .row strong {
    flex: 1;
    font-size: 14px;
  }

  .detail {
    font-size: 12px;
    line-height: 1.45;
    color: var(--muted);
  }

  .detail.mono {
    font-family: var(--mono);
  }

  li.warn .detail {
    color: var(--warn-text);
  }

  .mini {
    height: 28px;
    padding: 0 10px;
    border-radius: 7px;
    border: 1px solid var(--line-2);
    background: transparent;
    color: var(--text-2);
    font-size: 12px;
  }

  .ports {
    margin-top: 6px;
    display: flex;
    gap: 12px;
  }

  .ports label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
  }

  .ports input {
    width: 84px;
    height: 32px;
    padding: 0 10px;
    border-radius: 8px;
    border: 1px solid var(--line-2);
    background: var(--bg);
    color: var(--text);
    font-family: var(--mono);
    font-size: 13px;
  }

  .ports input[aria-invalid='true'] {
    border-color: var(--danger);
  }

  .error {
    margin-top: 4px;
    font-size: 12px;
    color: var(--danger);
  }

  aside {
    width: 250px;
    flex-shrink: 0;
    overflow-y: auto;
  }

  .footer {
    margin-top: 0;
  }
</style>
