<script lang="ts">
  import { go } from '../lib/state.svelte'
</script>

<span class="eyebrow kn-enter">Étape 1 sur 6</span>
<h1 class="kn-enter kn-d1" style="font-size: 40px; line-height: 1.08; max-width: 620px">
  Ce PC devient votre machine de jeu et de travail, partout chez vous.
</h1>
<p class="lead kn-enter kn-d2" style="font-size: 16px">
  Depuis votre Mac ou votre Apple TV, vous retrouvez l'écran de ce PC comme s'il était branché devant vous : l'image
  arrive en quelques millisecondes, votre clavier, votre souris et votre manette répondent instantanément.
</p>

<div class="schema card kn-enter kn-d3">
  <div class="pc">
    <svg width="40" height="40" viewBox="0 0 40 40" aria-hidden="true"><rect x="7" y="5" width="26" height="30" rx="3" fill="none" stroke="#E9EEF5" stroke-width="1.8" /><rect class="kn-glow" x="12" y="10" width="16" height="7" rx="1.5" fill="none" stroke="#6FE3C1" stroke-width="1.6" /><circle cx="20" cy="27" r="3" fill="none" stroke="#E9EEF5" stroke-width="1.6" /></svg>
    <strong>Ce PC</strong>
    <span class="mono">GPU NVIDIA · NVENC</span>
  </div>
  <div class="links">
    <span class="mono">image et son</span>
    <div class="flow kn-flow"></div>
    <div class="flow back kn-flow"></div>
    <span class="mono">clavier, souris, manette</span>
  </div>
  <div class="devices">
    <div class="device kn-enter kn-d4">
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true"><rect x="3" y="4" width="14" height="9" rx="1.5" fill="none" stroke="#E9EEF5" stroke-width="1.5" /><path d="M1.5 16h17" stroke="#E9EEF5" stroke-width="1.5" stroke-linecap="round" /></svg>
      Mac
    </div>
    <div class="device kn-enter kn-d5">
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true"><rect x="2" y="4" width="16" height="10" rx="1.5" fill="none" stroke="#E9EEF5" stroke-width="1.5" /><path d="M7 17h6" stroke="#E9EEF5" stroke-width="1.5" stroke-linecap="round" /></svg>
      Apple TV
    </div>
    <div class="device kn-enter kn-d6">
      <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.5" fill="none" stroke="#E9EEF5" stroke-width="1.5" /><path d="M2.5 10h15M10 2.5c2.2 2.2 2.2 12.8 0 15M10 2.5c-2.2 2.2-2.2 12.8 0 15" fill="none" stroke="#E9EEF5" stroke-width="1.3" /></svg>
      Navigateur web
    </div>
  </div>
</div>

<div class="pillars kn-enter kn-d4">
  <div>
    <strong>Le serveur de streaming</strong>
    <span>Capture l'écran, l'encode avec votre carte graphique et l'envoie à vos appareils.</span>
  </div>
  <div>
    <strong>La console d'administration</strong>
    <span>Une page web pour tout régler depuis n'importe quel appareil, sans toucher à un fichier.</span>
  </div>
  <div>
    <strong>Deux pilotes optionnels</strong>
    <span>Manettes virtuelles et écran virtuel adapté à l'appareil qui se connecte.</span>
  </div>
</div>

<div class="footer">
  <span class="note">Environ 5 minutes · 220 Mo · droits administrateur requis</span>
  <div class="spacer"></div>
  <button class="btn primary" onclick={() => go('checks')}>
    Commencer
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
</div>

<style>
  .mono {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--muted);
  }

  .schema {
    margin-top: 30px;
    height: 168px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 28px;
    background: rgba(18, 25, 34, 0.86);
  }

  .pc {
    width: 168px;
    height: 112px;
    border-radius: 12px;
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    font-size: 13px;
  }

  .links {
    flex: 1;
    max-width: 180px;
    margin: 0 12px;
    white-space: nowrap;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .flow {
    width: 100%;
    height: 2px;
    background-image: repeating-linear-gradient(90deg, var(--accent) 0 10px, transparent 10px 16px);
    background-size: 32px 2px;
  }

  .flow.back {
    background-image: repeating-linear-gradient(270deg, #4a5a70 0 10px, transparent 10px 16px);
    animation-direction: reverse;
  }

  .devices {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .device {
    width: 196px;
    height: 40px;
    border-radius: 10px;
    background: var(--surface-2);
    border: 1px solid var(--line-2);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    font-size: 13px;
  }

  .pillars {
    margin-top: 22px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }

  .pillars div {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pillars strong {
    font-size: 14px;
  }

  .pillars span {
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }

  .note {
    font-size: 13px;
    color: var(--muted);
  }
</style>
