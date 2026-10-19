<script lang="ts">
  import { app, go, STEPS, stepIndex } from './state.svelte'

  const current = $derived(app.screen === 'done' ? STEPS.length : stepIndex())
</script>

<nav aria-label="Étapes de l'installation">
  <div class="brand">
    <span class="name">Kyber<span>Now</span></span>
    <span class="tag">Serveur de streaming pour Windows</span>
  </div>

  <ol>
    {#each STEPS as step, i (step.screen)}
      {@const done = i < current || (app.screen === 'done' && i === STEPS.length - 1)}
      {@const isCurrent = i === current && app.screen !== 'done'}
      <li class:current={isCurrent || (app.screen === 'done' && i === STEPS.length - 1)}>
        {#if done}
          {#key current}
            <span class="dot done kn-pop" style="animation-delay: {app.screen === 'done' ? i * 0.08 : 0}s">
              <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6.2l2.3 2.3 4.7-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
            </span>
          {/key}
        {:else if isCurrent}
          <span class="dot now kn-ring">{i + 1}</span>
        {:else}
          <span class="dot todo">{i + 1}</span>
        {/if}
        <span class="label">{step.label}</span>
      </li>
    {/each}
  </ol>

  <div class="spacer"></div>
  <div class="about">
    <p>Propulsé par Kyber 0.29<br />Logiciel libre (AGPL-3.0)</p>
    {#if app.screen === 'about'}
      <button onclick={() => go(app.previous)}>
        <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M13 8H3M7 4L3 8l4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
        Retour
      </button>
    {:else}
      <button onclick={() => go('about')}>
        <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><circle cx="7" cy="7" r="5.8" fill="none" stroke="currentColor" stroke-width="1.3" /><path d="M7 6.2v3.6M7 4.2v.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" /></svg>
        À propos
      </button>
    {/if}
  </div>
</nav>

<style>
  nav {
    width: 264px;
    flex-shrink: 0;
    padding: 32px 24px 16px;
    background: var(--rail);
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .brand {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    font-family: var(--display);
    font-weight: 700;
    font-size: 24px;
    letter-spacing: -0.02em;
  }

  .name span {
    color: var(--accent);
  }

  .tag {
    font-size: 13px;
    color: var(--muted);
  }

  ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    transition: background-color 0.3s;
  }

  li.current {
    background: var(--surface-2);
  }

  .dot {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--mono);
    font-size: 12px;
    flex-shrink: 0;
  }

  .dot.done {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .dot.now {
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 500;
  }

  .dot.todo {
    border: 1px solid #344052;
    color: var(--muted);
  }

  .label {
    font-size: 14px;
    color: var(--muted);
  }

  li.current .label {
    color: var(--text);
    font-weight: 600;
  }

  li:has(.done) .label {
    color: #b4c0cf;
  }

  .about p {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--faint);
  }

  .about button {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: none;
    background: none;
    font-size: 12px;
    color: var(--muted);
  }

  .about button:hover {
    color: var(--text);
  }
</style>
