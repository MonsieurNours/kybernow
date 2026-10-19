<script lang="ts">
  import qrcode from 'qrcode-generator'
  import { send } from '../lib/backend'
  import { app } from '../lib/state.svelte'

  let openConsole = $state(true)

  const ip = $derived(app.result?.lanIp ?? app.lanIp ?? 'adresse-du-pc')
  const streamPort = $derived(app.result?.streamPort ?? app.ports.stream)
  const consolePort = $derived(app.result?.consolePort ?? app.ports.console)
  const consoleUrl = $derived(`https://${ip}:${consolePort}`)
  const needReboot = $derived(app.result?.needReboot ?? false)

  // QR code généré localement (aucun service externe).
  const qr = $derived.by(() => {
    const code = qrcode(0, 'M')
    code.addData(consoleUrl)
    code.make()
    return code.createSvgTag({ cellSize: 4, margin: 0, scalable: true })
  })

  function finish(reboot: boolean) {
    if (openConsole) send({ cmd: 'openUrl', url: consoleUrl })
    send({ cmd: reboot ? 'reboot' : 'close' })
  }
</script>

<div class="top">
  <div class="check">
    <span class="ripple"></span>
    <span class="ripple late"></span>
    <span class="disc kn-pop">
      <svg width="26" height="26" viewBox="0 0 24 24" aria-hidden="true"><path class="draw" pathLength="100" d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </span>
  </div>
  <span class="eyebrow kn-enter" style="animation-delay: 0.5s">Installation terminée</span>
</div>
<h1 class="kn-enter" style="margin-top: 14px; font-size: 30px; animation-delay: 0.5s">
  C'est prêt. Allumez votre Mac ou votre Apple TV.
</h1>
<p class="lead kn-enter" style="margin-top: 6px; animation-delay: 0.65s">
  Le serveur tourne déjà. Vos appareils trouveront cet ordinateur tout seuls sur le réseau ; sinon, saisissez son adresse.
</p>

<div class="link card kn-enter" style="animation-delay: 0.8s">
  <div class="node"><span></span>Cet ordinateur</div>
  <div class="wire"><div class="wire-line"></div><span class="packet"></span></div>
  <div class="node late"><span></span>Votre Apple TV</div>
  <span class="mono">prêt à se connecter</span>
</div>

<div class="grid">
  <div class="card panel kn-enter" style="animation-delay: 0.95s">
    <span class="label-mono">Pour jouer ou travailler</span>
    <strong>Ouvrez l'app sur votre Mac ou votre Apple TV</strong>
    <div class="address">
      <span>Adresse de cet ordinateur</span>
      <span class="big">{ip}<span>:{streamPort}</span></span>
    </div>
    <span class="hint">
      Sans app ? Ouvrez <button class="url" onclick={() => send({ cmd: 'openUrl', url: `https://${ip}:${streamPort}/webclient/` })}>https://{ip}:{streamPort}/webclient/</button> dans Chrome.
    </span>
  </div>

  <div class="card panel row kn-enter" style="animation-delay: 1.1s">
    <div class="col">
      <span class="label-mono">Pour tout régler</span>
      <strong>La console d'administration</strong>
      <button class="url" onclick={() => send({ cmd: 'openUrl', url: consoleUrl })}>{consoleUrl}</button>
      <span class="hint">
        Scannez le code avec votre téléphone. Un avertissement de certificat s'affiche la première fois : c'est normal.
      </span>
    </div>
    <div class="qr" role="img" aria-label="QR code de la console : {consoleUrl}">{@html qr}</div>
  </div>
</div>

{#if needReboot}
  <div class="reboot kn-enter" style="animation-delay: 1.1s">
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><path d="M14.5 9a5.5 5.5 0 11-1.6-3.9M14.5 3v3.2h-3.2" fill="none" stroke="#F5B65A" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
    <span>Les manettes fonctionneront après un redémarrage de l'ordinateur. Le reste est déjà actif.</span>
  </div>
{/if}

<div class="footer">
  <label class="open">
    <input type="checkbox" bind:checked={openConsole} />
    Ouvrir la console maintenant
  </label>
  <div class="spacer"></div>
  {#if needReboot}
    <button class="btn" onclick={() => finish(false)}>Redémarrer plus tard</button>
    <button class="btn primary" onclick={() => finish(true)}>Redémarrer maintenant</button>
  {:else}
    <button class="btn primary" onclick={() => finish(false)}>Terminer</button>
  {/if}
</div>

<style>
  .top {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .check {
    position: relative;
    width: 52px;
    height: 52px;
    flex-shrink: 0;
  }

  .ripple,
  .disc {
    position: absolute;
    inset: 0;
    border-radius: 50%;
  }

  .ripple {
    border: 2px solid var(--accent);
    animation: kn-ripple 2.2s ease-out infinite;
  }

  .ripple.late {
    animation-delay: 1.1s;
  }

  .disc {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent);
    color: var(--accent-ink);
  }

  .draw {
    stroke-dasharray: 100;
    animation: kn-draw 0.7s cubic-bezier(0.6, 0.1, 0.2, 1) 0.15s both;
  }

  .link {
    margin-top: 14px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 14px;
  }

  .node {
    height: 34px;
    padding: 0 12px;
    border-radius: 9px;
    background: var(--surface-2);
    border: 1px solid var(--accent);
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    animation: kn-light 1.6s ease-in-out infinite;
  }

  .node.late {
    animation-delay: 0.8s;
  }

  .node span {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }

  .wire {
    position: relative;
    flex: 1;
    height: 8px;
  }

  .wire-line {
    position: absolute;
    left: 0;
    right: 0;
    top: 3px;
    height: 2px;
    background: #1f2d2a;
  }

  .packet {
    position: absolute;
    top: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 10px var(--accent);
    animation: kn-travel 1.6s ease-in-out infinite;
  }

  .mono {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--muted);
  }

  .grid {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  .panel {
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .panel strong {
    font-size: 16px;
  }

  .panel.row {
    flex-direction: row;
    gap: 16px;
  }

  .col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .address {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 14px;
    border-radius: 10px;
    background: var(--bg);
    font-size: 12px;
    color: var(--muted);
  }

  .big {
    font-family: var(--mono);
    font-size: 20px;
    letter-spacing: 0.02em;
    color: var(--text);
    user-select: text;
    -webkit-user-select: text;
  }

  .big span {
    color: var(--faint);
  }

  .hint {
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }

  .url {
    padding: 0;
    border: none;
    background: none;
    font-family: var(--mono);
    font-size: 13px;
    color: var(--text-2);
    text-align: left;
    text-decoration: underline;
    text-decoration-color: var(--line-2);
    text-underline-offset: 3px;
    word-break: break-all;
  }

  .url:hover {
    color: var(--accent);
  }

  .qr {
    width: 104px;
    height: 104px;
    flex-shrink: 0;
    padding: 10px;
    border-radius: 12px;
    background: #f4f7fa;
  }

  .qr :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }

  .reboot {
    margin-top: 10px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border-radius: 12px;
    background: var(--warn-bg);
    border: 1px solid var(--warn-line);
    font-size: 13px;
    color: var(--warn-text);
  }

  .open {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
    cursor: pointer;
  }

  .open input {
    width: 18px;
    height: 18px;
    accent-color: var(--accent);
  }
</style>
