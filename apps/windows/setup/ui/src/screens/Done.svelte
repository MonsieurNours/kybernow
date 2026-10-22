<!-- C'est prêt : ouvrir KyberNow Link, et les raccourcis à connaître pendant une session. -->
<script lang="ts">
  import { send } from '../lib/backend'
  import { t } from '../lib/i18n/index.svelte'
  import { app } from '../lib/state.svelte'

  const keys: [string, string][] = [
    ['D', 'done.keys.disconnect'],
    ['F', 'done.keys.fullscreen'],
    ['M', 'done.keys.mouse'],
  ]
</script>

<div class="done">
  <div class="text">
    <span class="lbl ok kn-enter"><span class="dot"></span>{app.existing ? t('done.updateLabel') : t('done.installLabel')}</span>
    <h1 class="title kn-enter kn-d1">{t('done.title')}</h1>
    <p class="lead kn-enter kn-d2">
      {app.existing ? t('done.updateLead') : t('done.lead')}
    </p>
    <p class="note kn-enter kn-d3">{t(app.desktop ? 'done.whereDesktop' : 'done.where')}</p>
  </div>

  <div class="panel keys kn-enter kn-d2">
    <span class="lbl">{t('done.session')}</span>
    <ul>
      {#each keys as [key, what] (key)}
        <li>
          <span class="combo mono"><kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>{t('done.shift')}</kbd><kbd class="k">{key}</kbd></span>
          <span>{t(what)}</span>
        </li>
      {/each}
    </ul>
    <span class="note">{t('done.bar')}</span>
  </div>
</div>

<div class="footer">
  <div class="spacer"></div>
  <button type="button" class="ghost" onclick={() => send({ cmd: 'close' })}>{t('done.close')}</button>
  <button type="button" class="primary lit" onclick={() => send({ cmd: 'launch' })}>{t('done.open')}</button>
</div>

<style>
  .done {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 40px;
    align-items: start;
    padding-top: 40px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .text h1 {
    font-size: 42px;
  }

  .lbl.ok {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ok-text);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
  }

  .keys {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  li {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text-2);
  }

  .combo {
    display: flex;
    gap: 4px;
  }

  kbd {
    padding: 3px 7px;
    border-radius: 6px;
    background: var(--ink-0);
    box-shadow: inset 0 0 0 1px var(--line-2, var(--line));
    font: inherit;
    font-size: 11px;
    color: var(--muted);
  }

  kbd.k {
    color: var(--text);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.6);
  }

  .keys .note {
    font-size: 12px;
  }
</style>
