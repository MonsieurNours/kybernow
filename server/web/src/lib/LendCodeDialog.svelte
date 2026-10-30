<!--
  « Prêter maintenant » (page « Prêt » d'Admin) : en deux étapes, la durée et le
  compte Steam, puis le code pour une séance en grand, son QR code et son lien
  (comme le partage : lien du relais attendu 10 s au plus). Le code reste ensuite
  dans la carte d'état.
-->
<script lang="ts">
  import { adminLend, type LendDuration, type LendSteam, type LendView } from './api'
  import { errorText, notify, setLend } from './store.svelte'
  import { copyText, durationName, LEND_DURATIONS, steamName } from './lend'
  import Icon from './Icon.svelte'
  import Qr from './Qr.svelte'
  import Segmented from './Segmented.svelte'
  import { t } from './i18n/index.svelte'

  let { view, onchange, onclose }: { view: LendView | null; onchange: (view: LendView) => void; onclose: () => void } = $props()

  let step = $state<1 | 2>(1)
  let duration = $state<LendDuration>(7200)
  let steam = $state<LendSteam>('friend')
  let busy = $state(false)
  let now = $state(Date.now())

  const code = $derived(view?.code ?? null)
  const spaced = $derived(code ? `${code.code.slice(0, 3)} ${code.code.slice(3)}` : '')

  $effect(() => {
    if (step !== 2) return
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  let waitFrom = 0
  const relay = $derived.by(() => {
    const st = code?.relay?.state ?? null
    if (st !== 'connecting') return st
    if (!waitFrom) waitFrom = Date.now()
    return now - waitFrom < 10000 ? 'waiting' : 'unreachable'
  })

  async function create() {
    busy = true
    try {
      const fresh = await adminLend.code(duration, steam)
      setLend(fresh)
      onchange(fresh)
      step = 2
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  async function copy() {
    if (code && (await copyText(code.web))) notify('ok', t('share.copied'))
    else notify('error', t('share.copyFailed'))
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }
</script>

<svelte:window {onkeydown} />

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="lend-dialog-title">
    <div class="steps" aria-hidden="true"><i class="on"></i><i class:on={step === 2}></i></div>
    {#if step === 1}
      <h3 id="lend-dialog-title" class="disp">{t('lend.admin.lendNow')}</h3>
      <div class="field">
        <span class="label">{t('lend.duration.label')}</span>
        <Segmented label={t('lend.duration.label')} options={LEND_DURATIONS.map((d) => ({ value: String(d), label: durationName(d) }))} value={String(duration)} onchange={(v: string) => (duration = v === 'null' ? null : (Number(v) as LendDuration))} />
      </div>
      <div class="field">
        <span class="label">{t('lend.steam.label')}</span>
        <div class="choice" role="group" aria-label={t('lend.steam.label')}>
          {#each ['friend', 'host'] as const as s (s)}
            <button type="button" aria-pressed={steam === s} onclick={() => (steam = s)}>
              <strong>{steamName(s)}</strong>
              <span>{s === 'friend' ? t('lend.admin.dialog.steamFriendHelp') : t('lend.admin.dialog.steamHostHelp')}</span>
            </button>
          {/each}
        </div>
      </div>
      <div class="foot">
        <button type="button" class="ghost" onclick={onclose}>{t('lend.friends.cancel')}</button>
        <button type="button" class="primary" disabled={busy} onclick={create}><Icon name="key" size={16} stroke={2} />{t('lend.create')}</button>
      </div>
    {:else}
      <h3 id="lend-dialog-title" class="disp">{t('lend.admin.dialog.codeTitle')}</h3>
      {#if code}
        <div class="bigcode">
          <span class="qr">
            {#if relay === 'waiting'}<span class="spin" aria-label={t('share.relayPreparing')}></span>{:else}<Qr text={code.link} size={128} label={t('lend.qrLabel')} />{/if}
          </span>
          <span class="code">{spaced}</span>
          <span class="lbl">{durationName(code.duration)} · {steamName(code.steam)} · {t('lend.admin.dialog.valid')}</span>
        </div>
        <p class="help">{t('lend.help')}</p>
        {#if relay === 'waiting'}<p class="help" role="status">{t('share.relayPreparing')}</p>{:else if relay === 'unreachable'}<p class="help">{t('share.relayUnreachable')}</p>{/if}
      {/if}
      <div class="foot">
        <button type="button" class="ghost" disabled={!code || relay === 'waiting'} onclick={copy}><Icon name="copy" size={15} stroke={2} />{t('share.copy')}</button>
        <button type="button" class="primary" onclick={onclose}>{t('lend.admin.dialog.done')}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 16px;
    background: rgb(5 7 10 / 0.7);
    backdrop-filter: blur(8px);
    animation: fade 0.25s ease-out;
  }

  .dialog {
    width: min(540px, 100%);
    max-height: 100%;
    overflow: auto;
    border-radius: 18px;
    background: var(--ink-1);
    box-shadow: inset 0 0 0 1px var(--line-2), 0 30px 80px rgb(0 0 0 / 0.6);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    animation: rise 0.35s var(--ease-out) both;
  }

  .steps {
    display: flex;
    gap: 8px;
  }

  .steps i {
    height: 4px;
    flex: 1;
    border-radius: 4px;
    background: var(--ink-3);
  }

  .steps i.on {
    background: var(--signal);
    box-shadow: 0 0 8px var(--signal);
  }

  h3 {
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }

  .label {
    font-size: 12.5px;
    color: var(--muted);
  }

  .choice {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .choice button {
    text-align: left;
    padding: 12px 14px;
    border: 0;
    border-radius: 12px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .choice button strong {
    font-size: 14px;
  }

  .choice button span {
    font-size: 12px;
    color: var(--faint);
  }

  .choice button[aria-pressed='true'] {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal);
  }

  .bigcode {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
  }

  .code {
    font-family: var(--mono);
    font-size: 52px;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--neon-core);
    text-shadow: var(--text-glow);
    white-space: nowrap;
  }

  .qr {
    width: 140px;
    height: 140px;
    border-radius: 10px;
    background: #f2f8fa;
    padding: 6px;
    display: grid;
    place-items: center;
  }

  .spin {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 3px solid rgb(5 7 10 / 0.15);
    border-top-color: var(--signal-strong);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .help {
    text-align: center;
    color: var(--text-2);
    font-size: 13.5px;
  }

  .foot {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @media (max-width: 560px) {
    .choice {
      grid-template-columns: 1fr;
    }

    .code {
      font-size: 40px;
    }
  }
</style>
