<!--
  Colonne de gauche : le symbole « now » de l'app, et les trois étapes. Le
  symbole immense, en filigrane, se remplit de néon le long de la couture au
  fil des étapes (et de la progression pendant l'installation).
-->
<script lang="ts">
  import { send } from './backend'
  import { app, go, stepIndex, STEPS } from './state.svelte'
  import Mark from './Mark.svelte'

  const index = $derived(stepIndex())
  const finished = $derived(app.screen === 'done')
  // Avancée de la couture : par étape, puis la progression réelle de l'installation.
  const k = $derived(finished ? 1 : app.screen === 'install' ? 0.3 + (0.6 * app.percent) / 100 : [0.2, 0.3, 1][index])
  const sx = $derived((44 + k * 292).toFixed(1))
  const sy = $derived((336 - k * 292).toFixed(1))

  function drag(event: MouseEvent) {
    if (event.button === 0 && !(event.target as HTMLElement).closest('button')) send({ cmd: 'drag' })
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside onmousedown={drag}>
  <div class="ghost-mark" aria-hidden="true">
    <svg viewBox="0 0 48 48" class="halves">
      <path fill="#0E141A" d="M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z" />
      <path fill="#141B26" d="M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z" />
    </svg>
    <svg viewBox="0 0 380 380" class="seam">
      <defs><filter id="rail-g" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4" /></filter></defs>
      <line x1="44" y1="336" x2="336" y2="44" stroke="rgba(227,237,240,.08)" stroke-width="3" stroke-linecap="round" />
      <line x1="44" y1="336" x2={sx} y2={sy} stroke="#9A8CFF" stroke-width="8" stroke-linecap="round" filter="url(#rail-g)" />
      <line x1="44" y1="336" x2={sx} y2={sy} stroke="#F4F1FF" stroke-width="2.5" stroke-linecap="round" />
    </svg>
  </div>

  <div class="brand">
    <Mark size={38} state="now" />
    <div>
      <span class="disp name"><span>Kyber</span><b>Now</b></span>
      <span class="version">Version {__APP_VERSION__}{app.checksDone ? ` · ${app.arch === 'arm64' ? 'ARM64' : 'x64'}` : ''}</span>
    </div>
  </div>

  <ol>
    {#each STEPS as step, i (step.label)}
      {@const done = i < index || (finished && i === index)}
      {@const current = i === index && !finished}
      <li class:done class:current aria-current={current ? 'step' : undefined}>
        <span class="num">{done ? '✓' : i + 1}</span>
        <span>{step.label}</span>
      </li>
    {/each}
  </ol>

  <div class="foot">
    Logiciel libre (AGPL-3.0) ·
    <button type="button" class="text-button" onclick={() => go(app.screen === 'about' ? app.previous : 'about')}>{app.screen === 'about' ? 'Retour' : 'À propos'}</button>
  </div>
</aside>

<style>
  aside {
    position: relative;
    overflow: hidden;
    width: 280px;
    flex: none;
    background: var(--ink-1);
    border-right: 1px solid var(--line);
    padding: 48px 32px 28px;
    display: flex;
    flex-direction: column;
    gap: 36px;
  }

  .ghost-mark {
    position: absolute;
    right: -120px;
    top: -60px;
    width: 380px;
    height: 380px;
    pointer-events: none;
  }

  .ghost-mark svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .seam line {
    transition: all 0.6s var(--ease-out);
  }

  .brand {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .brand > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    font-size: 20px;
    letter-spacing: -0.01em;
  }

  .name span {
    font-weight: 420;
  }

  .name b {
    font-weight: 720;
  }

  .version {
    font-size: 12.5px;
    color: var(--faint);
  }

  ol {
    position: relative;
    list-style: none;
    margin: 170px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  li {
    display: grid;
    grid-template-columns: 26px 1fr;
    gap: 12px;
    align-items: center;
    font-size: 14px;
    color: var(--ghost);
  }

  .num {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 12px;
    font-weight: 600;
    box-shadow: inset 0 0 0 1px #33404b;
  }

  li.done {
    color: var(--text);
  }

  li.done .num {
    background: rgb(76 214 138 / 0.16);
    box-shadow: inset 0 0 0 1px rgb(76 214 138 / 0.5);
    color: var(--ok-text);
  }

  li.current {
    color: var(--text);
    font-weight: 650;
  }

  li.current .num {
    background: var(--signal-strong);
    box-shadow: 0 0 0 1px var(--neon-core), 0 0 12px rgb(109 91 255 / 0.8);
    color: #fff;
  }

  .foot {
    position: relative;
    margin-top: auto;
    font-size: 12px;
    color: var(--ghost);
  }

  .foot .text-button {
    font-size: 12px;
    color: var(--faint);
  }
</style>
