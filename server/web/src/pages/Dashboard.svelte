<script lang="ts">
  import { api, type ServiceState } from '../lib/api'
  import { app, refreshStatus, notify, errorText } from '../lib/store.svelte'

  const stateLabels: Record<ServiceState, [string, string]> = {
    running: ['En marche', 'ok'],
    stopped: ['Arrêté', 'danger'],
    starting: ['Démarrage…', 'warn'],
    stopping: ['Arrêt…', 'warn'],
    not_installed: ['Non installé', 'danger'],
    unknown: ['Inconnu', 'warn'],
  }

  let busy = $state(false)
  const s = $derived(app.status)
  const clientUrl = $derived(s?.lan_ip ? `https://${s.lan_ip}:${s.controller.port}/webclient/` : null)

  async function act(action: 'start' | 'stop' | 'restart') {
    busy = true
    try {
      await api.service(action)
      await refreshStatus()
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  // États de performance NVIDIA : P0 = puissance maximale, P8 et au-delà = repos.
  function pstateInfo(p: number) {
    if (p <= 1) return { label: 'puissance maximale', hint: 'idéal pendant un stream', tone: 'ok' }
    if (p <= 4) return { label: 'performances élevées', hint: 'convient au stream', tone: 'ok' }
    if (p <= 7) return { label: 'puissance réduite', hint: "le GPU économise de l'énergie", tone: 'warn' }
    return { label: 'au repos', hint: 'normal sans stream ; sinon, vérifiez le mode d’alimentation', tone: 'idle' }
  }

  const fmt = (v: number | undefined, unit = '') => (v === undefined || v === null ? '—' : `${v}${unit}`)
</script>

<header>
  <h1>Tableau de bord</h1>
  <p class="muted">État du serveur Kyber, actualisé toutes les 3 secondes.</p>
</header>

{#if !s}
  <p class="muted">Chargement…</p>
{:else}
  {#if !s.config_found}
    <div class="notice danger">
      Configuration introuvable : <code>{s.config_path}</code>. Kyber est-il installé dans <code>{s.kyber_dir}</code> ?
    </div>
  {/if}

  <div class="grid">
    <section class="card">
      <div class="row">
        <h2>Service Kyber</h2>
        <span class="spacer"></span>
        <span class="badge {stateLabels[s.service.state][1]}">{stateLabels[s.service.state][0]}</span>
      </div>
      {#if s.service.simulated}
        <p class="muted small">Service simulé (développement hors Windows).</p>
      {/if}
      <div class="row actions">
        {#if s.service.state === 'running'}
          <button disabled={busy} onclick={() => act('restart')}>Redémarrer</button>
          <button class="danger" disabled={busy} onclick={() => act('stop')}>Arrêter</button>
        {:else}
          <button class="primary" disabled={busy || s.service.state === 'not_installed'} onclick={() => act('start')}>
            Démarrer
          </button>
        {/if}
      </div>
    </section>

    <section class="card">
      <div class="row">
        <h2>Connexions</h2>
        <span class="spacer"></span>
        <span class="badge {s.controller.reachable ? 'ok' : 'danger'}">
          {s.controller.reachable ? 'Port ouvert' : 'Ne répond pas'}
        </span>
      </div>
      <dl class="kv">
        <dt>Machine</dt>
        <dd>{s.hostname ?? '—'}</dd>
        <dt>Système</dt>
        <dd>{s.os ?? '—'}</dd>
        <dt>Adresse</dt>
        <dd>{s.lan_ip ?? '—'}</dd>
        <dt>Port</dt>
        <dd>{s.controller.port} (TCP + UDP)</dd>
        <dt>Client web</dt>
        <dd>
          {#if clientUrl}<a href={clientUrl} target="_blank" rel="noreferrer">{clientUrl}</a>{:else}—{/if}
        </dd>
      </dl>
    </section>
  </div>

  {#if s.gpu}
    {#each s.gpu.devices as gpu, i (i)}
      <section class="card">
        <div class="row">
          <h2>{gpu.name}</h2>
          <span class="spacer"></span>
          {#if gpu.encoder_sessions}
            <span class="badge ok">{gpu.encoder_sessions} flux en cours</span>
          {:else}
            <span class="badge">Aucun flux</span>
          {/if}
        </div>
        <div class="meters">
          <div class="meter">
            <span class="muted">GPU</span>
            <strong>{fmt(gpu.gpu_utilization, ' %')}</strong>
          </div>
          <div class="meter">
            <span class="muted">Encodeur NVENC</span>
            <strong>{fmt(gpu.encoder_utilization, ' %')}</strong>
          </div>
          <div class="meter">
            <span class="muted">Images encodées</span>
            <strong>{gpu.encoder_sessions ? fmt(gpu.encoder_fps, ' i/s') : '—'}</strong>
          </div>
          <div class="meter">
            <span class="muted">Latence d'encodage</span>
            <strong>
              {gpu.encoder_sessions && gpu.encoder_latency_us !== undefined
                ? `${(gpu.encoder_latency_us / 1000).toFixed(1)} ms`
                : '—'}
            </strong>
          </div>
          <div class="meter">
            <span class="muted">Température</span>
            <strong>{fmt(gpu.temperature_c, ' °C')}</strong>
          </div>
          <div class="meter">
            <span class="muted">Consommation</span>
            <strong>{gpu.power_w !== undefined ? `${Math.round(gpu.power_w)} W` : '—'}</strong>
          </div>
        </div>
        <dl class="kv">
          <dt>Pilote</dt>
          <dd>{s.gpu.driver ?? '—'}</dd>
          <dt>Mémoire</dt>
          <dd>
            {gpu.memory_used_mb !== undefined && gpu.memory_total_mb !== undefined
              ? `${(gpu.memory_used_mb / 1024).toFixed(1)} / ${(gpu.memory_total_mb / 1024).toFixed(1)} Go`
              : '—'}
          </dd>
          <dt>État d'alimentation</dt>
          <dd>
            {#if gpu.pstate !== undefined && gpu.pstate !== null}
              {@const p = pstateInfo(gpu.pstate)}
              <span class="pstate {p.tone}">P{gpu.pstate}</span> {p.label}
              <span class="muted small-inline">· {p.hint}</span>
            {:else}—{/if}
          </dd>
        </dl>
      </section>
    {/each}
  {:else}
    <section class="card">
      <h2>GPU</h2>
      <p class="muted">Aucun GPU NVIDIA détecté (pilote NVML indisponible).</p>
    </section>
  {/if}
{/if}

<style>
  .actions {
    margin-top: 14px;
  }

  .small {
    font-size: 0.85rem;
    margin-top: 6px;
  }

  .meters {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 10px;
    margin-top: 14px;
  }

  .meter {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    background: var(--surface-2);
    border-radius: 8px;
    font-size: 0.82rem;
  }

  .meter strong {
    font-size: 1.15rem;
    font-variant-numeric: tabular-nums;
  }

  dd a {
    word-break: break-all;
  }

  .pstate {
    font-family: var(--mono);
    font-size: 0.82rem;
    font-weight: 500;
    padding: 1px 7px;
    border-radius: 6px;
    margin-right: 4px;
    background: var(--surface-2);
  }

  .pstate.ok {
    background: var(--ok-bg);
    color: var(--ok);
  }

  .pstate.warn {
    background: #3a2e17;
    color: var(--warn);
  }

  .small-inline {
    font-size: 0.84rem;
  }
</style>
