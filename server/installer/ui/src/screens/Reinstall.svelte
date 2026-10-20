<!--
  Étape 2 quand KyberNow est déjà installé : garder ses réglages (comptes,
  appareils appairés, ports, certificat) ou repartir de zéro. « Repartir de
  zéro » passe ensuite par « Votre compte », comme une première installation.
-->
<script lang="ts">
  import { app, go } from '../lib/state.svelte'

  const existing = $derived(app.existing!)
  const canKeep = $derived(existing.settings)

  function next() {
    go(app.keepSettings && canKeep ? 'options' : 'account')
  }
</script>

<div class="reinstall">
  <div class="head">
    <span class="lbl signal kn-enter">Réinstallation</span>
    <h1 class="title kn-enter kn-d1">KyberNow {existing.version} est déjà là.</h1>
    <p class="lead kn-enter kn-d2">Vos appareils connaissent déjà cet ordinateur. Que faisons-nous de ses réglages ?</p>
  </div>

  <div class="choices kn-enter kn-d2" role="radiogroup" aria-label="Réglages de KyberNow">
    <button
      type="button"
      role="radio"
      class="choice"
      class:on={app.keepSettings && canKeep}
      aria-checked={app.keepSettings && canKeep}
      disabled={!canKeep}
      onclick={() => (app.keepSettings = true)}
    >
      <span class="dot" aria-hidden="true"></span>
      <span class="text">
        <strong>Conserver mes réglages {#if canKeep}<span class="tag">recommandé</span>{/if}</strong>
        {#if canKeep}
          <span>
            Comptes, appareils appairés, certificat, réglages de la console et ports
            (<span class="mono">flux {existing.streamPort} · console {existing.consolePort}</span>) restent tels quels. Seuls les programmes, les pilotes et les
            correctifs de Kyber sont remplacés.
          </span>
        {:else}
          <span>Impossible ici : les réglages de cette installation sont introuvables.</span>
        {/if}
      </span>
    </button>

    <button
      type="button"
      role="radio"
      class="choice"
      class:on={!app.keepSettings || !canKeep}
      aria-checked={!app.keepSettings || !canKeep}
      onclick={() => (app.keepSettings = false)}
    >
      <span class="dot" aria-hidden="true"></span>
      <span class="text">
        <strong>Repartir de zéro</strong>
        <span>Nouveau compte, nouveaux ports, nouveau certificat, réglages par défaut.</span>
      </span>
    </button>
  </div>

  {#if !app.keepSettings || !canKeep}
    <div class="notice warn kn-enter">
      <span>
        Tous vos appareils (Mac, Apple TV, navigateurs) perdront l'accès : il faudra les appairer de nouveau avec le nouveau compte. Les réglages de la console
        et de Kyber reviennent à ceux d'une première installation.
      </span>
    </div>
  {/if}
</div>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go('welcome')}>Retour</button>
  <div class="spacer"></div>
  <button type="button" class="primary" onclick={next}>Continuer</button>
</div>

<style>
  .reinstall {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
    max-width: 720px;
    padding-top: 24px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .head h1 {
    font-size: 32px;
  }

  .choices {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .choice {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
    width: 100%;
    padding: 14px 18px;
    border: 0;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      box-shadow 0.2s,
      background-color 0.2s;
  }

  .choice.on {
    background: rgb(109 91 255 / 0.1);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.55);
  }

  .choice:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .dot {
    width: 16px;
    height: 16px;
    margin-top: 2px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--faint);
  }

  .choice.on .dot {
    background: radial-gradient(circle, var(--signal-light) 0 4px, transparent 4.5px);
    box-shadow: inset 0 0 0 1.5px var(--signal-light);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  strong {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 15px;
    font-weight: 600;
  }

  .text > span {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--faint);
  }

  .mono {
    font-family: var(--mono);
    font-size: 11.5px;
    white-space: nowrap;
  }

  .tag {
    padding: 2px 8px;
    border-radius: 999px;
    background: rgb(154 140 255 / 0.16);
    color: var(--signal-light);
    font-size: 11px;
    font-weight: 600;
  }
</style>
