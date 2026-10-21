<!-- Étape 3 · Les options, déjà réglées au mieux. -->
<script lang="ts">
  import Toggle from '../lib/Toggle.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { app, beforeOptions, go, startInstall } from '../lib/state.svelte'
</script>

<div class="options">
  <div class="head">
    <span class="lbl signal kn-enter">{t('options.label')}</span>
    <h1 class="title kn-enter kn-d1">{t('options.title')}</h1>
    <p class="lead kn-enter kn-d2">{t('options.lead')}</p>
  </div>

  <div class="list kn-enter kn-d2">
    <div class="opt">
      <div>
        <h3>{t('options.virtualDisplay')} <span class="tag">{t('common.recommended')}</span></h3>
        <p>{t('options.virtualDisplayDetail')}</p>
      </div>
      <Toggle bind:checked={app.options.virtualDisplay} label={t('options.virtualDisplayLabel')} />
    </div>
    <div class="opt">
      <div>
        <h3>{t('options.gamepads')}</h3>
        <p>{t('options.gamepadsDetail')}</p>
      </div>
      <Toggle bind:checked={app.options.gamepads} label={t('options.gamepads')} />
    </div>
    <div class="opt" class:off={!app.steamSpeakers}>
      <div>
        <h3>{t('options.audio')}</h3>
        <p>
          {app.steamSpeakers ? t('options.audioDetail') : t('options.audioMissing')}
        </p>
      </div>
      <Toggle bind:checked={app.options.virtualAudio} label={t('options.audio')} disabled={!app.steamSpeakers} />
    </div>
    <div class="opt">
      <div>
        <h3>{t('options.startWithWindows')}</h3>
        <p>{t('options.startWithWindowsDetail')}</p>
      </div>
      <Toggle bind:checked={app.options.startWithWindows} label={t('options.startWithWindows')} />
    </div>
    <div class="opt">
      <div>
        <h3>{t('options.maxPerformance')}</h3>
        <p>
          {app.powerNeedsTuning ? t('options.maxPerformanceDetail') : t('options.maxPerformanceAlready')}
        </p>
      </div>
      <Toggle bind:checked={app.options.maxPerformance} label={t('options.maxPerformance')} />
    </div>
  </div>
</div>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go(beforeOptions())}>{t('common.back')}</button>
  <div class="spacer"></div>
  <button type="button" class="primary" onclick={startInstall}>{t('options.install')}</button>
</div>

<style>
  .options {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 760px;
    padding-top: 12px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .head h1 {
    font-size: 30px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-bottom: 12px;
  }

  .opt {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px 22px;
    align-items: center;
    padding: 11px 18px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }

  .opt > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  h3 {
    font-size: 15px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .tag {
    padding: 2px 8px;
    border-radius: 999px;
    background: rgb(154 140 255 / 0.16);
    color: var(--signal-light);
    font-size: 11px;
    font-weight: 600;
  }

  p {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--faint);
  }

  .opt.off {
    opacity: 0.6;
  }
</style>
