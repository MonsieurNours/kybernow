<!--
  Panneau du prêt de l'ordinateur, côté hôte (docs/pret.md, docs/link-ui.md,
  « Prêt de l'ordinateur ») : les demandes des amis (« Accepter », « Refuser »,
  le compte Steam), le prêt en cours (« Prêté à Léa jusqu'à 18 h »,
  « Reprendre l'ordinateur », le préavis), et le code pour une séance
  (« Durée », « Compte Steam », « Créer un code », le code, le lien et son QR
  code, « Annuler le code »). Le même dans Admin (page « Prêt » et accueil) et
  dans la barre de session du client web, sur les mêmes routes.
-->
<script lang="ts">
  import { adminLend, type LendDuration, type LendOps, type LendSteam, type LendView } from './api'
  import { errorText, notify as storeNotify, refreshStatus, setLend } from './store.svelte'
  import { sinceText } from './format'
  import { askingRequests, countdown, durationName, LEND_DURATIONS, lendAppName, lentLine, slotStarting, steamName, takingBack } from './lend'
  import Icon from './Icon.svelte'
  import Qr from './Qr.svelte'
  import Segmented from './Segmented.svelte'
  import { t } from './i18n/index.svelte'

  let {
    view,
    computer,
    ops = adminLend,
    onchange = setLend,
    notify = storeNotify,
    refresh = refreshStatus,
    own = false,
    admin = true,
    oncancel,
  }: {
    view: LendView
    /** Nom de l'ordinateur : titre du code (« Prêter PC-SALON »). */
    computer: string
    ops?: LendOps
    /** La vue après une action. */
    onchange?: (view: LendView) => void
    notify?: (kind: 'ok' | 'error', text: string) => void
    /** Relit le prêt (demande partie, prêt fini entre-temps). */
    refresh?: () => unknown
    /** Panneau de l'hôte dans sa propre session : un code ou un accord y ferme la session. */
    own?: boolean
    /** Dans Admin : « Prêt coupé » mène à la page « Prêt ». */
    admin?: boolean
    /** « Annuler » à côté de « Créer un code » (accueil d'Admin : le panneau se referme). */
    oncancel?: () => void
  } = $props()

  let busy = $state<string | null>(null)
  let duration = $state<LendDuration>(7200)
  let steam = $state<LendSteam>('friend')
  /** Compte Steam choisi pour chaque demande (« Son compte Steam » proposé). */
  let answers = $state<Record<string, LendSteam>>({})
  let now = $state(Date.now() / 1000)

  // Le préavis compte à la seconde ; sinon, « depuis 12 min » suffit à la minute.
  $effect(() => {
    const fast = takingBack(view) !== null || view.code?.relay?.state === 'connecting'
    const timer = setInterval(() => (now = Date.now() / 1000), fast ? 1000 : 15000)
    return () => clearInterval(timer)
  })

  const lent = $derived(!!view.borrower && view.state !== 'free')
  const noticeAt = $derived(takingBack(view))
  const slotName = $derived(slotStarting(view))
  const requests = $derived(askingRequests(view))
  const code = $derived(view.code ? `${view.code.code.slice(0, 3)} ${view.code.code.slice(3)}` : '')

  /** Le relais du code : comme le panneau du partage, lien du relais attendu 10 s au plus. */
  const RELAY_WAIT_MS = 10000
  let waitFor: { code: string; since: number } | null = null
  const relay = $derived.by(() => {
    const st = view.code?.relay?.state ?? null
    if (st !== 'connecting' || !view.code) {
      waitFor = null
      return st
    }
    if (waitFor?.code !== view.code.code) waitFor = { code: view.code.code, since: Date.now() }
    return now * 1000 - waitFor.since < RELAY_WAIT_MS ? 'waiting' : 'unreachable'
  })

  /** Ce qu'on dit de celui qui a l'ordinateur : son app, d'où il vient, depuis quand, le compte Steam. */
  const borrowerMeta = $derived.by(() => {
    const b = view.borrower
    if (!b) return ''
    if (view.state === 'preparing') return t('lend.steps.preparing')
    return [
      lendAppName(b.app),
      b.via === 'relay' ? t('share.away') : '',
      b.connected ? t('share.since', { duration: sinceText(now - b.since) }) : t('lend.host.disconnected'),
      steamName(b.steam),
    ]
      .filter(Boolean)
      .join(' · ')
  })

  async function run(key: string, action: () => Promise<LendView>, done?: string) {
    busy = key
    try {
      onchange(await action())
      if (done) notify('ok', done)
    } catch (e) {
      notify('error', errorText(e))
      refresh()
    } finally {
      busy = null
    }
  }

  async function copy() {
    if (!view.code) return
    try {
      await navigator.clipboard.writeText(view.code.web)
      notify('ok', t('share.copied'))
    } catch {
      notify('error', t('share.copyFailed'))
    }
  }
</script>

<div class="lend">
  {#if requests.length}
    <div class="people requests">
      {#each requests as r (r.id)}
        <div class="person" role="group" aria-labelledby="lend-ask-{r.id}">
          <span class="device-icon live"><Icon name="lend" size={19} stroke={1.6} /></span>
          <div class="who">
            <span class="name" id="lend-ask-{r.id}">{t('lend.requests.asks', { name: r.name })}</span>
            <span class="meta">{r.via === 'relay' ? t('lend.requests.away', { app: lendAppName(r.app) }) : lendAppName(r.app)}</span>
            {#if own}<span class="meta warn">{t('lend.requests.sessionCloses')}</span>{/if}
          </div>
          <div class="moves">
            <Segmented
              label={t('lend.steam.label')}
              options={(['friend', 'host'] as LendSteam[]).map((s) => ({ value: s, label: steamName(s) }))}
              value={answers[r.id] ?? 'friend'}
              disabled={busy !== null}
              onchange={(s: LendSteam) => (answers[r.id] = s)}
            />
            <button type="button" class="ghost small accept" disabled={busy !== null} onclick={() => run(r.id, () => ops.answer(r.id, true, answers[r.id] ?? 'friend'))}><Icon name="check" size={15} stroke={2} />{t('lend.requests.accept')}</button>
            <button type="button" class="ghost small" disabled={busy !== null} onclick={() => run(r.id, () => ops.answer(r.id, false, answers[r.id] ?? 'friend'))}>{t('lend.requests.refuse')}</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if slotName !== null}
    <p class="notice slot" role="status"><Icon name="info" size={16} /><span>{t('lend.requests.slot', { name: slotName })}</span></p>
  {/if}

  {#if lent && view.borrower}
    <div class="current">
      <span class="device-icon live"><Icon name="lend" size={22} stroke={1.6} /></span>
      <div class="who">
        <h2 class="disp">{lentLine(view)}</h2>
        <span class="meta">{borrowerMeta}</span>
      </div>
      <div class="moves">
        {#if noticeAt !== null}
          <span class="mono countdown" role="timer">{t('lend.host.takingBack', { time: countdown(noticeAt - now) })}</span>
          <button type="button" class="danger" disabled={busy !== null || view.state === 'ending'} onclick={() => run('back', () => ops.takeBack(true))}>{t('lend.host.takeBackNow')}</button>
        {:else}
          <button type="button" class="danger" disabled={busy !== null || view.state === 'ending'} onclick={() => run('back', () => ops.takeBack(false))}>{t('lend.host.takeBack')}</button>
        {/if}
      </div>
    </div>
  {:else if !view.enabled}
    <p class="off">
      {#if admin}<a href="#/pret">{t('lend.off')}</a>{:else}{t('lend.off')}{/if}
    </p>
  {:else if view.code}
    <div class="top">
      {#if relay === 'waiting'}
        <div class="qr-wait" style="width: 152px; height: 152px" role="img" aria-label={t('share.relayPreparing')}><span class="spin big"></span></div>
      {:else}
        <Qr text={view.code.link} size={152} label={t('lend.qrLabel')} />
      {/if}
      <div class="intro">
        <div class="heading">
          <h2 class="disp">{t('lend.title', { name: computer })}</h2>
          <span class="sub">{[durationName(view.code.duration), steamName(view.code.steam)].join(' · ')}</span>
        </div>
        <div class="code">
          <span class="lbl">{t('share.codeLabel')}</span>
          <strong class="mono" aria-label={t('share.codeAria', { digits: view.code.code.split('').join(' ') })}>{code}</strong>
          <span class="once">{t('lend.once')}</span>
        </div>
        <div class="link-row">
          <button type="button" class="ghost small" onclick={copy} disabled={relay === 'waiting'}><Icon name="copy" size={15} stroke={2} />{t('share.copy')}</button>
          {#if relay !== 'waiting'}<span class="mono link">{view.code.web}</span>{/if}
        </div>
        <p class="help">{t('lend.help')}</p>
        {#if relay === 'waiting'}
          <p class="help" role="status"><span class="spin" aria-hidden="true"></span>{t('share.relayPreparing')}</p>
        {:else if relay === 'unreachable'}<p class="help">{t('share.relayUnreachable')}</p>{/if}
        {#if own}<p class="help warn">{t('lend.closesSession')}</p>{/if}
      </div>
    </div>
    <div class="foot">
      <button type="button" class="danger" disabled={busy !== null} onclick={() => run('cancel', () => ops.cancelCode())}>{t('lend.cancelCode')}</button>
    </div>
  {:else}
    <div class="form">
      <h2 class="disp">{t('lend.title', { name: computer })}</h2>
      <div class="field">
        <span class="lbl" aria-hidden="true">{t('lend.duration.label')}</span>
        <Segmented label={t('lend.duration.label')} options={LEND_DURATIONS.map((d) => ({ value: String(d), label: durationName(d) }))} value={String(duration)} disabled={busy !== null} onchange={(v: string) => (duration = v === 'null' ? null : (Number(v) as LendDuration))} />
      </div>
      <div class="field">
        <span class="lbl" aria-hidden="true">{t('lend.steam.label')}</span>
        <Segmented label={t('lend.steam.label')} options={(['friend', 'host'] as LendSteam[]).map((s) => ({ value: s, label: steamName(s) }))} value={steam} disabled={busy !== null} onchange={(s: LendSteam) => (steam = s)} />
      </div>
      {#if own}<p class="help warn">{t('lend.closesSession')}</p>{/if}
      <div class="foot">
        {#if oncancel}<button type="button" class="ghost" disabled={busy !== null} onclick={oncancel}>{t('share.cancel')}</button>{/if}
        <button type="button" class="primary" disabled={busy !== null} onclick={() => run('code', () => ops.code(duration, steam))}><Icon name="lend" size={16} stroke={2} />{t('lend.create')}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .lend {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .top {
    display: flex;
    flex-wrap: wrap;
    gap: 22px 26px;
    align-items: flex-start;
  }

  .intro {
    flex: 1 1 320px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .heading {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  h2 {
    font-size: 23px;
    font-weight: 620;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
  }

  .sub {
    font-size: 13px;
    color: var(--faint);
  }

  .code {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .code .lbl {
    color: var(--signal-light);
    font-size: 9.5px;
  }

  .code strong {
    font-size: 34px;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--neon-core);
    text-shadow: var(--text-glow);
    white-space: nowrap;
    user-select: all;
  }

  .once {
    font-size: 12.5px;
    color: var(--faint);
  }

  .link-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 14px;
  }

  .link {
    font-size: 11px;
    color: var(--faint);
    overflow-wrap: anywhere;
    user-select: all;
  }

  .qr-wait {
    display: grid;
    place-items: center;
    flex: none;
  }

  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    margin-right: 8px;
    vertical-align: -2px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, currentColor 20%, transparent);
    border-top-color: var(--signal, currentColor);
    animation: spin 0.8s linear infinite;
  }

  .spin.big {
    width: 32px;
    height: 32px;
    margin: 0;
    border-width: 3px;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation-duration: 2.4s;
    }
  }

  .help {
    font-size: 13px;
    line-height: 1.5;
    color: var(--faint);
    max-width: 60ch;
  }

  .help.warn,
  .meta.warn {
    color: var(--warn-text);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  .person,
  .current {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto;
    gap: 10px 14px;
    align-items: center;
  }

  .person {
    padding: 12px 0;
    border-top: 1px solid var(--signal-line);
  }

  .person:first-child {
    border-top: 0;
    padding-top: 0;
  }

  .current {
    grid-template-columns: 44px minmax(0, 1fr) auto;
  }

  .device-icon {
    width: 36px;
    height: 36px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    background: var(--ink-3);
    color: var(--text-2);
  }

  .current .device-icon {
    width: 44px;
    height: 44px;
    border-radius: 11px;
  }

  .device-icon.live {
    background: rgb(109 91 255 / 0.14);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.5);
    color: var(--signal-light);
  }

  .who {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .name {
    font-size: 14.5px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .meta {
    font-size: 12.5px;
    color: var(--faint);
  }

  .moves {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    align-items: center;
    gap: 8px 10px;
  }

  /* Une demande : le prénom, puis le compte Steam et la réponse dessous (la ligne serait trop serrée). */
  .requests .person {
    grid-template-columns: 36px minmax(0, 1fr);
  }

  .requests .moves {
    grid-column: 2;
    justify-content: flex-start;
  }

  .requests:not(:last-child) {
    padding-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }

  .moves .accept {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
  }

  .moves .accept:hover:not(:disabled) {
    background: rgb(109 91 255 / 0.22);
    color: #fff;
  }

  .countdown {
    font-size: 14px;
    color: var(--warn-text);
  }

  .notice.slot {
    background: var(--signal-soft);
    box-shadow: inset 0 0 0 1px var(--signal-line);
    color: var(--signal-light);
  }

  .off {
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--text-2);
  }

  .off a {
    color: var(--text-2);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 10px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }

  @media (max-width: 760px) {
    .person,
    .current {
      grid-template-columns: 36px minmax(0, 1fr);
    }

    .current .device-icon {
      width: 36px;
      height: 36px;
    }

    .moves {
      grid-column: 1 / -1;
      justify-content: flex-start;
    }

    .foot > button {
      flex: 1 1 auto;
    }
  }
</style>
