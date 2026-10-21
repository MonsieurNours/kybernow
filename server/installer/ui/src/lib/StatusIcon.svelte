<!-- État d'une vérification ou d'une tâche : en cours (point allumé), fait, attention, erreur, sauté, à venir. -->
<script lang="ts">
  import { t } from './i18n/index.svelte'

  let { status, size = 20 }: { status: string; size?: number } = $props()
</script>

<span class="icon" style="width: {size}px; height: {size}px">
  {#if status === 'running'}
    <svg width={size} height={size} viewBox="0 0 20 20" role="img" aria-label={t('status.running')} class="running"><circle cx="10" cy="10" r="4" fill="#F4F1FF" /></svg>
  {:else if status === 'ok' || status === 'done'}
    <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={t('status.done')} class="draw"><path d="M5 12l5 5 9-10" fill="none" stroke="#4CD68A" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" /></svg>
  {:else if status === 'warn'}
    <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={t('status.warn')}><path d="M12 3l9.5 17h-19zM12 10v4M12 17.5v.01" fill="none" stroke="#F5B841" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
  {:else if status === 'error' || status === 'failed'}
    <svg width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={t('status.error')}><path d="M6 6l12 12M18 6L6 18" fill="none" stroke="#FF6B6B" stroke-width="2.2" stroke-linecap="round" /></svg>
  {:else if status === 'skipped'}
    <svg width={size} height={size} viewBox="0 0 20 20" role="img" aria-label={t('status.skipped')}><path d="M6 10h8" stroke="#65737E" stroke-width="1.6" stroke-linecap="round" /></svg>
  {:else if status === 'info'}
    <svg width={size} height={size} viewBox="0 0 20 20" role="img" aria-label={t('status.info')}><circle cx="10" cy="10" r="8" fill="none" stroke="#9A8CFF" stroke-width="1.6" /><path d="M10 9v5M10 6.5v.01" stroke="#D4CEFF" stroke-width="1.8" stroke-linecap="round" /></svg>
  {:else}
    <svg width={size} height={size} viewBox="0 0 20 20" role="img" aria-label={t('status.pending')}><circle cx="10" cy="10" r="4" fill="none" stroke="#8A99A3" stroke-width="1.5" /></svg>
  {/if}
</span>

<style>
  .icon {
    flex: none;
    display: inline-grid;
    place-items: center;
  }

  .running {
    filter: drop-shadow(0 0 1px #f4f1ff) drop-shadow(0 0 5px #9a8cff) drop-shadow(0 0 12px rgb(109 91 255 / 0.8));
    animation: respire 1.6s ease-in-out infinite;
  }

  .draw path {
    stroke-dasharray: 24;
    animation: kn-draw 0.35s ease-out both;
  }
</style>
