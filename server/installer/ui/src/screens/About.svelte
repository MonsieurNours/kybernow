<script lang="ts">
  import { send } from '../lib/backend'
  import Mark from '../lib/Mark.svelte'

  const licenses = [
    ['KyberNow', "Installateur et console d'administration", 'AGPL-3.0'],
    ['Kyber 0.29', 'Moteur de streaming (Kyber SAS)', 'AGPL-3.0'],
    ['VLC / libVLC', 'Lecture audio et vidéo (VideoLAN)', 'LGPL-2.1+'],
    ['FFmpeg', 'Capture et encodage', 'LGPL-2.1+ / GPL-2.0+'],
    ['x264', 'Encodage logiciel H.264', 'GPL-2.0+'],
    ['Opus', 'Compression audio', 'BSD-3-Clause'],
    ['SDL2', 'Manettes', 'Zlib'],
    ['SudoVDA', "Pilote d'écran virtuel", 'MIT / CC0'],
    ['ViGEmBus', 'Pilote de manettes virtuelles', 'BSD-3-Clause'],
    ['nefcon', 'Installation de pilotes', 'MIT'],
    ['Bricolage Grotesque, IBM Plex', "Polices de l'interface", 'OFL-1.1'],
    ['Svelte, wry, bibliothèques Rust', 'Interface et installateur', 'MIT / Apache-2.0'],
  ]
</script>

<span class="eyebrow kn-enter">À propos</span>
<div class="head kn-enter kn-d1">
  <Mark size={52} animated id="kn-about" />
  <h1>Kyber<span>Now</span></h1>
  <div class="spacer"></div>
  <span class="version">version 0.1.0</span>
</div>

<div class="card dev kn-enter kn-d2">
  <div class="who">
    <span>Conçu et développé par</span>
    <strong>Les Lignes Claires</strong>
  </div>
  <button class="btn" onclick={() => send({ cmd: 'openUrl', url: 'https://www.leslignesclaires.fr' })}>
    www.leslignesclaires.fr
    <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"><path d="M5.5 3H3v8h8V8.5M8 2.5h3.5V6M11.5 2.5L6.5 7.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
</div>

<div class="card list kn-enter kn-d3">
  <div class="list-head"><strong>Logiciels libres inclus</strong></div>
  <div class="scroll">
    <table>
      <thead>
        <tr><th scope="col">Composant</th><th scope="col">Rôle</th><th scope="col">Licence</th></tr>
      </thead>
      <tbody>
        {#each licenses as [name, role, spdx] (name)}
          <tr><td>{name}</td><td class="role">{role}</td><td><span class="spdx">{spdx}</span></td></tr>
        {/each}
      </tbody>
    </table>
  </div>
  <p class="note">
    KyberNow est distribué sous AGPL-3.0. Les textes des licences sont installés dans C:\Program Files\KyberNow\licenses.
  </p>
</div>

<style>
  .head {
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .head h1 {
    margin: 0;
  }

  .head h1 span {
    color: var(--accent);
  }

  .version {
    font-family: var(--mono);
    font-size: 13px;
    color: var(--muted);
  }

  .dev {
    margin-top: 16px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 18px;
  }

  .who {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: var(--muted);
  }

  .who strong {
    font-size: 17px;
    color: var(--text);
  }

  .list {
    margin-top: 14px;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .list-head {
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    font-size: 15px;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th {
    text-align: left;
    padding: 8px 18px;
    font-weight: 500;
    color: var(--muted);
  }

  td {
    padding: 7px 18px;
    border-top: 1px solid #1a222d;
    font-weight: 600;
  }

  td.role {
    font-weight: 400;
    color: var(--muted);
  }

  .spdx {
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 400;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--text-2);
    white-space: nowrap;
  }

  .note {
    margin: 0;
    padding: 10px 18px;
    border-top: 1px solid var(--line);
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }
</style>
