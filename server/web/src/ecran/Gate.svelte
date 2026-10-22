<!-- Le mot de passe de la console, une fois par navigateur et par 12 h. -->
<script lang="ts">
  import { backend } from './backend'
  import { errorText } from '../lib/store.svelte'
  import Mark from '../lib/Mark.svelte'
  import { t } from '../lib/i18n/index.svelte'

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
    <Mark product="link" size={26} />
  </header>

  <main>
    <div class="head">
      <span class="lbl host"><span class="dot"></span>{hostname ?? t('web.app.thisComputer')}</span>
      <h1 class="disp">{t('web.gate.title')}</h1>
      <p>{t('web.gate.lead')}</p>
    </div>
    <form onsubmit={submit}>
      <input type="text" name="username" autocomplete="username" value="console" hidden />
      <label class="sr-only" for="pw">{t('web.gate.password')}</label>
      <input id="pw" type="password" placeholder={t('web.gate.password')} required autocomplete="current-password" bind:value={password} />
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <button type="submit" class="primary" disabled={busy}>{busy ? t('web.gate.wait') : t('web.gate.continue')}</button>
    </form>
    <p class="hint">{t('web.gate.hint')}</p>
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
