<!-- Réglages de l'app : version, canal des mises à jour, recherche et installation. -->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Segmented from './lib/Segmented.svelte'
  import type { AppInfo, Update } from './lib/api'
  import { invoke } from './lib/api'

  let { update, progress, onclose, oninstall }: { update: Update | null; progress: number | null; onclose: () => void; oninstall: () => void } = $props()

  let info = $state<AppInfo | null>(null)
  let checking = $state(false)
  let checked = $state(false)
  let error = $state<string | null>(null)

  onMount(() => {
    invoke<AppInfo>('app_info').then((i) => (info = i))
  })

  async function run(f: () => Promise<unknown>) {
    checking = true
    error = null
    try {
      await f()
      checked = true
    } catch (e) {
      error = String(e)
    } finally {
      checking = false
    }
  }

  const check = () => run(() => invoke<Update | null>('check_update'))
  const channel = (c: 'stable' | 'beta') =>
    run(async () => {
      if (info) info.channel = c
      await invoke<Update | null>('set_channel', { channel: c })
    })
</script>

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="app-title">
    <button class="close" aria-label="Fermer" onclick={onclose}><Icon name="x" size={14} /></button>
    <h2 id="app-title">KyberNow</h2>
    <div class="mono ver">Version {info?.version ?? '…'} · Windows {info?.arch ?? ''}</div>

    <div class="line">
      <span>Versions proposées</span>
      <Segmented
        label="Canal des mises à jour"
        value={info?.channel ?? 'stable'}
        options={[
          { value: 'stable', label: 'Stables' },
          { value: 'beta', label: 'Aussi les bêtas' },
        ]}
        onchange={channel}
      />
    </div>

    {#if update}
      <div class="update">
        <div>
          <div class="eyebrow">Mise à jour</div>
          <div class="uv">KyberNow {update.version}{update.channel === 'beta' ? ' (bêta)' : ''}</div>
        </div>
        {#if progress !== null}
          <div class="bar" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100}><span style:width="{progress}%"></span></div>
        {:else}
          <button class="primary" onclick={oninstall}>Installer et relancer</button>
        {/if}
      </div>
      {#if update.notes}<pre class="notes">{update.notes}</pre>{/if}
    {:else}
      <div class="row">
        <button class="ghost" disabled={checking} onclick={check}>{checking ? 'Recherche…' : 'Rechercher une mise à jour'}</button>
        {#if checked && !error}<span class="ok">Vous avez la dernière version.</span>{/if}
      </div>
    {/if}
    {#if error}<div class="error">{error}</div>{/if}

    <p class="help">
      Chaque version est vérifiée avant de s’installer : manifeste signé par KyberNow, taille et empreinte de l’installateur. L’app cherche toute seule au lancement, puis toutes les 6 heures.
    </p>
  </div>
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    z-index: 30;
    background: rgb(5 7 10 / 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
  }

  .sheet {
    position: relative;
    width: min(620px, 100%);
    max-height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 32px 36px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
  }

  .close {
    position: absolute;
    right: 14px;
    top: 14px;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  h2 {
    margin: 0;
    font-size: 28px;
    font-weight: 640;
    font-stretch: 125%;
  }

  .ver {
    font-size: 12px;
    color: var(--muted);
    margin-top: -10px;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    font-size: 14px;
  }

  .update {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 14px;
    padding: 16px 18px;
    border: 1px solid var(--signal);
    border-radius: 14px;
    background: var(--signal-soft);
  }

  .uv {
    margin-top: 4px;
    font-size: 18px;
    font-weight: 600;
  }

  .bar {
    width: 220px;
    height: 6px;
    border-radius: 3px;
    background: var(--line-2);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--signal);
    transition: width 0.2s;
  }

  .notes {
    margin: 0;
    max-height: 200px;
    overflow-y: auto;
    padding: 12px 14px;
    border-radius: 10px;
    background: var(--ink-0);
    font-family: var(--sans);
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
    white-space: pre-wrap;
    user-select: text;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }

  .ok {
    font-size: 13px;
    color: var(--ok);
  }

  .help {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--faint);
  }
</style>
