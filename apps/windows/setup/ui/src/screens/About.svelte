<!-- À propos : qui fait KyberNow Link, et les logiciels libres qu'il embarque. -->
<script lang="ts">
  import { send } from '../lib/backend'
  import { app, go } from '../lib/state.svelte'
  import Mark from '../lib/Mark.svelte'
  import { t } from '../lib/i18n/index.svelte'

  const licenses = $derived([
    ['KyberNow Link', t('about.roles.kybernow'), 'AGPL-3.0'],
    ['Kyber 0.29', t('about.roles.kyber'), 'AGPL-3.0'],
    ['FFmpeg', t('about.roles.ffmpeg'), 'LGPL-2.1+'],
    ['Opus', t('about.roles.opus'), 'BSD-3-Clause'],
    ['SDL2', t('about.roles.sdl'), 'Zlib'],
    ['Tauri, wry, WebView2', t('about.roles.tauri'), 'MIT / Apache-2.0'],
    [t('about.names.svelte'), t('about.roles.svelte'), 'MIT / Apache-2.0'],
    ['Archivo, Martian Mono', t('about.roles.fonts'), 'OFL-1.1'],
  ])
</script>

<div class="about">
  <div class="head kn-enter">
    <Mark product="link" size={48} />
    <span class="mono version">{t('about.version', { version: __APP_VERSION__ })}</span>
  </div>

  <div class="panel dev kn-enter kn-d1">
    <div><span class="lbl">{t('about.madeBy')}</span><strong>Les Lignes Claires</strong></div>
    <button type="button" class="ghost" onclick={() => send({ cmd: 'openUrl', url: 'https://www.leslignesclaires.fr' })}>www.leslignesclaires.fr</button>
  </div>

  <div class="panel list kn-enter kn-d2">
    <span class="lbl">{t('about.included')}</span>
    <div class="scroll">
      <table>
        <thead><tr><th scope="col">{t('about.component')}</th><th scope="col">{t('about.role')}</th><th scope="col">{t('about.license')}</th></tr></thead>
        <tbody>
          {#each licenses as [name, role, spdx] (name)}
            <tr><td>{name}</td><td class="role">{role}</td><td class="mono spdx">{spdx}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">{t('about.note')}</p>
  </div>
</div>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go(app.previous)}>{t('about.back')}</button>
  <div class="spacer"></div>
</div>

<style>
  .about {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 0 16px;
  }

  .head {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }

  .version {
    font-size: 12px;
    color: var(--faint);
  }

  .dev {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .dev > div {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .dev strong {
    font-size: 17px;
  }

  .list {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    scrollbar-width: thin;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th {
    text-align: left;
    padding: 6px 0;
    font-weight: 500;
    color: var(--faint);
  }

  td {
    padding: 7px 12px 7px 0;
    border-top: 1px solid var(--line);
  }

  .role {
    color: var(--muted);
  }

  .spdx {
    font-size: 11px;
    color: var(--text-2);
    white-space: nowrap;
  }
</style>
