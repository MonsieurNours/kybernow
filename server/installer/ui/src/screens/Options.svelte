<script lang="ts">
  import Toggle from '../lib/Toggle.svelte'
  import { app, go, startInstall } from '../lib/state.svelte'

  let phase = $state(0)
  $effect(() => {
    const timer = setInterval(() => (phase = (phase + 1) % 3), 2200)
    return () => clearInterval(timer)
  })

  const phases = [
    ['1 · À la connexion', "Un écran virtuel est créé, par exemple en 3840 × 2160 à 60 Hz pour l'Apple TV."],
    ['2 · Pendant la session', "Les écrans branchés à l'ordinateur s'éteignent : personne ne voit ce que vous faites."],
    ['3 · À la déconnexion', "L'écran virtuel disparaît, vos écrans reviennent avec leur disposition."],
  ]
</script>

<span class="eyebrow kn-enter">Étape 4 sur 6</span>
<h1 class="kn-enter kn-d1">Choisissez vos options.</h1>
<p class="lead kn-enter kn-d2" style="margin-top: 8px">Tout est modifiable plus tard dans la console d'administration.</p>

<section class="card vdd kn-enter kn-d3" class:on={app.options.virtualDisplay}>
  <div class="head">
    <div class="text">
      <div class="title">
        <strong>Écran virtuel adapté à chaque appareil</strong>
        <span class="tag">recommandé</span>
      </div>
      <span>Votre Mac ou votre Apple TV reçoit une image exactement à sa résolution et à sa fréquence, sans bandes noires ni mise à l'échelle.</span>
    </div>
    <Toggle bind:checked={app.options.virtualDisplay} label="Écran virtuel" />
  </div>
  <div class="phases">
    {#each phases as [label, text], i (label)}
      <div class="phase" class:active={app.options.virtualDisplay && i === phase}>
        <span class="mono">{label}</span>
        <span>{text}</span>
      </div>
    {/each}
  </div>
  <div class="warn">
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1.8l5.5 2.2v4c0 3.2-2.3 5.6-5.5 6.4C4.8 13.6 2.5 11.2 2.5 8V4z" fill="none" stroke="#F5B65A" stroke-width="1.4" stroke-linejoin="round" /></svg>
    <span>Ce pilote libre (SudoVDA) n'est pas signé par Microsoft : son certificat sera ajouté aux autorités de confiance de Windows.</span>
  </div>
</section>

<div class="grid kn-enter kn-d4">
  <section class="card small" class:on={app.options.gamepads}>
    <div class="row"><strong>Manettes</strong><Toggle bind:checked={app.options.gamepads} label="Manettes" /></div>
    <span>Votre manette Xbox ou PlayStation est vue par l'ordinateur comme une vraie manette (pilote ViGEmBus).</span>
  </section>
  <section class="card small" class:on={app.options.virtualAudio} class:off={!app.steamSpeakers}>
    <div class="row"><strong>Son à distance</strong><Toggle bind:checked={app.options.virtualAudio} label="Son à distance" disabled={!app.steamSpeakers} /></div>
    {#if app.steamSpeakers}
      <span>Pendant le stream, l'ordinateur se tait et le son part vers votre appareil en stéréo, 5.1 ou 7.1 (haut-parleurs virtuels de Steam).</span>
    {:else}
      <span>Demande Steam, qui fournit des haut-parleurs virtuels signés. Sans lui, le son sort aussi de l'ordinateur.</span>
    {/if}
  </section>
  <section class="card small" class:on={app.options.maxPerformance}>
    <div class="row"><strong>Performances maximales</strong><Toggle bind:checked={app.options.maxPerformance} label="Performances maximales" /></div>
    <span>Empêche le processeur et le GPU de ralentir pendant le stream, pour une latence stable.</span>
  </section>
  <section class="card small" class:on={app.options.startWithWindows}>
    <div class="row"><strong>Démarrer avec Windows</strong><Toggle bind:checked={app.options.startWithWindows} label="Démarrer avec Windows" /></div>
    <span>Le serveur est prêt dès l'allumage de l'ordinateur, même avant votre connexion à Windows. L'icône KyberNow s'affiche près de l'horloge à l'ouverture de votre session.</span>
  </section>
</div>

<div class="footer">
  <button class="btn" onclick={() => go('account')}>Retour</button>
  <div class="spacer"></div>
  {#if app.options.gamepads}<span class="note">Un redémarrage sera peut-être nécessaire pour les manettes</span>{/if}
  <button class="btn primary" onclick={startInstall}>
    Installer
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
</div>

<style>
  .card {
    transition: border-color 0.3s;
  }

  .card.on {
    border-color: #2b4a42;
  }

  .vdd {
    margin-top: 20px;
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }

  .title {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .title strong {
    font-size: 16px;
    color: var(--text);
  }

  .tag {
    font-family: var(--mono);
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .phases {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .phase {
    padding: 12px;
    border-radius: 10px;
    background: var(--surface-2);
    border: 1px solid transparent;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    line-height: 1.45;
    transition: background-color 0.4s, border-color 0.4s;
  }

  .phase.active {
    background: #1a2a2a;
    border-color: var(--accent);
  }

  .mono {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--accent);
  }

  .vdd:not(.on) .mono {
    color: var(--faint);
  }

  .warn {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    font-size: 12px;
    line-height: 1.5;
    color: var(--warn-text);
  }

  .warn svg {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .grid {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }

  .small {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .row strong {
    flex: 1;
    font-size: 14px;
    color: var(--text);
  }

  .note {
    font-size: 13px;
    color: var(--muted);
  }
</style>
