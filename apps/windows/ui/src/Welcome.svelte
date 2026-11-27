<!--
  Accueil au premier lancement (docs/link-ui.md « Accueil et compte KyberNow ») : trois écrans par-dessus
  l'accueil, sous la barre de titre. Chacun : le logo de l'app en haut à gauche, « Passer » en haut à droite
  (écran 3 : « Plus tard »), l'étiquette « BIENVENUE · {n} / 3 », un grand titre, un texte, les boutons, trois
  traits d'avancement en bas.

  1. « Votre ordinateur, sur tous vos appareils » et « Continuer ».
  2. Les quatre avantages d'un compte ; « Connecter un compte » (la connexion de toujours : la page du relais dans
     le navigateur, « Connexion au compte… » jusqu'à son retour ; réussie, l'app ferme l'accueil et les ordinateurs
     du compte arrivent ; annulée, on reste ici), « Créer un compte » (la page d'inscription dans le navigateur,
     puis l'avis « Créez votre compte dans le navigateur, puis revenez ici… »), « Continuer sans compte ».
  3. « Ajoutez votre ordinateur » : les ordinateurs vus sur le réseau (carte « SUR LE RÉSEAU », « Ajouter »), « UN
     CODE » (« Rejoindre avec un code »), « PAS ENCORE INSTALLÉ » (la page de téléchargement de la Station).

  « Passer », « Plus tard », « Ajouter » ou « Rejoindre avec un code » : l'accueil est vu (`welcome_done`, par l'app).
-->
<script lang="ts">
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import type { Computer, Found } from './lib/api'
  import { invoke } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    computers,
    discovered,
    relay,
    signingIn = false,
    error = null,
    onsignin,
    oncancelsignin,
    onadd,
    onjoin,
    ondone,
  }: {
    computers: Computer[]
    /** Les Stations trouvées sur le réseau, sans celles déjà à l'accueil (l'app les écarte). */
    discovered: Found[]
    /** Le relais de « Connecter un compte », tel qu'on le montre (« share.kybernow.app »). */
    relay: string
    /** « Connexion au compte… » : la page du relais est ouverte, l'app attend son retour. */
    signingIn?: boolean
    /** Le refus de la connexion (texte du cœur). */
    error?: string | null
    onsignin: () => void
    oncancelsignin: () => void
    /** « Ajouter » (un ordinateur vu sur le réseau) : l'ajout, l'accueil fermé. */
    onadd: (found: Found) => void
    /** « Rejoindre avec un code » : le parcours de toujours, l'accueil fermé. */
    onjoin: () => void
    /** « Passer », « Plus tard » : l'accueil fermé. */
    ondone: () => void
  } = $props()

  let step = $state(1)
  /** « Créer un compte » : la page s'est ouverte dans le navigateur, l'avis remplace l'aide. */
  let signedUp = $state(false)
  let localError = $state<string | null>(null)

  /** Les ordinateurs vus sur le réseau et pas encore ajoutés (ceux que l'accueil montre dans « Sur le réseau »). */

  async function signUp() {
    localError = null
    try {
      await invoke('welcome_sign_up')
      signedUp = true
    } catch (e) {
      localError = String(e)
    }
  }

  async function download() {
    localError = null
    try {
      await invoke('welcome_station_download')
    } catch (e) {
      localError = String(e)
    }
  }
</script>

<div class="welcome step{step}" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
  <div class="top">
    <Mark product="link" size={24} />
    <button class="skip" onclick={ondone}>{step === 3 ? t('welcome.later') : t('welcome.skip')}</button>
  </div>

  <div class="body">
    {#key step}
      <section class="screen">
        <div class="label mono">{t('welcome.step', { n: step })}</div>
        {#if step === 1}
          <h1 id="welcome-title">{t('welcome.s1_title')}</h1>
          <p class="lead">{t('welcome.s1_text')}</p>
          <div class="actions">
            <button class="primary" onclick={() => (step = 2)}>{t('common.continue')}<Icon name="chevron" size={18} stroke={2.2} /></button>
          </div>
        {:else if step === 2}
          <h1 id="welcome-title">{t('welcome.s2_title')}</h1>
          <p class="lead">{t('welcome.s2_text')}</p>
          <div class="perks">
            {#each [{ icon: 'globe', k: 'away' }, { icon: 'monitor', k: 'devices' }, { icon: 'share', k: 'share' }, { icon: 'sliders', k: 'settings' }] as p (p.k)}
              <div class="perk">
                <Icon name={p.icon} size={24} stroke={1.8} />
                <span class="eyebrow">{t(`welcome.${p.k}_title`)}</span>
                <span class="text">{t(`welcome.${p.k}_text`)}</span>
              </div>
            {/each}
          </div>
          <div class="actions">
            {#if signingIn}
              <button class="primary" disabled><Icon name="user" size={18} />{t('account.signing_in')}</button>
              <button class="ghost tall" onclick={oncancelsignin}>{t('common.cancel')}</button>
            {:else}
              <button class="primary" onclick={() => ((localError = null), onsignin())}><Icon name="user" size={18} />{t('account.sign_in')}</button>
              <button class="ghost tall" onclick={signUp}>{t('welcome.create_account')}</button>
            {/if}
            <button class="link" onclick={() => (step = 3)}>{t('welcome.without')}</button>
          </div>
          {#if localError || error}<div class="error" role="alert">{localError ?? error}</div>{/if}
          <p class="help" role="status">{signedUp ? t('welcome.sign_up_notice') : t('welcome.s2_help', { relay })}</p>
        {:else}
          <h1 id="welcome-title">{t('welcome.s3_title')}</h1>
          <p class="lead">{t('welcome.s3_text')}</p>
          <div class="reach">
            <div class="eyebrow">{t('welcome.in_reach')}</div>
            <div class="cards">
              {#each discovered as f (f.id)}
                <div class="card found">
                  <div class="head">
                    <span class="eyebrow"><Icon name="wifi" size={14} stroke={2} />{t('welcome.on_network')}</span>
                    <strong class="name">{f.name}</strong>
                  </div>
                  <button class="small accent" onclick={() => onadd(f)}>{t('welcome.add')}</button>
                </div>
              {/each}
              <div class="card">
                <div class="head">
                  <span class="eyebrow">{t('welcome.code_title')}</span>
                  <span class="text">{t('welcome.code_text')}</span>
                </div>
                <button class="small" onclick={onjoin}><Icon name="key" size={15} stroke={1.8} />{t('home.join')}</button>
              </div>
              <div class="card dashed">
                <div class="head">
                  <span class="eyebrow">{t('welcome.not_installed')}</span>
                  <span class="text">{t('welcome.not_installed_text')}</span>
                </div>
                <button class="link" onclick={download}>{t('welcome.station_link')} →</button>
              </div>
            </div>
          </div>
          {#if localError}<div class="error" role="alert">{localError}</div>{/if}
        {/if}
      </section>
    {/key}

    {#if step === 1}
      <!-- Un ordinateur, et les appareils qui le rejoignent. -->
      <svg class="art" width="400" height="330" viewBox="0 0 400 330" fill="none" aria-hidden="true">
        <rect x="110" y="20" width="200" height="130" rx="14" stroke="#9A8CFF" stroke-width="2.5" />
        <path d="M175 190h70M210 150v40" stroke="#9A8CFF" stroke-width="2.5" stroke-linecap="round" />
        <rect x="10" y="230" width="140" height="88" rx="12" stroke="#65737E" stroke-width="2" />
        <rect x="290" y="214" width="58" height="104" rx="12" stroke="#65737E" stroke-width="2" />
        <rect x="358" y="246" width="36" height="72" rx="8" stroke="#65737E" stroke-width="2" />
        <path d="M110 90C60 100 50 170 70 226M310 90c50 10 40 80 6 120" stroke="#9A8CFF" stroke-width="2" stroke-dasharray="4 7" stroke-linecap="round" />
      </svg>
    {/if}
  </div>

  <div class="dots" role="img" aria-label={t('welcome.step_label', { n: step })}>
    {#each [1, 2, 3] as n (n)}<span class:on={n === step}></span>{/each}
  </div>
</div>

<style>
  .welcome {
    position: absolute;
    inset: 0;
    z-index: 15;
    display: flex;
    flex-direction: column;
    padding: 52px clamp(32px, 5vw, 72px) clamp(24px, 4vh, 40px);
    background: radial-gradient(900px 520px at 78% 30%, #2a2160 0%, #120f2a 38%, var(--ink-0) 75%);
    overflow-y: auto;
    animation: fade 0.3s var(--ease-out);
  }

  .welcome.step2 {
    background: radial-gradient(900px 500px at 85% 10%, #231c55 0%, #0f0d24 40%, var(--ink-0) 75%);
  }

  .welcome.step3 {
    background: radial-gradient(800px 480px at 20% 100%, #1d1846 0%, #0c0b1e 45%, var(--ink-0) 75%);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .skip {
    height: 36px;
    padding: 0 14px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: rgb(9 13 18 / 0.7);
    color: var(--muted);
    font-size: 14px;
    white-space: nowrap;
  }

  .skip:hover {
    color: var(--text);
    border-color: var(--faint);
  }

  .body {
    flex: 1 0 auto;
    position: relative;
    display: flex;
    align-items: center;
    padding: clamp(20px, 5vh, 56px) 0;
  }

  .screen {
    position: relative;
    z-index: 1;
    width: 100%;
    max-width: 1040px;
    display: flex;
    flex-direction: column;
    gap: clamp(14px, 2.4vh, 22px);
    animation: rise 0.4s var(--ease-out);
  }

  .step1 .screen {
    max-width: 680px;
  }

  @keyframes rise {
    from {
      transform: translateY(14px);
      opacity: 0;
    }
  }

  .label {
    font-size: 12px;
    letter-spacing: 0.16em;
    color: var(--signal);
  }

  h1 {
    margin: 0;
    max-width: 760px;
    font-family: var(--display);
    font-size: clamp(40px, min(5.4vw, 8vh), 64px);
    line-height: 0.98;
    font-weight: 780;
    font-stretch: 125%;
    letter-spacing: -0.015em;
    text-transform: uppercase;
  }

  .lead {
    margin: 0;
    max-width: 640px;
    font-size: 18px;
    line-height: 1.45;
    color: #c9d3dc;
  }

  .art {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    width: min(400px, 34vw);
    height: auto;
  }

  .actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 6px;
  }

  /* Un bouton tient sur une ligne : la rangée passe à la ligne, jamais le libellé. */
  .actions button,
  .card button {
    white-space: nowrap;
  }

  .ghost.tall {
    height: 52px;
    padding: 0 22px;
    border-radius: 10px;
    background: rgb(14 20 26 / 0.85);
    color: var(--text);
    font-size: 15px;
    font-weight: 600;
  }

  .link {
    height: 40px;
    padding: 0 8px;
    border: 0;
    background: none;
    color: var(--signal);
    font-size: 15px;
  }

  .link:hover {
    color: #d4ceff;
  }

  .help {
    margin: 0;
    font-size: 13px;
    color: var(--faint);
  }

  .error {
    max-width: 640px;
  }

  .perks {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 16px;
    margin: 10px 0 8px;
  }

  .perk {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--ink-2);
  }

  .perk :global(svg) {
    color: var(--signal);
  }

  .perk .eyebrow,
  .card .eyebrow {
    letter-spacing: 0.12em;
  }

  .text {
    font-size: 14px;
    line-height: 1.45;
    color: #c9d3dc;
  }

  .reach {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: clamp(10px, 4vh, 40px);
  }

  .reach > .eyebrow {
    font-size: 12px;
    letter-spacing: 0.18em;
    color: var(--muted);
  }

  .cards {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(240px, 1fr);
    gap: 16px;
    overflow-x: auto;
    padding: 2px 2px 6px;
  }

  .card {
    min-height: 156px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 14px;
    padding: 18px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--ink-2);
  }

  .card.dashed {
    border-style: dashed;
    border-color: var(--line-2);
  }

  /* « SUR LE RÉSEAU » : la lueur des ordinateurs trouvés, comme sur l'accueil. */
  .card.found {
    border-color: var(--signal-strong);
    background: linear-gradient(160deg, #1b1640 0%, #0e1020 100%);
    box-shadow: 0 0 34px rgb(109 91 255 / 0.35);
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .head .eyebrow {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .name {
    font-size: 22px;
    font-weight: 700;
    font-stretch: 125%;
    overflow-wrap: anywhere;
  }

  .small {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 16px;
    border: 1px solid var(--line-2);
    border-radius: 9px;
    background: var(--ink-3);
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
  }

  .small.accent {
    border-color: var(--signal-strong);
    background: #241e4a;
  }

  .small:hover {
    border-color: var(--signal);
  }

  .card .link {
    align-self: flex-start;
    padding: 0;
    height: auto;
    font-weight: 600;
  }

  .dots {
    display: flex;
    gap: 8px;
  }

  .dots span {
    width: 10px;
    height: 4px;
    border-radius: 2px;
    background: var(--line-2);
    transition: width 0.3s var(--ease-out), background 0.3s;
  }

  .dots span.on {
    width: 26px;
    background: var(--signal);
  }

  /* Fenêtre étroite : le texte d'abord, sans le dessin. */
  @media (max-width: 1179px) {
    .art {
      display: none;
    }
  }

  /* Fenêtre basse : des avantages plus serrés, les boutons restent visibles. */
  @media (max-height: 720px) {
    .perk {
      gap: 8px;
      padding: 14px;
    }

    .perk .text {
      font-size: 13px;
    }
  }
</style>
