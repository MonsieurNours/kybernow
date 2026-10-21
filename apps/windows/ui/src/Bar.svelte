<!--
  Barre de session : fenêtre transparente au-dessus de la session, montrée
  quand le pointeur touche le haut de l'écran, cachée quand il la quitte (le
  clavier revient alors à la session). Mesures en direct, souris, plein
  écran, fin de la session.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import type { Stats } from './lib/api'
  import { invoke, listen, ms } from './lib/api'

  let stats = $state<Stats | null>(null)
  let label = $state('')
  let history = $state<number[]>([])
  let hideTimer: ReturnType<typeof setTimeout> | null = null

  onMount(() => {
    const off = [
      listen<Stats>('stats', (s) => {
        stats = s
        if (s.latency_ms != null) history = [...history, s.latency_ms].slice(-40)
      }),
      listen<string>('bar-shown', (l) => {
        label = l
        cancelHide()
      }),
    ]
    return () => off.forEach((p) => p.then((f) => f()))
  })

  const spark = $derived.by(() => {
    if (history.length < 2) return ''
    const max = Math.max(20, ...history)
    return history.map((v, i) => `${(i / (history.length - 1)) * 70},${18 - (v / max) * 16}`).join(' ')
  })

  function cancelHide() {
    if (hideTimer) clearTimeout(hideTimer)
    hideTimer = null
  }

  function scheduleHide() {
    cancelHide()
    hideTimer = setTimeout(() => invoke('bar_hide'), 700)
  }

  const command = (c: string) => invoke('session_command', { command: c })
</script>

<div class="bar" role="toolbar" aria-label="Barre de session" tabindex="-1" onmouseleave={scheduleHide} onmouseenter={cancelHide}>
  <Mark size={20} state="now" lit />
  <div class="lat">
    <span class="mono">{ms(stats?.latency_ms)} ms</span>
    <svg width="70" height="20" viewBox="0 0 70 20" aria-hidden="true"><polyline points={spark} fill="none" stroke="#9A8CFF" stroke-width="1.5" /></svg>
  </div>
  <span class="sep"></span>
  <span class="mono info">{stats ? `${stats.width} × ${stats.height} · ${stats.fps} i/s · ${stats.decoder}` : '—'}</span>
  <span class="sep"></span>
  {#if label}<span class="label">{label}</span>{/if}
  <button class="icon" class:on={stats?.relative} aria-label={stats?.relative ? 'Souris des jeux (Ctrl+Alt+Maj+M)' : 'Souris du bureau (Ctrl+Alt+Maj+M)'} title={stats?.relative ? 'Souris des jeux' : 'Souris du bureau'} onclick={() => command('mouse')}>
    <Icon name="mouse" size={20} stroke={1.6} />
  </button>
  <button class="icon" aria-label="Plein écran ou fenêtre (Ctrl+Alt+Maj+F)" title="Plein écran ou fenêtre" onclick={() => command('fullscreen')}>
    <Icon name="fullscreen" size={20} stroke={1.6} />
  </button>
  <span class="sep"></span>
  <button class="quit" onclick={() => command('disconnect')}>Quitter</button>
</div>

<style>
  :global(html.bar #app) {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 6px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 56px;
    padding: 0 8px 0 16px;
    border: 1px solid rgb(43 55 66 / 0.9);
    border-radius: 14px;
    background: rgb(9 13 18 / 0.9);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.5);
    white-space: nowrap;
    animation: drop 0.25s var(--ease-out);
  }

  @keyframes drop {
    from {
      transform: translateY(-10px);
      opacity: 0;
    }
  }

  .lat {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px;
    font-size: 15px;
  }

  .sep {
    width: 1px;
    height: 24px;
    background: var(--line-2);
  }

  .info {
    padding: 0 8px;
    font-size: 12px;
    color: var(--muted);
  }

  .label {
    padding: 0 10px;
    height: 34px;
    display: flex;
    align-items: center;
    border: 1px solid var(--line-2);
    border-radius: 8px;
    font-size: 13px;
  }

  .icon {
    width: 44px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon:hover {
    background: rgb(255 255 255 / 0.06);
    color: var(--text);
  }

  .icon.on {
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--signal);
    color: var(--text);
  }

  .quit {
    height: 40px;
    padding: 0 14px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: #ff8a8a;
    font-size: 13px;
  }

  .quit:hover {
    background: rgb(255 107 107 / 0.12);
  }
</style>
