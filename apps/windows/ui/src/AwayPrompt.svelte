<!--
  « Hors de chez vous ? » (docs/link-ui.md « Accueil et compte KyberNow ») : un ordinateur appairé qui n'est pas sur ce
  réseau et que le relais ne tient pas, sans compte. « Pas maintenant », « Créer un compte » (la page d'inscription
  dans le navigateur), « Connecter un compte » (la connexion de toujours). Une fois par ordinateur et par jour au plus
  (`welcome_away_hint`, côté app).
-->
<script lang="ts">
  import Sheet from './lib/Sheet.svelte'
  import Icon from './lib/Icon.svelte'
  import { t } from './lib/i18n/index.svelte'

  let { name, onsignin, onsignup, onclose }: { name: string; onsignin: () => void; onsignup: () => void; onclose: () => void } = $props()
</script>

<Sheet onclose={onclose} shade="rgb(2 3 5 / 0.62)">
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="away-title" aria-describedby="away-text">
    <div class="top">
      <span class="badge"><Icon name="globe" size={28} stroke={1.8} /></span>
      <div class="words">
        <h2 id="away-title">{t('welcome.away_dialog_title')}</h2>
        <p id="away-text">{t('welcome.away_dialog_text', { name })}</p>
      </div>
    </div>
    <div class="buttons">
      <button class="plain" onclick={onclose}>{t('welcome.not_now')}</button>
      <button class="ghost" onclick={onsignup}>{t('welcome.create_account')}</button>
      <!-- svelte-ignore a11y_autofocus -->
      <button class="primary" autofocus onclick={onsignin}>{t('account.sign_in')}</button>
    </div>
  </div>
</Sheet>

<style>

  .sheet {
    width: min(640px, 100%);
    display: flex;
    flex-direction: column;
    gap: 26px;
    padding: 34px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-0);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.6);
    animation: sheet-rise 0.32s var(--ease-out);
  }

  .top {
    display: flex;
    gap: 18px;
    align-items: flex-start;
  }

  .badge {
    flex: none;
    width: 64px;
    height: 64px;
    border-radius: 16px;
    background: #171431;
    color: var(--signal);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h2 {
    margin: 0;
    font-size: 30px;
    line-height: 1.05;
    font-weight: 700;
    font-stretch: 125%;
  }

  p {
    margin: 0;
    font-size: 16px;
    line-height: 1.45;
    color: #c9d3dc;
    user-select: text;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 12px;
  }

  .buttons button {
    white-space: nowrap;
  }

  .ghost {
    height: 52px;
    padding: 0 20px;
    border-radius: 10px;
    background: var(--ink-2);
    color: var(--text);
    font-size: 15px;
    font-weight: 600;
  }

  .plain {
    height: 52px;
    padding: 0 10px;
    border: 0;
    background: none;
    color: #c9d3dc;
    font-size: 15px;
  }

  .plain:hover {
    color: var(--text);
  }
</style>
