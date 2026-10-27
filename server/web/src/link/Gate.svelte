<!-- Le mot de passe de la console, une fois par navigateur et par 12 h. -->
<script lang="ts">
  import { backend } from './backend'
  import { errorText } from '../lib/store.svelte'
  import GateFrame from './GateFrame.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let { hostname, notice = '', onDone, onJoin }: { hostname: string | null; notice?: string; onDone: () => void; onJoin: () => void } = $props()

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

<GateFrame>
  <div class="head">
    {#if notice}<p class="notice" role="status">{notice}</p>{/if}
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
  <button type="button" class="ghost join" onclick={onJoin}>{t('share.join.button')}</button>
</GateFrame>

<style>
  .head .notice {
    align-self: flex-start;
    padding: 8px 14px;
    border-radius: 12px;
    font-size: 13.5px;
    color: var(--ok-text);
    background: var(--ok-bg);
    box-shadow: inset 0 0 0 1px var(--ok-line);
  }

  .join {
    align-self: flex-start;
    height: 44px;
  }
</style>
