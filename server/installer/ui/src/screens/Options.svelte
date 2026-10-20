<!-- Étape 3 · Les options, déjà réglées au mieux. -->
<script lang="ts">
  import Toggle from '../lib/Toggle.svelte'
  import { app, go, startInstall } from '../lib/state.svelte'
</script>

<div class="options">
  <div class="head">
    <span class="lbl signal kn-enter">Options</span>
    <h1 class="title kn-enter kn-d1">Tout est déjà réglé au mieux.</h1>
    <p class="lead kn-enter kn-d2">Tout se change plus tard dans la console.</p>
  </div>

  <div class="list kn-enter kn-d2">
    <div class="opt">
      <div>
        <h3>Écran virtuel adapté à chaque appareil <span class="tag">recommandé</span></h3>
        <p>L'image arrive à la définition et à la fréquence de l'appareil. Le pilote libre SudoVDA n'est pas signé par Microsoft : son certificat sera ajouté aux autorités de confiance.</p>
      </div>
      <Toggle bind:checked={app.options.virtualDisplay} label="Écran virtuel" />
    </div>
    <div class="opt">
      <div>
        <h3>Manettes</h3>
        <p>Xbox ou PlayStation, vues comme de vraies manettes (pilote ViGEmBus). Un redémarrage sera peut-être demandé.</p>
      </div>
      <Toggle bind:checked={app.options.gamepads} label="Manettes" />
    </div>
    <div class="opt" class:off={!app.steamSpeakers}>
      <div>
        <h3>Son à distance</h3>
        <p>
          {app.steamSpeakers
            ? "L'ordinateur se tait, le son part vers l'appareil, jusqu'au 7.1. Utilise les haut-parleurs de Steam, trouvés sur cet ordinateur."
            : "Demande Steam, qui fournit des haut-parleurs virtuels signés. Sans lui, le son sort aussi de l'ordinateur."}
        </p>
      </div>
      <Toggle bind:checked={app.options.virtualAudio} label="Son à distance" disabled={!app.steamSpeakers} />
    </div>
    <div class="opt">
      <div>
        <h3>Démarrer avec l'ordinateur</h3>
        <p>Disponible avant même l'ouverture de votre session Windows. L'icône KyberNow s'affiche près de l'horloge.</p>
      </div>
      <Toggle bind:checked={app.options.startWithWindows} label="Démarrer avec l'ordinateur" />
    </div>
    <div class="opt">
      <div>
        <h3>Performances maximales</h3>
        <p>
          {app.powerNeedsTuning
            ? 'Le processeur et la carte graphique ne ralentissent pas pendant une session : une latence stable.'
            : 'Déjà réglé : cet ordinateur est en mode Performances élevées.'}
        </p>
      </div>
      <Toggle bind:checked={app.options.maxPerformance} label="Performances maximales" />
    </div>
  </div>
</div>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go('account')}>Retour</button>
  <div class="spacer"></div>
  <button type="button" class="primary" onclick={startInstall}>Installer</button>
</div>

<style>
  .options {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 760px;
    padding-top: 12px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .head h1 {
    font-size: 30px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-bottom: 12px;
  }

  .opt {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px 22px;
    align-items: center;
    padding: 11px 18px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }

  .opt > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  h3 {
    font-size: 15px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .tag {
    padding: 2px 8px;
    border-radius: 999px;
    background: rgb(154 140 255 / 0.16);
    color: var(--signal-light);
    font-size: 11px;
    font-weight: 600;
  }

  p {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--faint);
  }

  .opt.off {
    opacity: 0.6;
  }
</style>
