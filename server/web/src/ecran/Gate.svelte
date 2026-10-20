<!-- Le mot de passe de la console, une fois par navigateur et par 12 h. -->
<script lang="ts">
  import { backend } from './backend'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'

  let { hostname, onDone }: { hostname: string | null; onDone: () => void } = $props()

  let password = $state('')
  let error = $state('')
  let busy = $state(false)

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    error = ''
    busy = true
    try {
      await backend.login(password)
      onDone()
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }
</script>

<div class="gate">
  <header>
    <Mark size={26} state="now" />
    <span class="disp wordmark"><span>Kyber</span><b>Now</b></span>
  </header>

  <main>
    <div class="head">
      <span class="lbl host"><span class="dot"></span>{hostname ?? 'Cet ordinateur'}</span>
      <h1 class="disp">Votre ordinateur, dans ce navigateur.</h1>
      <p>Le mot de passe de la console, choisi à l'installation.</p>
    </div>
    <form onsubmit={submit}>
      <input type="text" name="username" autocomplete="username" value="console" hidden />
      <label class="sr-only" for="pw">Mot de passe</label>
      <input id="pw" type="password" placeholder="Mot de passe" required autocomplete="current-password" bind:value={password} />
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <button type="submit" class="primary" disabled={busy}>{busy ? 'Un instant…' : 'Continuer'}</button>
    </form>
    <p class="hint">Rien à installer. Connexion chiffrée par le certificat de cet ordinateur.</p>
  </main>
</div>

<style>
  .gate {
    min-height: 100vh;
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(60% 50% at 80% 10%, rgb(109 91 255 / 0.16), transparent 70%),
      var(--ink-0);
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 28px max(24px, 5vw);
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

  main {
    flex: 1;
    width: min(440px, calc(100% - 48px));
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 26px;
    padding-bottom: 12vh;
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

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
  }

  h1 {
    font-size: clamp(32px, 6vw, 44px);
    line-height: 1.02;
    font-weight: 650;
    letter-spacing: -0.035em;
    text-wrap: balance;
  }

  .head p {
    color: var(--muted);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  input {
    height: 50px;
    padding: 0 16px;
    font-size: 16px;
    border-radius: 12px;
  }

  .primary {
    height: 52px;
    font-size: 16px;
  }

  .error {
    font-size: 13.5px;
    color: var(--bad-text);
  }

  .hint {
    font-size: 12.5px;
    color: var(--faint);
  }
</style>
