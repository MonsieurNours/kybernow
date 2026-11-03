<!--
  Étape 2 quand KyberNow Station est déjà sur ce Mac : garder ses réglages
  (comptes, appareils, ports, certificat) ou repartir de zéro. Même choix
  que sous Windows, à la manière des écrans de la maquette.
-->
<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte'
  import { app, go } from '../state.svelte'

  const existing = $derived(app.existing!)
  const canKeep = $derived(existing.settings)
</script>

<div class="reinstall">
  <span class="lbl-m kn-enter">{t('mac.account.label')}</span>
  <h1 class="t kn-enter kn-d1">{t('mac.reinstall.title', { version: existing.version })}</h1>
  <p class="lead-m kn-enter kn-d2">{t('mac.reinstall.lead')}</p>

  <div class="choices kn-enter kn-d2" role="radiogroup" aria-label={t('reinstall.group')}>
    <button type="button" role="radio" class="card-m choice" class:on={app.keepSettings && canKeep} aria-checked={app.keepSettings && canKeep} disabled={!canKeep} onclick={() => (app.keepSettings = true)}>
      <span class="dot" aria-hidden="true"></span>
      <span class="text">
        <strong>{t('mac.reinstall.keep')}</strong>
        <span>
          {#if canKeep}
            {t('mac.reinstall.keepDetail')}
            <span class="mono-m">{t('mac.checks.portsDetail', { stream: existing.streamPort, console: existing.consolePort })}</span>
          {:else}
            {t('mac.reinstall.keepImpossible')}
          {/if}
        </span>
      </span>
    </button>
    <button type="button" role="radio" class="card-m choice" class:on={!app.keepSettings || !canKeep} aria-checked={!app.keepSettings || !canKeep} onclick={() => (app.keepSettings = false)}>
      <span class="dot" aria-hidden="true"></span>
      <span class="text">
        <strong>{t('mac.reinstall.fresh')}</strong>
        <span>{t('mac.reinstall.freshDetail')}</span>
      </span>
    </button>
  </div>
</div>

<div class="foot-m end">
  <div class="buttons">
    <button type="button" class="btn" onclick={() => go('welcome')}>{t('common.back')}</button>
    <button type="button" class="btn primary-m" onclick={() => go(app.keepSettings && canKeep ? 'options' : 'account')}>{t('common.continue')}</button>
  </div>
</div>

<style>
  .reinstall {
    padding-top: 8px;
    display: flex;
    flex-direction: column;
  }

  .choices {
    margin-top: 26px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .choice {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
    padding: 16px 18px;
    text-align: left;
    color: var(--text);
  }

  .choice.on {
    border-color: var(--signal);
    box-shadow: 0 0 0 3px rgb(154 140 255 / 0.14);
  }

  .dot {
    width: 18px;
    height: 18px;
    margin-top: 2px;
    border-radius: 50%;
    border: 1px solid #3b4955;
  }

  .on .dot {
    border: 5px solid var(--signal-strong);
    background: #fff;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .text strong {
    font-size: 14.5px;
    font-weight: 600;
  }

  .text > span {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--faint);
  }

  .mono-m {
    font-size: 11.5px;
    color: var(--muted);
  }
</style>
