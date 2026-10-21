<!-- À propos : qui fait KyberNow, et les logiciels libres qu'il embarque. -->
<script lang="ts">
  import { send } from '../lib/backend'
  import { app, go } from '../lib/state.svelte'
  import Mark from '../lib/Mark.svelte'

  const licenses = [
    ['KyberNow', 'L’app et son installateur', 'AGPL-3.0'],
    ['Kyber 0.29', 'Connexion, son et entrées (Kyber SAS)', 'AGPL-3.0'],
    ['FFmpeg', 'Décodage vidéo', 'LGPL-2.1+'],
    ['Opus', 'Décodage audio', 'BSD-3-Clause'],
    ['SDL2', 'Manettes', 'Zlib'],
    ['Tauri, wry, WebView2', 'Fenêtres et interface', 'MIT / Apache-2.0'],
    ['Svelte, bibliothèques Rust', 'Interface et cœur', 'MIT / Apache-2.0'],
    ['Archivo, Martian Mono', 'Polices de l’interface', 'OFL-1.1'],
  ]
</script>

<div class="about">
  <div class="head kn-enter">
    <Mark size={48} state="now" />
    <div>
      <span class="disp name"><span>Kyber</span><b>Now</b></span>
      <span class="mono version">version {__APP_VERSION__}</span>
    </div>
  </div>

  <div class="panel dev kn-enter kn-d1">
    <div><span class="lbl">Conçu et développé par</span><strong>Les Lignes Claires</strong></div>
    <button type="button" class="ghost" onclick={() => send({ cmd: 'openUrl', url: 'https://www.leslignesclaires.fr' })}>www.leslignesclaires.fr</button>
  </div>

  <div class="panel list kn-enter kn-d2">
    <span class="lbl">Logiciels libres inclus</span>
    <div class="scroll">
      <table>
        <thead><tr><th scope="col">Composant</th><th scope="col">Rôle</th><th scope="col">Licence</th></tr></thead>
        <tbody>
          {#each licenses as [name, role, spdx] (name)}
            <tr><td>{name}</td><td class="role">{role}</td><td class="mono spdx">{spdx}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">KyberNow est distribué sous AGPL-3.0. Les textes des licences sont dans le dépôt du projet, sur GitHub.</p>
  </div>
</div>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go(app.previous)}>Retour</button>
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
    align-items: center;
    gap: 16px;
  }

  .head > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .name {
    font-size: 26px;
    letter-spacing: -0.02em;
  }

  .name span {
    font-weight: 420;
  }

  .name b {
    font-weight: 720;
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
