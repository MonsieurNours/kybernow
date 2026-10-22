<!-- À propos : qui fait KyberNow Station, et les logiciels libres qu'elle embarque. -->
<script lang="ts">
  import { send } from '../lib/backend'
  import { t } from '../lib/i18n/index.svelte'
  import { app, go } from '../lib/state.svelte'
  import Logo from '../lib/Logo.svelte'

  // [composant, clé du rôle (about.roles.*), licence]
  const licenses = $derived([
    ['KyberNow', 'kybernow', 'AGPL-3.0'],
    ['Kyber 0.29', 'kyber', 'AGPL-3.0'],
    ['VLC / libVLC', 'vlc', 'LGPL-2.1+'],
    ['FFmpeg', 'ffmpeg', 'LGPL-2.1+ / GPL-2.0+'],
    ['x264', 'x264', 'GPL-2.0+'],
    ['Opus', 'opus', 'BSD-3-Clause'],
    ['SDL2', 'sdl', 'Zlib'],
    ['SudoVDA', 'sudovda', 'MIT / CC0'],
    ['ViGEmBus', 'vigembus', 'BSD-3-Clause'],
    ['nefcon', 'nefcon', 'MIT'],
    ['Archivo, Martian Mono', 'fonts', 'OFL-1.1'],
    [t('about.rustLibraries'), 'rust', 'MIT / Apache-2.0'],
  ])
</script>

<div class="about">
  <div class="head kn-enter">
    <Logo height={56} />
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
          {#each licenses as [name, role, spdx] (role)}
            <tr><td>{name}</td><td class="role">{t(`about.roles.${role}`)}</td><td class="mono spdx">{spdx}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">{t('about.note')}</p>
  </div>
</div>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go(app.previous)}>{t('common.back')}</button>
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
    align-items: flex-end;
    gap: 20px;
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
