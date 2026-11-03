<!--
  Colonne de gauche de l'installateur Mac (maquette) : feux de la fenêtre,
  grand symbole en filigrane dont la couture se remplit au fil des étapes,
  logo signé de KyberNow Station, les cinq étapes, licence et « À propos ».
-->
<script lang="ts">
  import Logo from '../lib/Logo.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { send } from './bridge'
  import Lights from './Lights.svelte'
  import { app, go, stepIndex, STEPS } from './state.svelte'

  const index = $derived(stepIndex())
  const finished = $derived(app.screen === 'done')
  // Couture : un cran par étape, puis la progression réelle pendant l'installation.
  const k = $derived(
    finished ? 1 : app.screen === 'install' ? 0.6 + (0.3 * app.percent) / 100 : [0.2, 0.41, 0.6, 0.6, 0.91][index],
  )
  const sx = $derived((44 + k * 292).toFixed(1))
  const sy = $derived((336 - k * 292).toFixed(1))

  function drag(event: MouseEvent) {
    if (event.button === 0 && !(event.target as HTMLElement).closest('button, a')) send({ cmd: 'drag' })
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside onmousedown={drag}>
  <Lights disabled={app.installing} />
  <svg class="filigree" viewBox="0 0 380 380" aria-hidden="true">
    <defs><filter id="mac-rail-glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="3" /></filter></defs>
    <path fill="#0E141A" d="M55 0 L333 0 A9 9 0 0 1 339 15 L15 339 A9 9 0 0 1 0 333 L0 55 A55 55 0 0 1 55 0 Z" />
    <path fill="#121922" d="M380 45 L380 325 A55 55 0 0 1 325 380 L45 380 A9 9 0 0 1 39 365 L365 39 A9 9 0 0 1 380 45 Z" />
    {#if !finished}<line x1="44" y1="336" x2="336" y2="44" stroke="rgba(227,237,240,.08)" stroke-width="3" stroke-linecap="round" />{/if}
    <line x1="44" y1="336" x2={sx} y2={sy} stroke="#9A8CFF" stroke-width="8" stroke-linecap="round" opacity={finished ? 0.8 : 0.7} filter="url(#mac-rail-glow)" />
    <line x1="44" y1="336" x2={sx} y2={sy} stroke="#F4F1FF" stroke-width="2.5" stroke-linecap="round" />
  </svg>

  <div class="brand">
    <Logo height={41} />
    <span class="version">{t('mac.rail.version', { version: __APP_VERSION__ })}</span>
  </div>

  <ol>
    {#each STEPS as step, i (step.key)}
      {@const done = i < index || finished}
      {@const current = i === index && !finished}
      <li class:done class:current aria-current={current ? 'step' : undefined}>
        <span class="num">{done ? '✓' : i + 1}</span>{t(step.key)}
      </li>
    {/each}
  </ol>

  <p class="foot">
    {t('mac.rail.license')} ·
    <button type="button" class="about" onclick={() => go(app.screen === 'about' ? app.previous : 'about')}>{app.screen === 'about' ? t('common.back') : t('rail.about')}</button>
  </p>
</aside>

<style>
  aside {
    position: relative;
    width: 300px;
    flex: none;
    background: #090d12;
    border-right: 1px solid #1e2832;
    padding: 20px 28px 24px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .filigree {
    position: absolute;
    right: -130px;
    top: -40px;
    width: 380px;
    height: 380px;
    overflow: visible;
    pointer-events: none;
  }

  .filigree line {
    transition: all 0.6s var(--ease-out);
  }

  .brand {
    position: relative;
    margin-top: 36px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .version {
    font-size: 12.5px;
    color: #65737e;
  }

  ol {
    position: relative;
    list-style: none;
    margin: auto 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
    color: #65737e;
    transition: color 0.3s;
  }

  .num {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 12px;
    font-family: var(--mono);
    border: 1px solid #2b3742;
    flex: none;
  }

  li.done {
    color: #b4c2c9;
  }

  li.done .num {
    border: 0;
    background: #141b23;
    color: #4cd68a;
    font-family: var(--sans);
  }

  li.current {
    color: #e3edf0;
    font-weight: 600;
  }

  li.current .num {
    border: 0;
    background: #6d5bff;
    color: #fff;
    box-shadow: 0 0 12px rgb(109 91 255 / 0.55);
  }

  .foot {
    position: relative;
    margin: 28px 0 0;
    font-size: 12px;
    color: #65737e;
  }

  .about {
    border: 0;
    padding: 0;
    background: none;
    color: var(--signal);
    font-size: 12px;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .about:hover {
    color: #c9c1ff;
  }
</style>
