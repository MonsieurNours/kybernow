<script lang="ts">
  import qrcode from 'qrcode-generator'
  import { api, type Pairing } from '../lib/api'
  import InfoTip from '../lib/InfoTip.svelte'
  import { errorText, notify } from '../lib/store.svelte'

  let pairing = $state<Pairing | null>(null)

  $effect(() => {
    api.pairing().then((p) => (pairing = p)).catch((e) => notify('error', errorText(e)))
  })

  // QR code en SVG, menthe sur fond sombre comme le reste de la console.
  const qr = $derived.by(() => {
    if (!pairing) return ''
    const code = qrcode(0, 'M')
    code.addData(pairing.link)
    code.make()
    const n = code.getModuleCount()
    let path = ''
    for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) if (code.isDark(y, x)) path += `M${x} ${y}h1v1h-1z`
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 ${n + 4} ${n + 4}" shape-rendering="crispEdges"><rect x="-2" y="-2" width="${n + 4}" height="${n + 4}" fill="#f4f7fa" rx="1.5"/><path d="${path}" fill="#06251d"/></svg>`
  })

  // Empreinte groupée par quatre caractères : les quatre premiers groupes suffisent à comparer.
  const groups = $derived(
    (pairing?.fingerprint ?? '')
      .replace(/^sha256:/, '')
      .replaceAll(':', '')
      .toUpperCase()
      .match(/.{1,4}/g) ?? [],
  )

  async function copy() {
    if (!pairing) return
    await navigator.clipboard.writeText(pairing.link)
    notify('ok', 'Lien copié.')
  }
</script>

<header>
  <span class="eyebrow">Vos appareils</span>
  <h1>Ajouter un appareil</h1>
  <p class="muted intro">
    L'app KyberNow sur Mac (et bientôt Apple TV) trouve cet ordinateur toute seule sur le réseau local. Par VPN, ou
    pour aller plus vite, scannez ce QR code avec la caméra du Mac, ou collez le lien dans l'app.
  </p>
</header>

{#if pairing}
  <section class="card pair">
    <div class="qr" aria-label="QR code d'appairage">{@html qr}</div>
    <div class="side">
      <div>
        <div class="label">
          <strong>Lien d'appairage</strong>
          <InfoTip label="Lien d'appairage">
            <strong>Ce que contient le lien</strong>
            <span>
              L'adresse et le port de cette console, et l'empreinte de son certificat : l'app sait ainsi qu'elle parle
              bien à cet ordinateur, sans question à la première connexion. Il ne contient aucun mot de passe.
            </span>
          </InfoTip>
        </div>
        <div class="link">
          <code>{pairing.link}</code>
          <button onclick={copy}>Copier</button>
        </div>
      </div>
      <div>
        <div class="label"><strong>Empreinte du certificat</strong></div>
        <p class="muted small">À comparer avec celle que l'app affiche si vous ajoutez cet ordinateur par son adresse.</p>
        <div class="fp mono">
          {#each groups as g, i (i)}
            <span class:first={i < 4}>{g}</span>
          {/each}
        </div>
      </div>
      <dl class="kv">
        <dt>Ordinateur</dt>
        <dd>{pairing.hostname}</dd>
        <dt>Adresse</dt>
        <dd class="mono">{pairing.address} · console {pairing.port}</dd>
      </dl>
    </div>
  </section>
{/if}

<style>
  .intro {
    max-width: 720px;
    font-size: 0.92rem;
  }

  .pair {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 28px;
    align-items: start;
  }

  .qr :global(svg) {
    width: 260px;
    height: 260px;
    display: block;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
  }

  .link {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .link code {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    font-family: var(--mono);
    font-size: 0.8rem;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
  }

  .fp {
    display: grid;
    grid-template-columns: repeat(8, minmax(0, 1fr));
    gap: 6px 10px;
    font-size: 0.9rem;
    color: var(--muted);
  }

  .fp .first {
    color: var(--accent);
    font-weight: 600;
  }

  .mono {
    font-family: var(--mono);
  }

  .small {
    font-size: 0.84rem;
    margin: 0 0 8px;
  }

  @media (max-width: 860px) {
    .pair {
      grid-template-columns: 1fr;
    }
  }
</style>
