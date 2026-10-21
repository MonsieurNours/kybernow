<!--
  Ajouter un ordinateur : le trouver (réseau, lien de la console, adresse),
  puis y avoir accès (code à accepter dans la console, ou compte et mot de passe).
  Pas de QR code à la caméra : sur un PC, le lien de la console suffit.
-->
<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { Candidate, Computer, Found } from './lib/api'
  import { invoke } from './lib/api'

  let {
    discovered,
    computers,
    deviceName,
    initial,
    onclose,
    onadded,
  }: {
    discovered: Found[]
    computers: Computer[]
    deviceName: string
    initial: Found | null
    onclose: () => void
    onadded: (id: string) => void
  } = $props()

  type Way = 'reseau' | 'lien' | 'adresse'
  let way = $state<Way>('reseau')
  let candidate = $state<Candidate | null>(null)
  let access = $state<'code' | 'compte'>('code')
  let error = $state<string | null>(null)
  let working = $state(false)

  // Lien et adresse.
  let link = $state('')
  let host = $state('')
  let port = $state('9443')
  let seen = $state<Candidate | null>(null)

  // Code.
  let code = $state<string | null>(null)
  let pairState = $state<'pending' | 'refused' | 'expired'>('pending')
  // Interrogation de la console : un appel à la fois, relancé 1,5 s après
  // la réponse ; `round` invalide une interrogation dépassée.
  let timer: ReturnType<typeof setTimeout> | null = null
  let round = 0

  // Compte.
  let user = $state('')
  let password = $state('')

  // Ouvert depuis une carte « Sur le réseau » : l'ordinateur est déjà choisi.
  const first = untrack(() => initial)
  if (first) choose({ host: first.host, port: first.port, name: first.name, fingerprint: first.fingerprint })

  const known = (f: Found) => computers.some((c) => c.host === f.host && c.port === f.port)
  const groups = (fp: string | null) => (fp ?? '').replace(/^sha256:/, '').split(':')

  function stopPolling() {
    round++
    if (timer) clearTimeout(timer)
    timer = null
  }

  /** Ordinateur trouvé : l'accès par code commence aussitôt. */
  function choose(c: Candidate) {
    candidate = c
    error = null
    if (access === 'code') startCode()
  }

  onDestroy(() => {
    stopPolling()
    invoke('pair_cancel').catch(() => {})
  })

  async function run<T>(f: () => Promise<T>): Promise<T | undefined> {
    working = true
    error = null
    try {
      return await f()
    } catch (e) {
      error = String(e)
    } finally {
      working = false
    }
  }

  async function fromLink() {
    const c = await run(() => invoke<Candidate>('parse_link', { link }))
    if (c) choose(c)
  }

  async function fromAddress() {
    const c = await run(() => invoke<Candidate>('probe_address', { host: host.trim(), port: Number(port) || 9443 }))
    if (c) seen = c
  }

  async function startCode() {
    stopPolling()
    const mine = round
    pairState = 'pending'
    code = null
    const c = await run(() => invoke<string>('pair_start', { candidate }))
    if (!c || mine !== round) return
    code = c
    const poll = async () => {
      try {
        const r = await invoke<string>('pair_poll')
        if (mine !== round) return
        if (r === 'pending') {
          timer = setTimeout(poll, 1500)
          return
        }
        if (r === 'refused' || r === 'expired') pairState = r
        else onadded(r)
      } catch (e) {
        if (mine === round) error = String(e)
      }
    }
    timer = setTimeout(poll, 1500)
  }

  async function withAccount() {
    const id = await run(() => invoke<string>('add_computer', { candidate, user, password }))
    if (id) onadded(id)
  }

  function chooseAccess(a: 'code' | 'compte') {
    if (a === access) return
    access = a
    error = null
    if (a === 'compte') {
      stopPolling()
      invoke('pair_cancel').catch(() => {})
    } else startCode()
  }

  function back() {
    stopPolling()
    invoke('pair_cancel').catch(() => {})
    candidate = null
    seen = null
    code = null
    error = null
  }

  const ways: { value: Way; title: string; sub: string; icon: string }[] = [
    { value: 'reseau', title: 'Sur le réseau', sub: 'Trouvé tout seul', icon: 'wifi' },
    { value: 'lien', title: 'Avec le lien de la console', sub: 'kybernow://… collé ou cliqué', icon: 'link' },
    { value: 'adresse', title: 'Par adresse', sub: 'VPN, Tailscale, autre réseau', icon: 'globe' },
  ]
</script>

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="add-title">
    <nav>
      <h2 id="add-title">Ajouter un ordinateur</h2>
      {#each ways as w (w.value)}
        <button class="way" class:on={!candidate && way === w.value} onclick={() => { back(); way = w.value }}>
          <Icon name={w.icon} size={18} />
          <span><span class="t">{w.title}</span><span class="s">{w.sub}</span></span>
        </button>
      {/each}
      <p class="aside">Pas de caméra à viser : le QR code de la console porte le même lien. Collez-le ici, ou cliquez-le sur ce PC : KyberNow s’ouvre directement sur l’ajout.</p>
    </nav>

    <div class="pane">
      <button class="close" aria-label="Fermer" onclick={onclose}><Icon name="x" size={14} /></button>

      {#if !candidate}
        {#if way === 'reseau'}
          <div class="eyebrow">Sur ce réseau</div>
          {#if discovered.length === 0}
            <p class="hint">Aucun ordinateur annoncé pour l’instant. La console KyberNow s’annonce sur le réseau local ; à distance (VPN), passez par le lien ou l’adresse.</p>
          {:else}
            <ul class="found">
              {#each discovered as f (f.id)}
                <li>
                  <button disabled={known(f)} onclick={() => choose({ host: f.host, port: f.port, name: f.name, fingerprint: f.fingerprint })}>
                    <span class="name">{f.name}</span>
                    <span class="mono addr">{f.host}:{f.port}</span>
                    <span class="go">{known(f) ? 'Déjà ajouté' : 'Choisir'}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {:else if way === 'lien'}
          <label class="col">
            <span class="eyebrow">Lien de la console (Accès)</span>
            <textarea class="field area mono" rows="3" placeholder="kybernow://192.168.1.100:31544?name=…&fp=sha256:…" bind:value={link}></textarea>
          </label>
          <div><button class="primary" disabled={working || !link.trim()} onclick={fromLink}>Continuer</button></div>
        {:else}
          {#if !seen}
            <div class="grid">
              <label class="col"><span class="eyebrow">Adresse</span><input class="field" placeholder="192.168.1.100" bind:value={host} /></label>
              <label class="col"><span class="eyebrow">Port de la console</span><input class="field mono" inputmode="numeric" bind:value={port} /></label>
            </div>
            <div><button class="primary" disabled={working || !host.trim()} onclick={fromAddress}>{working ? 'Recherche…' : 'Continuer'}</button></div>
          {:else}
            <div class="eyebrow">Empreinte de la console</div>
            <div class="fp mono">
              {#each groups(seen.fingerprint) as g, i}<span class:strong={i < 4}>{g}</span>{/each}
            </div>
            <p class="hint">Comparez les quatre premiers groupes avec l’empreinte affichée dans la console de l’ordinateur, espace Accès. Différente : n’allez pas plus loin.</p>
            <div class="row">
              <button class="primary" onclick={() => seen && choose(seen)}>C’est la même</button>
              <button class="ghost" onclick={() => (seen = null)}>Annuler</button>
            </div>
          {/if}
        {/if}
      {:else}
        <div class="who">
          <div>
            <div class="eyebrow">Ordinateur choisi</div>
            <div class="whoname">{candidate.name ?? candidate.host} <span class="mono addr">· {candidate.host}:{candidate.port}</span></div>
          </div>
          <button class="ghost" onclick={back}>Changer</button>
        </div>

        <div class="tabs" role="tablist">
          <button role="tab" aria-selected={access === 'code'} onclick={() => chooseAccess('code')}>Avec un code</button>
          <button role="tab" aria-selected={access === 'compte'} onclick={() => chooseAccess('compte')}>Avec un compte</button>
        </div>

        {#if access === 'code'}
          {#if code}
            <div class="digits" aria-label="Code {code}">
              {#each code.split('') as d, i}{#if i === 3}<span class="gap"></span>{/if}<span class="d mono">{d}</span>{/each}
            </div>
            <ol>
              <li>Ouvrez la console de {candidate.name ?? 'l’ordinateur'}, espace <strong>Accès › Appareils</strong>, ou le panneau de son icône près de l’horloge.</li>
              <li>Acceptez la demande de <strong>{deviceName || 'ce PC'}</strong> avec ce code.</li>
              <li>Rien d’autre à taper : ce PC reçoit son propre accès, que vous pourrez retirer depuis la console.</li>
            </ol>
            {#if pairState === 'pending'}
              <div class="wait"><span class="pulse"></span>En attente de la console…</div>
            {:else}
              <div class="error">{pairState === 'refused' ? 'La console a refusé la demande.' : 'La demande a expiré.'}</div>
              <div><button class="ghost" onclick={startCode}>Nouveau code</button></div>
            {/if}
          {:else if working}
            <div class="wait"><span class="pulse"></span>Demande à la console…</div>
          {:else if error}
            <div><button class="ghost" onclick={startCode}>Réessayer</button></div>
          {/if}
        {:else}
          <div class="grid">
            <label class="col"><span class="eyebrow">Compte</span><input class="field" autocomplete="username" bind:value={user} /></label>
            <label class="col"><span class="eyebrow">Mot de passe</span><input class="field" type="password" autocomplete="current-password" bind:value={password} /></label>
          </div>
          <p class="hint">Le compte de l’ordinateur (celui choisi à l’installation, ou un compte d’appareil). Le mot de passe est rangé dans le gestionnaire d’identifiants de Windows.</p>
          <div><button class="primary" disabled={working || !user.trim() || !password} onclick={withAccount}>{working ? 'Vérification…' : 'Ajouter'}</button></div>
        {/if}
      {/if}

      {#if error}<div class="error" role="alert">{error}</div>{/if}
    </div>
  </div>
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    z-index: 30;
    background: rgb(5 7 10 / 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
    animation: fade 0.25s var(--ease-out);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .sheet {
    width: min(980px, 100%);
    height: min(680px, 100%);
    display: flex;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
    overflow: hidden;
    animation: rise 0.32s var(--ease-out);
  }

  @keyframes rise {
    from {
      transform: translateY(18px) scale(0.985);
      opacity: 0;
    }
  }

  nav {
    width: 320px;
    flex: none;
    padding: 30px 18px;
    border-right: 1px solid var(--line);
    background: var(--ink-0);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h2 {
    margin: 0 10px 18px;
    font-size: 25px;
    font-weight: 640;
    font-stretch: 125%;
  }

  .way {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 13px 14px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: none;
    text-align: left;
    color: var(--muted);
  }

  .way > span {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .way .t {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .way .s {
    font-size: 12px;
    color: var(--faint);
  }

  .way.on {
    border-color: var(--signal);
    background: var(--ink-3);
    box-shadow: 0 0 14px rgb(109 91 255 / 0.25);
  }

  .way.on :global(svg) {
    color: var(--signal);
  }

  .aside {
    margin: auto 8px 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--faint);
  }

  .pane {
    position: relative;
    flex: 1;
    padding: 40px 48px;
    display: flex;
    flex-direction: column;
    gap: 22px;
    overflow-y: auto;
  }

  .close {
    position: absolute;
    right: 16px;
    top: 16px;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close:hover {
    background: rgb(255 255 255 / 0.06);
  }

  .hint {
    margin: 0;
    font-size: 14px;
    line-height: 1.55;
    color: var(--muted);
  }

  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .found button {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
    border: 1px solid var(--line-2);
    border-radius: 12px;
    background: var(--ink-2);
    text-align: left;
  }

  .found button:hover:not(:disabled) {
    border-color: var(--signal);
  }

  .found .name {
    font-size: 16px;
    font-weight: 600;
  }

  .addr {
    font-size: 12px;
    color: var(--faint);
  }

  .go {
    font-size: 13px;
    color: var(--signal);
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 14px;
  }

  .area {
    height: auto;
    padding: 12px 14px;
    resize: none;
    font-size: 13px;
    line-height: 1.5;
  }

  .fp {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 15px;
    color: var(--faint);
    user-select: text;
  }

  .fp .strong {
    color: var(--text);
    padding: 4px 6px;
    border-radius: 6px;
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.4);
  }

  .row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .who {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    padding-right: 40px;
  }

  .whoname {
    margin-top: 6px;
    font-size: 20px;
    font-weight: 600;
  }

  .tabs {
    display: flex;
    gap: 6px;
    border-bottom: 1px solid var(--line);
  }

  .tabs button {
    height: 40px;
    padding: 0 14px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font-size: 14px;
  }

  .tabs button[aria-selected='true'] {
    color: var(--text);
    border-bottom-color: var(--signal);
  }

  .digits {
    display: flex;
    gap: 10px;
  }

  .d {
    width: 64px;
    height: 84px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: var(--ink-2);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 40px;
    user-select: text;
  }

  .gap {
    width: 18px;
  }

  ol {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 15px;
    line-height: 1.5;
  }

  .wait {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--ink-0);
    font-size: 14px;
  }

  .pulse {
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background: var(--signal);
    box-shadow: 0 0 0 4px rgb(154 140 255 / 0.18), 0 0 14px var(--signal);
    animation: breathe 1.6s ease-in-out infinite;
  }

  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 7px rgb(154 140 255 / 0.08), 0 0 6px var(--signal);
    }
  }
</style>
