<!-- Étape 2 · Le compte unique : se connecter depuis les appareils et ouvrir KyberNow Admin. -->
<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte'
  import { app, go } from '../state.svelte'

  let show = $state(false)
  let touched = $state(false)
  let userInput: HTMLInputElement | undefined = $state()

  $effect(() => {
    const timer = setTimeout(() => userInput?.focus(), 150)
    return () => clearTimeout(timer)
  })

  // Même barème que l'installateur Windows.
  const score = $derived.by(() => {
    const p = app.password
    let s = 0
    if (p.length >= 10) s++
    if (p.length >= 14) s++
    if (/[^a-zA-Z]/.test(p)) s++
    if (p.length >= 18 || (/[A-Z]/.test(p) && /[0-9]/.test(p))) s++
    return s
  })
  const labels = ['short', 'fair', 'good', 'veryGood', 'excellent']
  const strength = $derived(t(`mac.account.strength.${app.password.length < 10 ? labels[0] : labels[score]}`))
  const color = $derived(app.password.length < 10 ? '#F5B841' : score >= 2 ? '#4CD68A' : '#9A8CFF')

  const userError = $derived.by(() => {
    const u = app.user.trim()
    if (!u) return t('account.userEmpty')
    if (u.includes(':')) return t('account.userColon')
    if (u.length > 64) return t('account.userLong')
    return null
  })
  const passError = $derived(app.password.length < 10 ? t('account.passwordShort') : null)

  function next(event?: SubmitEvent) {
    event?.preventDefault()
    touched = true
    if (!userError && !passError) go('options')
  }
</script>

<form class="account" id="account" onsubmit={next}>
  <span class="lbl-m kn-enter">{t('mac.account.label')}</span>
  <h1 class="t kn-enter kn-d1">{t('mac.account.title')}</h1>
  <p class="lead-m kn-enter kn-d2">{t('mac.account.lead')}</p>

  <div class="fields kn-enter kn-d2">
    <label>
      {t('account.user')}
      <input bind:this={userInput} bind:value={app.user} autocomplete="username" spellcheck="false" aria-invalid={touched && !!userError} />
      {#if touched && userError}<span class="err">{userError}</span>{/if}
    </label>
    <label>
      {t('account.password')}
      <span class="pass">
        <input type={show ? 'text' : 'password'} bind:value={app.password} autocomplete="new-password" aria-invalid={touched && !!passError} />
        <button type="button" onclick={() => (show = !show)}>{show ? t('account.hide') : t('account.show')}</button>
      </span>
      {#if touched && passError}<span class="err">{passError}</span>{/if}
    </label>
  </div>
  <div class="meter kn-enter kn-d3">
    <span class="bars" aria-hidden="true">
      {#each [1, 2, 3, 4] as i (i)}<span style:background={app.password.length && (app.password.length < 10 ? i === 1 : score >= i) ? color : '#1E2832'}></span>{/each}
    </span>
    <span class="hint">{app.password ? t('mac.account.hint', { strength }) : t('mac.account.hintEmpty')}</span>
  </div>

  <p class="lbl-s uses kn-enter kn-d3">{t('mac.account.uses')}</p>
  <div class="cards kn-enter kn-d3">
    <div class="card-m">
      <strong>{t('mac.account.devices')}</strong>
      <span>{t('mac.account.devicesDetail')}</span>
    </div>
    <div class="card-m">
      <strong>{t('mac.account.console')}</strong>
      <span>{t('mac.account.consoleDetail')}</span>
    </div>
  </div>
</form>

<div class="foot-m end">
  <div class="buttons">
    <button type="button" class="btn" onclick={() => go(app.existing ? 'reinstall' : 'welcome')}>{t('common.back')}</button>
    <button type="submit" form="account" class="btn primary-m">{t('common.continue')}</button>
  </div>
</div>

<style>
  .account {
    padding-top: 8px;
    display: flex;
    flex-direction: column;
  }

  .lead-m {
    margin-top: 14px;
  }

  .fields {
    margin-top: 28px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
  }

  input {
    width: 100%;
    height: 42px;
    padding: 0 14px;
    border-radius: 10px;
    border: 1px solid #2b3742;
    background: #0e141a;
    color: var(--text);
    font: inherit;
    font-size: 14.5px;
    outline: none;
    user-select: text;
  }

  input:focus {
    border-color: var(--signal);
  }

  input[aria-invalid='true'] {
    border-color: var(--bad);
  }

  .pass {
    position: relative;
    display: block;
  }

  .pass input {
    padding-right: 86px;
  }

  .pass button {
    position: absolute;
    right: 6px;
    top: 6px;
    height: 30px;
    padding: 0 10px;
    border: 0;
    border-radius: 6px;
    background: #141b23;
    color: var(--muted);
    font-size: 12.5px;
  }

  .err {
    font-size: 12.5px;
    color: var(--bad-text);
  }

  .meter {
    margin-top: 10px;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .bars {
    display: flex;
    gap: 4px;
    flex: none;
  }

  .bars span {
    width: 34px;
    height: 4px;
    border-radius: 2px;
    transition: background-color 0.2s;
  }

  .hint {
    font-size: 12.5px;
    color: var(--faint);
  }

  .uses {
    margin: 30px 0 12px;
    font-size: 12px;
    letter-spacing: 0.06em;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }

  .cards .card-m {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cards strong {
    font-size: 14.5px;
    font-weight: 600;
  }

  .cards span {
    font-size: 13px;
    line-height: 1.5;
    color: var(--faint);
  }
</style>
