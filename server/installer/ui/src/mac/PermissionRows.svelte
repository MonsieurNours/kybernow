<!--
  Les trois accords de macOS pour KyberNow Station, relus toutes les
  1,5 s : une ligne se coche toute seule dès que l'accord est donné. La
  première ligne qui manque est mise en avant.
-->
<script lang="ts">
  import { onDestroy } from 'svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { send, type Which } from './bridge'
  import { app } from './state.svelte'

  const rows: Which[] = ['screen', 'accessibility', 'microphone']

  send({ cmd: 'watchPermissions', on: true })
  onDestroy(() => send({ cmd: 'watchPermissions', on: false }))

  const granted = (w: Which) => {
    const p = app.permissions
    if (!p) return false
    return w === 'microphone' ? p.microphone === 'granted' : p[w]
  }
  const next = $derived(rows.find((w) => !granted(w)))

  function action(w: Which): string {
    if (w === 'microphone' && app.permissions?.microphone !== 'denied') return t('mac.permissions.allow')
    return t('mac.permissions.open')
  }
</script>

<div class="rows">
  {#each rows as w (w)}
    {@const ok = granted(w)}
    <div class="card-m row" class:next={w === next}>
      <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke={w === next ? '#E3EDF0' : '#B4C2C9'} stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        {#if w === 'screen'}<rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" />
        {:else if w === 'accessibility'}<path d="M5 4l12 7-5 1.5L9.5 18z" /><path d="M13 13l5 5" />
        {:else}<rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" />{/if}
      </svg>
      <span>
        <span class="name">{t(`mac.permissions.${w}`)}</span>
        <span class="detail">{t(`mac.permissions.${w}Detail`)}</span>
      </span>
      {#if !app.permissions}
        <span class="wait">…</span>
      {:else if ok}
        <span class="ok"><span aria-hidden="true">✓</span>{t('mac.permissions.granted')}</span>
      {:else}
        <button type="button" class="btn small" class:primary-m={w === next} onclick={() => send({ cmd: 'requestPermission', which: w })}>{action(w)}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .rows {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 16px;
    align-items: center;
    padding: 14px 18px;
    transition: border-color 0.2s, box-shadow 0.2s;
  }

  .row.next {
    border-color: var(--signal);
    box-shadow: 0 0 0 3px rgb(154 140 255 / 0.14);
  }

  .name {
    display: block;
    font-size: 14.5px;
    font-weight: 600;
  }

  .detail {
    display: block;
    margin-top: 3px;
    font-size: 12.5px;
    color: var(--faint);
  }

  .ok {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--ok);
  }

  .wait {
    color: var(--faint);
  }
</style>
