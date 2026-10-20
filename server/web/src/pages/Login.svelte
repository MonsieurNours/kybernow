<!--
  Connexion à la console, ou première visite (choix du mot de passe avec le
  code d'initialisation). Le symbole immense, la couture allumée : la porte
  de l'ordinateur.
-->
<script lang="ts">
  import { api } from '../lib/api'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'

  let {
    configured,
    setupCodeFile,
    hostname,
    onDone,
  }: { configured: boolean; setupCodeFile: string; hostname: string | null; onDone: () => void } = $props()

  let code = $state('')
  let password = $state('')
  let confirm = $state('')
  let error = $state('')
  let busy = $state(false)

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    error = ''
    if (!configured && password !== confirm) {
      error = 'Les deux mots de passe ne correspondent pas.'
      return
    }
    busy = true
    try {
      if (configured) await api.login(password)
      else await api.setup(code.trim(), password)
      onDone()
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }
</script>

<div class="gate">
  <div class="symbol" aria-hidden="true">
    <svg class="halves" viewBox="0 0 48 48">
      <path fill="#0C1218" d="M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z" />
      <path fill="#0E141A" d="M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z" />
    </svg>
    <svg class="seam" viewBox="0 0 980 980">
      <defs>
        <filter id="h" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="22" /></filter>
        <filter id="g" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="5" /></filter>
      </defs>
      <g class="breathe">
        <line class="draw" x1="114" y1="866" x2="866" y2="114" stroke="#6D5BFF" stroke-width="60" stroke-linecap="round" opacity=".55" filter="url(#h)" />
        <line class="draw" x1="114" y1="866" x2="866" y2="114" stroke="#9A8CFF" stroke-width="14" stroke-linecap="round" filter="url(#g)" />
      </g>
      <line class="draw" x1="114" y1="866" x2="866" y2="114" stroke="#F4F1FF" stroke-width="4" stroke-linecap="round" />
    </svg>
  </div>
  <div class="veil" aria-hidden="true"></div>

  <div class="lockup">
    <Mark size={30} lit breathe />
    <span class="disp wordmark"><span>Kyber</span><b>Now</b></span>
    <span class="tag">Console de l'ordinateur</span>
  </div>

  <main>
    <div class="head">
      {#if configured}
        <span class="lbl host"><span class="dot ok"></span>{hostname ?? 'Cet ordinateur'}</span>
        <h1 class="disp">Bon retour.</h1>
        <p>Le mot de passe de la console, choisi à l'installation.</p>
      {:else}
        <span class="lbl first">Première visite</span>
        <h1 class="disp small">Choisissez le mot de passe de la console.</h1>
        <p>Le code prouve que vous êtes bien devant cet ordinateur, ou que vous y avez accès.</p>
      {/if}
    </div>

    <form onsubmit={submit}>
      {#if !configured}
        <div class="field">
          <label for="code">Code d'initialisation</label>
          <input id="code" class="mono code" type="text" autocomplete="one-time-code" required bind:value={code} />
          <span class="hint">Sur l'ordinateur : menu de l'icône KyberNow, ou le fichier {setupCodeFile}</span>
        </div>
      {/if}
      <div class="field">
        <label for="pw">{configured ? 'Mot de passe' : 'Nouveau mot de passe'}</label>
        <input id="pw" type="password" required minlength={configured ? undefined : 10} autocomplete={configured ? 'current-password' : 'new-password'} bind:value={password} />
        {#if !configured}<span class="hint">10 caractères au moins.</span>{/if}
      </div>
      {#if !configured}
        <div class="field">
          <label for="cf">Confirmation</label>
          <input id="cf" type="password" required minlength="10" autocomplete="new-password" bind:value={confirm} />
        </div>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <button type="submit" class="primary" disabled={busy}>
        {busy ? 'Un instant…' : configured ? 'Ouvrir la console' : 'Créer et ouvrir la console'}
      </button>
    </form>

    {#if configured}
      <details class="hint">
        <summary>Mot de passe oublié ?</summary>
        <p>Sur l'ordinateur, menu de l'icône KyberNow › Réinitialiser le mot de passe de la console. Vos appareils et leurs comptes ne changent pas.</p>
      </details>
    {/if}
  </main>

  <p class="foot hint">Connexion chiffrée par le certificat de cet ordinateur.</p>
</div>

<style>
  .gate {
    position: relative;
    min-height: 100vh;
    overflow: hidden;
    background: var(--ink-0);
  }

  .symbol {
    position: absolute;
    left: -120px;
    top: -40px;
    width: min(980px, 90vh);
    height: min(980px, 90vh);
  }

  .symbol svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .breathe {
    animation: respire 3.6s ease-in-out infinite;
  }

  .draw {
    stroke-dasharray: 1100;
    animation: seam 1.4s 0.2s var(--ease-out) both;
  }

  @keyframes seam {
    from {
      stroke-dashoffset: 1100;
    }
    to {
      stroke-dashoffset: 0;
    }
  }

  .veil {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, rgb(5 7 10 / 0) 30%, rgb(5 7 10 / 0.9) 62%);
  }

  .lockup {
    position: absolute;
    left: 64px;
    top: 52px;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .wordmark {
    font-size: 18px;
    letter-spacing: -0.01em;
  }

  .wordmark span {
    font-weight: 420;
  }

  .wordmark b {
    font-weight: 720;
  }

  .tag {
    font-size: 13px;
    color: var(--faint);
  }

  main {
    position: relative;
    margin-left: auto;
    margin-right: max(24px, 10vw);
    width: min(420px, calc(100% - 48px));
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 28px;
    padding: 120px 0 80px;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .host {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
  }

  .first {
    color: var(--signal-light);
  }

  h1 {
    font-size: 46px;
    line-height: 1.02;
    font-weight: 650;
    letter-spacing: -0.035em;
    text-wrap: balance;
  }

  h1.small {
    font-size: 40px;
  }

  .head p {
    font-size: 15px;
    color: var(--muted);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field label {
    font-size: 13px;
    color: var(--text-2);
  }

  .field input {
    height: 46px;
    padding: 0 14px;
    font-size: 15px;
    border-radius: 10px;
    background: rgb(14 20 26 / 0.9);
  }

  .code {
    letter-spacing: 0.12em;
  }

  .hint {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--faint);
    overflow-wrap: anywhere;
  }

  .primary {
    height: 48px;
    font-size: 15px;
  }

  .error {
    font-size: 13.5px;
    color: var(--bad-text);
  }

  summary {
    cursor: pointer;
    color: var(--muted);
  }

  details p {
    margin-top: 8px;
  }

  .foot {
    position: absolute;
    left: 64px;
    bottom: 40px;
  }

  @media (max-width: 760px) {
    .symbol {
      left: -200px;
      top: -260px;
      opacity: 0.6;
    }

    .veil {
      background: linear-gradient(180deg, rgb(5 7 10 / 0.2) 0%, rgb(5 7 10 / 0.92) 40%);
    }

    .lockup {
      left: 24px;
      top: 28px;
    }

    .tag {
      display: none;
    }

    main {
      margin: 0 auto;
      padding-top: 160px;
    }

    h1,
    h1.small {
      font-size: 34px;
    }

    .foot {
      position: static;
      padding: 0 24px 32px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .breathe,
    .draw {
      animation: none;
    }
  }
</style>
