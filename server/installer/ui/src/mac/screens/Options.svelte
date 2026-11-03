<!-- Étape 3 · Options : tout est réglé au mieux, modifiable ensuite dans Admin. -->
<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte'
  import { app, beforeOptions, go, startInstall } from '../state.svelte'

  type Key = 'virtualDisplay' | 'virtualAudio' | 'startWithMac' | 'keepReachable'
  const rows: Key[] = ['virtualDisplay', 'virtualAudio', 'startWithMac', 'keepReachable']
</script>

<div class="options">
  <span class="lbl-m kn-enter">{t('mac.options.label')}</span>
  <h1 class="t kn-enter kn-d1">{t('mac.options.title')}</h1>
  <p class="lead-m kn-enter kn-d2">{t('mac.options.lead')}</p>

  <div class="list kn-enter kn-d2">
    {#each rows as key (key)}
      <div class="row">
        <span>
          <span class="name">{t(`mac.options.${key}`)}</span>
          <span class="detail">{t(`mac.options.${key}Detail`)}</span>
        </span>
        <button
          type="button"
          role="switch"
          class="switch"
          class:on={app.options[key]}
          aria-checked={app.options[key]}
          aria-label={t(`mac.options.${key}`)}
          onclick={() => (app.options[key] = !app.options[key])}
        ><span></span></button>
      </div>
    {/each}
    <div class="row soon">
      <span>
        <span class="name">{t('mac.options.gamepads')}</span>
        <span class="detail">{t('mac.options.gamepadsDetail')}</span>
      </span>
      <span class="tag">{t('mac.options.soon')}</span>
    </div>
  </div>
</div>

<div class="foot-m">
  <p class="foot-note">{t('mac.options.password')}</p>
  <div class="buttons">
    <button type="button" class="btn" onclick={() => go(beforeOptions())}>{t('common.back')}</button>
    <button type="button" class="btn primary-m" onclick={startInstall}>{t('mac.options.install')}</button>
  </div>
</div>

<style>
  .options {
    display: flex;
    flex-direction: column;
  }

  .list {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
    border-top: 1px solid #1e2832;
  }

  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 20px;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px solid #1e2832;
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
    line-height: 1.45;
    color: var(--faint);
  }

  .soon {
    opacity: 0.55;
  }

  .tag {
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--faint);
    border: 1px solid #2b3742;
    border-radius: 6px;
    padding: 4px 8px;
  }

  .foot-note {
    max-width: 400px;
  }
</style>
