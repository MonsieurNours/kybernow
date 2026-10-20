<!-- Terminé : l'ordinateur est prêt ; la console sur le téléphone en un scan. -->
<script lang="ts">
  import qrcode from 'qrcode-generator'
  import { send } from '../lib/backend'
  import { app } from '../lib/state.svelte'

  const ip = $derived(app.result?.lanIp ?? app.lanIp ?? 'adresse-du-pc')
  const consolePort = $derived(app.result?.consolePort ?? app.ports.console)
  const consoleUrl = $derived(`https://${ip}:${consolePort}`)
  const needReboot = $derived(app.result?.needReboot ?? false)

  // QR code généré localement (aucun service externe), sur fond clair.
  const qr = $derived.by(() => {
    const code = qrcode(0, 'M')
    code.addData(consoleUrl)
    code.make()
    const n = code.getModuleCount()
    let path = ''
    for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) if (code.isDark(y, x)) path += `M${x} ${y}h1v1h-1z`
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 ${n + 4} ${n + 4}" shape-rendering="crispEdges"><rect x="-2" y="-2" width="${n + 4}" height="${n + 4}" rx="2" fill="#F2F8FA"/><path d="${path}" fill="#05070A"/></svg>`
  })

  function finish(openConsole: boolean, reboot = false) {
    if (openConsole) send({ cmd: 'openUrl', url: consoleUrl })
    send({ cmd: reboot ? 'reboot' : 'close' })
  }
</script>

<div class="done">
  <div class="text">
    <span class="lbl ok kn-enter"><span class="dot"></span>Installation terminée</span>
    <h1 class="title kn-enter kn-d1">Cet ordinateur est prêt. Il est partout.</h1>
    <p class="lead kn-enter kn-d2">
      Ouvrez KyberNow sur n'importe lequel de vos appareils : cet ordinateur s'y trouve déjà. Sinon, son adresse :
      <span class="mono addr">{ip}</span>
    </p>
    <p class="note kn-enter kn-d3">
      Sans app ? Ouvrez
      <button type="button" class="text-button mono small" onclick={() => send({ cmd: 'openUrl', url: `${consoleUrl}/ecran` })}>{consoleUrl}/ecran</button>
      dans un navigateur.
    </p>
    {#if needReboot}
      <div class="notice warn kn-enter kn-d3">
        <span>Les manettes fonctionneront après le prochain redémarrage. Le reste est déjà actif.</span>
        <button type="button" class="ghost small" onclick={() => finish(true, true)}>Redémarrer maintenant</button>
      </div>
    {/if}
  </div>

  <div class="panel phone kn-enter kn-d2">
    <div class="qr" role="img" aria-label="QR code de la console : {consoleUrl}">{@html qr}</div>
    <strong>La console sur votre téléphone</strong>
    <span class="note">Un avertissement de certificat s'affiche la première fois : c'est normal.</span>
  </div>
</div>

<div class="footer">
  <div class="spacer"></div>
  <button type="button" class="ghost" onclick={() => finish(false)}>Fermer</button>
  <button type="button" class="primary lit" onclick={() => finish(true)}>Ouvrir la console</button>
</div>

<style>
  .done {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    gap: 44px;
    align-items: start;
    padding-top: 44px;
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

  .addr {
    color: var(--text);
    user-select: text;
  }

  .small {
    font-size: 12px;
  }

  .notice {
    align-items: center;
    flex-wrap: wrap;
  }

  .notice span {
    flex: 1 1 260px;
  }

  .phone {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 20px;
  }

  .qr {
    width: 140px;
    height: 140px;
  }

  .qr :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }

  .phone strong {
    font-size: 14px;
  }
</style>
