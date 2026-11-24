<!--
  Étape « Compte KyberNow », pendant l'installation (maquette, rangée
  « Installateurs ») : le choix, « Autre relais » vérifié en direct,
  l'attente d'Admin, puis la page du relais dans la fenêtre (l'installateur
  pose sa vue sur le cadre). Windows et Mac.
-->
<script lang="ts">
  import { onDestroy } from 'svelte'
  import { t } from './i18n/index.svelte'
  import {
    account,
    backToChoice,
    DEFAULT_RELAY,
    defaultRelay,
    failureText,
    handleAccount,
    hideView,
    later,
    openRelay,
    showView,
    startAccount,
    typeRelay,
    useRelay,
  } from './account.svelte'

  interface Props {
    percent: number
    /** Dernière ligne du journal de l'installation. */
    lastLine: string
    /** Vue de l'installateur (sinon, dans un navigateur : une page imitée). */
    native: boolean
    openUrl: (url: string) => void
  }

  let { percent, lastLine, native, openUrl }: Props = $props()

  let frame: HTMLDivElement | undefined = $state()
  let relayField: HTMLInputElement | undefined = $state()
  const finished = $derived(percent >= 100)
  const progressLabel = $derived(finished ? t('accountStep.installed') : t('accountStep.progress', { percent: Math.round(percent) }))
  const probe = $derived(account.probe)

  // La vue de l'installateur suit le cadre (après l'entrée de l'écran, puis à chaque changement de taille).
  $effect(() => {
    if (!frame || account.phase !== 'page' || !account.page || account.page.external || !native) return
    const el = frame
    const place = () => showView(el.getBoundingClientRect())
    const first = setTimeout(place, 480)
    const observer = new ResizeObserver(place)
    observer.observe(el)
    window.addEventListener('resize', place)
    return () => {
      clearTimeout(first)
      observer.disconnect()
      window.removeEventListener('resize', place)
    }
  })

  onDestroy(() => {
    if (account.phase === 'page') hideView()
  })

  $effect(() => {
    if (account.phase === 'relay') relayField?.focus()
  })

  /** Navigateur : la page du relais imitée, « Rattacher » répond comme le relais. */
  function simulateAttach() {
    handleAccount({
      type: 'accountAttached',
      relay: account.relay,
      owner: 'Matthieu',
      email: 'matthieu@exemple.fr',
      organization: false,
      accept: true,
      remoteAccess: true,
    } as { type: string })
  }

  const probeText = $derived.by(() => {
    if (account.probing) return t('accountStep.relay.checking')
    if (!probe) return ''
    if (probe.error === 'bad_address') return t('accountStep.relay.badAddress')
    if (!probe.ok) return t('accountStep.relay.unreachable', { reason: probe.error ?? '' })
    const accounts = probe.accounts ? t('accountStep.relay.accountsOn') : t('accountStep.relay.accountsOff')
    return t('accountStep.relay.joined', { version: probe.version ?? '?', accounts })
  })
</script>

{#snippet progress()}
  <div class="progress">
    <span class="lbl signal">{progressLabel}</span>
    <div class="track" aria-hidden="true"><span style:width="{Math.max(2, Math.min(100, percent))}%"></span></div>
    {#if !finished}<span class="mono last">{lastLine}</span>{/if}
  </div>
{/snippet}

{#if account.phase === 'choose'}
  <div class="step">
    {@render progress()}
    <div class="head">
      <h1 class="title">{t('accountStep.title')}</h1>
      <p class="lead">{t('accountStep.lead')}</p>
    </div>
    {#if account.relayAccounts}
      <div class="cards">
        <button type="button" class="card first" onclick={() => startAccount('signin')}>
          <strong>{t('accountStep.signin')}</strong>
          <span>{t('accountStep.signinDetail')}</span>
        </button>
        <button type="button" class="card" onclick={() => startAccount('signup')}>
          <strong>{t('accountStep.signup')}</strong>
          <span>{t('accountStep.signupDetail')}</span>
        </button>
      </div>
    {:else}
      <div class="notice">{t('accountStep.relay.noAccountsUse')}</div>
    {/if}
    <div class="relay-line">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
        ><circle cx="12" cy="12" r="9"></circle><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"></path></svg
      >
      <span>{t('accountStep.relayLabel')}</span>
      <span class="relay-name">{account.relay}</span>
      <span aria-hidden="true">·</span>
      <button type="button" class="text-button" onclick={openRelay}>{t('accountStep.otherRelay')}</button>
      {#if account.relay !== DEFAULT_RELAY}
        <span aria-hidden="true">·</span>
        <button type="button" class="text-button" onclick={defaultRelay}>{t('accountStep.relay.back')}</button>
      {/if}
    </div>
  </div>
  <div class="footer">
    <span class="note">{t('accountStep.laterNote')}</span>
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={later}>{t('accountStep.later')}</button>
  </div>
{:else if account.phase === 'relay'}
  <div class="step">
    {@render progress()}
    <div class="head">
      <h1 class="title">{t('accountStep.relay.title')}</h1>
      <p class="lead">{t('accountStep.relay.lead')}</p>
    </div>
    <div class="field narrow">
      <label for="relay-address">{t('accountStep.relay.address')}</label>
      <input
        id="relay-address"
        class="mono"
        bind:this={relayField}
        value={account.relayInput}
        oninput={(e) => typeRelay(e.currentTarget.value)}
        onkeydown={(e) => e.key === 'Enter' && useRelay()}
        placeholder="relais.exemple.org:9443"
        autocomplete="off"
        spellcheck="false"
        aria-invalid={!!probe && !probe.ok}
        aria-describedby="relay-hint relay-status"
      />
      <span id="relay-hint" class="hint">{t('accountStep.relay.hint')}</span>
    </div>
    <div id="relay-status" class="status narrow" role="status" class:ok={probe?.ok} class:bad={probe && !probe.ok} class:empty={!probeText}>
      {#if probe?.ok}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--ok)" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>
      {/if}
      <span>{probeText}</span>
    </div>
    {#if probe?.ok && !probe.accounts}
      <p class="note narrow">{t('accountStep.relay.noAccounts')}</p>
    {/if}
  </div>
  <div class="footer">
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={defaultRelay}>{t('accountStep.relay.back')}</button>
    <button type="button" class="primary" disabled={!probe?.ok} onclick={useRelay}>{t('accountStep.relay.use')}</button>
  </div>
{:else if account.phase === 'waiting'}
  <div class="step">
    {@render progress()}
    <div class="head">
      <h1 class="title">{t('accountStep.waiting.title')}</h1>
      <p class="lead">{t('accountStep.waiting.lead', { relay: account.relay })}</p>
      <span class="bar" aria-hidden="true"><span></span></span>
    </div>
  </div>
  <div class="footer">
    <button type="button" class="text-button" onclick={backToChoice}>{t('accountStep.back')}</button>
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={later}>{t('accountStep.later')}</button>
  </div>
{:else if account.phase === 'page' && account.page}
  <div class="page">
    <div class="page-bar">
      <button type="button" class="text-button" onclick={backToChoice}>{t('accountStep.back')}</button>
      <span class="origin">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"
          ><rect x="5" y="11" width="14" height="10" rx="2"></rect><path d="M8 11V7a4 4 0 0 1 8 0v4"></path></svg
        >
        {account.page.relay}
      </span>
      <span>{progressLabel}</span>
    </div>
    <div class="frame" bind:this={frame}>
      {#if account.page.external}
        <div class="external">
          <p class="lead">{t('accountStep.page.external')}</p>
          <button type="button" class="ghost" onclick={() => openUrl(account.page!.url)}>{t('accountStep.page.reopen')}</button>
        </div>
      {:else if !native}
        <div class="external">
          <span class="lbl">{t('accountStep.page.simulation')}</span>
          <p class="lead">{t('accountStep.page.simulationLead', { relay: account.page.relay })}</p>
          <button type="button" class="primary" onclick={simulateAttach}>{t('accountStep.page.simulationAttach')}</button>
        </div>
      {/if}
    </div>
  </div>
{:else if account.phase === 'failed'}
  <div class="step">
    {@render progress()}
    <div class="head">
      <h1 class="title">{t('accountStep.failed.title')}</h1>
      <p class="lead">{failureText()}</p>
    </div>
  </div>
  <div class="footer">
    <button type="button" class="text-button" onclick={backToChoice}>{t('accountStep.otherChoice')}</button>
    <div class="spacer"></div>
    <button type="button" class="ghost" onclick={later}>{t('accountStep.later')}</button>
    {#if account.failure?.code !== 'no_accounts' && account.failure?.code !== 'already_attached'}
      <button type="button" class="primary" onclick={() => startAccount(account.intent)}>{t('accountStep.retry')}</button>
    {/if}
  </div>
{/if}

<style>
  .step {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding-top: 18px;
    max-width: 680px;
  }

  .progress {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .track {
    height: 4px;
    border-radius: 2px;
    background: var(--line-2);
    overflow: hidden;
  }

  .track span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--signal);
    transition: width 0.4s var(--ease-out);
  }

  .last {
    font-size: 11px;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 4px;
  }

  .head h1 {
    font-size: 30px;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  .card {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 22px;
    border-radius: 16px;
    background: var(--ink-2);
    border: 1px solid var(--line-3);
    color: var(--text);
    cursor: pointer;
  }

  .card.first,
  .card:hover,
  .card:focus-visible {
    border-color: var(--signal);
  }

  .card strong {
    font-size: 18px;
    font-weight: 650;
  }

  .card span {
    font-size: 14px;
    line-height: 1.5;
    color: var(--muted);
  }

  .relay-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 14px;
    color: var(--muted);
  }

  .relay-name {
    color: var(--text);
  }

  .narrow {
    max-width: 560px;
  }

  .field input.mono {
    font-family: var(--mono);
    font-size: 15px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 22px;
    padding: 14px 16px;
    border-radius: 12px;
    background: var(--ink-2);
    border: 1px solid var(--line);
    font-size: 15px;
    color: var(--muted);
  }

  .status.empty {
    visibility: hidden;
  }

  .status.ok {
    color: var(--text);
  }

  .status.bad {
    color: var(--bad-text);
    border-color: var(--bad-line);
  }

  .bar {
    margin-top: 6px;
    height: 3px;
    max-width: 420px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.1);
    overflow: hidden;
  }

  .bar span {
    display: block;
    width: 35%;
    height: 100%;
    background: var(--signal);
    animation: slide 1.4s var(--ease-out) infinite;
  }

  @keyframes slide {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }

  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 8px 0 20px;
  }

  .page-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13px;
    color: var(--faint);
  }

  .origin {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .frame {
    flex: 1;
    min-height: 0;
    border-radius: 16px;
    border: 1px solid var(--line-2);
    background: var(--ink-0);
    overflow: hidden;
    display: flex;
  }

  .external {
    margin: auto;
    max-width: 440px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
  }
</style>
