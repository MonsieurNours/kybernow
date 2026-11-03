<!-- État d'un contrôle ou d'une tâche, en signe typographique comme sur la maquette : ✓, !, i, ✕, ●, ○, –. -->
<script lang="ts">
  import { t } from '../lib/i18n/index.svelte'

  let { status }: { status: string } = $props()

  const look = $derived(
    {
      ok: ['✓', '#4CD68A', 'status.done'],
      done: ['✓', '#4CD68A', 'status.done'],
      warn: ['!', '#F5B841', 'status.warn'],
      info: ['i', '#8A99A3', 'status.info'],
      error: ['✕', '#FF6B6B', 'status.error'],
      failed: ['✕', '#FF6B6B', 'status.error'],
      running: ['●', '#9A8CFF', 'status.running'],
      skipped: ['–', '#65737E', 'status.skipped'],
    }[status] ?? ['○', '#3A434D', 'status.pending'],
  )
</script>

<span class="g" class:running={status === 'running'} style:color={look[1]} role="img" aria-label={t(look[2])}>{look[0]}</span>

<style>
  .g {
    display: inline-block;
    width: 22px;
    font-size: 13.5px;
    line-height: 1.45;
  }

  .running {
    animation: respire 1.6s ease-in-out infinite;
    text-shadow: 0 0 8px rgb(109 91 255 / 0.9);
  }

  @media (prefers-reduced-motion: reduce) {
    .running {
      animation: none;
    }
  }
</style>
