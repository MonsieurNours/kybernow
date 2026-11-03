<!-- À propos : qui fait KyberNow Station, et les logiciels libres qu'elle embarque sur Mac. -->
<script lang="ts">
  import Logo from '../../lib/Logo.svelte'
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import { app, go } from '../state.svelte'

  const licenses = $derived([
    ['KyberNow', 'kybernow', 'AGPL-3.0'],
    ['Kyber 0.29', 'kyber', 'AGPL-3.0'],
    ['FFmpeg', 'ffmpeg', 'LGPL-2.1+ / GPL-2.0+'],
    ['Opus', 'opus', 'BSD-3-Clause'],
    ['SDL2', 'sdl', 'Zlib'],
    ['Archivo, Martian Mono', 'fonts', 'OFL-1.1'],
    [t('about.rustLibraries'), 'rust', 'MIT / Apache-2.0'],
  ])
</script>

<div class="about">
  <div class="head kn-enter">
    <Logo height={48} />
    <span class="mono-m version">{t('mac.rail.version', { version: __APP_VERSION__ })}</span>
  </div>

  <div class="card-m dev kn-enter kn-d1">
    <div><span class="lbl-s">{t('about.madeBy')}</span><strong>Les Lignes Claires</strong></div>
    <button type="button" class="btn small" onclick={() => send({ cmd: 'openUrl', url: 'https://www.leslignesclaires.fr' })}>www.leslignesclaires.fr</button>
  </div>

  <div class="card-m list kn-enter kn-d2">
    <span class="lbl-s">{t('about.included')}</span>
    <table>
      <thead><tr><th scope="col">{t('about.component')}</th><th scope="col">{t('about.role')}</th><th scope="col">{t('about.license')}</th></tr></thead>
      <tbody>
        {#each licenses as [name, role, spdx] (role)}
          <tr><td>{name}</td><td class="role">{t(`about.roles.${role}`)}</td><td class="mono-m spdx">{spdx}</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<div class="foot-m">
  <button type="button" class="btn" onclick={() => go(app.previous)}>{t('common.back')}</button>
</div>

<style>
  .about {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 0;
    flex: 1;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .version {
    font-size: 11.5px;
    color: var(--faint);
  }

  .dev {
    padding: 14px 18px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .dev div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .list {
    padding: 14px 18px;
    min-height: 0;
    overflow: auto;
  }

  table {
    width: 100%;
    margin-top: 8px;
    border-collapse: collapse;
    font-size: 12.5px;
  }

  th {
    text-align: left;
    font-weight: 500;
    color: var(--faint);
    padding: 6px 8px 6px 0;
  }

  td {
    padding: 6px 8px 6px 0;
    border-top: 1px solid #1e2832;
  }

  .role {
    color: var(--muted);
  }

  .spdx {
    font-size: 11px;
    color: var(--faint);
  }

  .foot-m {
    padding-top: 16px;
  }
</style>
