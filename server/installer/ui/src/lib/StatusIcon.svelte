<script lang="ts">
  let { status, size = 24 }: { status: string; size?: number } = $props()
</script>

{#if status === 'running'}
  <span class="ring kn-spin" style="width: {size}px; height: {size}px"></span>
{:else if status === 'ok' || status === 'done'}
  <span class="badge ok kn-pop" style="width: {size}px; height: {size}px">
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6.2l2.3 2.3 4.7-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </span>
{:else if status === 'warn'}
  <span class="badge warn kn-pop" style="width: {size}px; height: {size}px">
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M6 2.5v4.2M6 9.2v.3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg>
  </span>
{:else if status === 'error' || status === 'failed'}
  <span class="badge error kn-pop" style="width: {size}px; height: {size}px">
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg>
  </span>
{:else if status === 'skipped'}
  <span class="badge skipped" style="width: {size}px; height: {size}px">
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 6h6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg>
  </span>
{:else}
  <span class="pending" style="width: {size}px; height: {size}px"></span>
{/if}

<style>
  span {
    flex-shrink: 0;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .ring {
    border: 2px solid #263040;
    border-top-color: var(--accent);
  }

  .pending {
    border: 1px dashed #3c4a5e;
  }

  .ok {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .warn {
    background: #3a2e17;
    color: var(--warn);
  }

  .error {
    background: var(--danger-bg);
    color: var(--danger);
  }

  .skipped {
    background: #1a212b;
    color: var(--faint);
  }
</style>
