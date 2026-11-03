<!-- Terminé : ce Mac est prêt ; son adresse, /link et Admin en un scan. -->
<script lang="ts">
  import qrcode from 'qrcode-generator'
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import { allGranted, app } from '../state.svelte'

  const ip = $derived(app.result?.lanIp ?? app.lanIp ?? 'localhost')
  const port = $derived(app.result?.consolePort ?? app.ports.console)
  const address = $derived(`${ip}:${port}`)
  const url = $derived(`https://${address}`)
  let copied = $state(false)

  // QR code fait sur place (aucun service extérieur), sur fond clair.
  const qr = $derived.by(() => {
    const code = qrcode(0, 'M')
    code.addData(url)
    code.make()
    const n = code.getModuleCount()
    let path = ''
    for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) if (code.isDark(y, x)) path += `M${x} ${y}h1v1h-1z`
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 ${n + 4} ${n + 4}" shape-rendering="crispEdges"><rect x="-2" y="-2" width="${n + 4}" height="${n + 4}" fill="#E3EDF0"/><path d="${path}" fill="#05070A"/></svg>`
  })

  function copy() {
    send({ cmd: 'copy', text: url })
    copied = true
    setTimeout(() => (copied = false), 1600)
  }

  function finish(openAdmin: boolean) {
    if (openAdmin) send({ cmd: 'openUrl', url: `https://localhost:${port}` })
    send({ cmd: 'close' })
  }
</script>

<div class="done">
  <span class="lbl-m ok kn-enter">{t('mac.done.label')}</span>
  <h1 class="t kn-enter kn-d1">{t('mac.done.title')}</h1>
  <p class="lead-m kn-enter kn-d2">{t('mac.done.lead')}</p>

  <div class="grid kn-enter kn-d2">
    <div class="left">
      <div class="card-m address">
        <span class="mono-m">{address}</span>
        <button type="button" class="btn small" onclick={copy}>{copied ? t('mac.done.copied') : t('mac.done.copy')}</button>
      </div>
      <p class="web">
        {t('mac.done.webBefore')} <button type="button" class="url mono-m" onclick={() => send({ cmd: 'openUrl', url: `${url}/link` })}>{url}/link</button> {t('mac.done.webAfter')}
      </p>
      <div class="menubar" class:warn={!allGranted()}>
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke={allGranted() ? '#9A8CFF' : '#F5B841'} stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="18" height="4" rx="1" /><path d="M17 5h1" /></svg>
        <span>{allGranted() ? t('mac.done.menubar') : t('mac.done.missing')}</span>
      </div>
    </div>
    <figure class="card-m">
      <div class="qr" role="img" aria-label={t('done.qr', { url })}>{@html qr}</div>
      <figcaption>{t('mac.done.phone')}</figcaption>
    </figure>
  </div>
</div>

<div class="foot-m end">
  <div class="buttons">
    <button type="button" class="btn" onclick={() => finish(false)}>{t('mac.done.close')}</button>
    <button type="button" class="btn primary-m" onclick={() => finish(true)}>{t('mac.done.open')}</button>
  </div>
</div>

<style>
  .done {
    display: flex;
    flex-direction: column;
  }

  .grid {
    margin-top: 20px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 200px;
    gap: 20px;
    align-items: stretch;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .address {
    padding: 16px 18px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: 15px;
    user-select: text;
  }

  .address .btn {
    height: 32px;
    padding: 0 12px;
    font-size: 12.5px;
    color: var(--muted);
  }

  .web {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--faint);
  }

  .url {
    border: 0;
    padding: 0;
    background: none;
    font-size: 12px;
    color: var(--muted);
  }

  .url:hover {
    color: var(--signal);
  }

  .menubar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 10px;
    background: #0e141a;
    border: 1px solid #1e2832;
    font-size: 13px;
    color: var(--muted);
  }

  .menubar.warn {
    border-color: #4a3a1e;
    color: var(--warn-text);
  }

  figure {
    margin: 0;
    padding: 14px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .qr {
    width: 140px;
    height: 140px;
    border-radius: 8px;
    overflow: hidden;
  }

  .qr :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }

  figcaption {
    font-size: 12px;
    line-height: 1.4;
    color: var(--faint);
    text-align: center;
  }
</style>
