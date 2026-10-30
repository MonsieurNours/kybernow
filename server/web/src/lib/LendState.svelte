<!--
  La carte d'état de la page « Prêt » d'Admin (docs/pret.md, textes de docs/link-ui.md) :
  ce qui se passe et l'action principale, teintée selon l'état. Libre (« Prêter
  maintenant », le prochain créneau), code en attente (le code en néon et son QR
  code), demande (« Accepter », « Refuser », le compte Steam ; les autres demandes
  en une ligne), créneau qui commence, préparation, prêté (« Reprendre
  l'ordinateur », le temps), reprise en cours (le décompte, « Reprendre tout de
  suite »), fin du prêt.
-->
<script lang="ts">
  import { adminLend, type LendFriend, type LendSteam, type LendView } from './api'
  import { errorText, notify, refreshStatus, setLend } from './store.svelte'
  import { ago, sinceText } from './format'
  import {
    askingRequests,
    copyText,
    countdown,
    dayName,
    durationName,
    hourText,
    lendAppName,
    lentLine,
    nextSlot,
    slotStarting,
    steamName,
    takingBack,
    weekday,
  } from './lend'
  import Icon from './Icon.svelte'
  import Qr from './Qr.svelte'
  import Segmented from './Segmented.svelte'
  import { t } from './i18n/index.svelte'

  let { view, friends, onlend }: { view: LendView; friends: LendFriend[]; onlend: () => void } = $props()

  let busy = $state<string | null>(null)
  let now = $state(Date.now() / 1000)
  /** Le compte Steam choisi pour chaque demande (celui proposé par l'ami, sinon « Son compte Steam »). */
  let answers = $state<Record<string, LendSteam>>({})

  const noticeAt = $derived(takingBack(view))
  $effect(() => {
    const fast = noticeAt !== null || view.code?.relay?.state === 'connecting'
    const timer = setInterval(() => (now = Date.now() / 1000), fast ? 1000 : 15000)
    return () => clearInterval(timer)
  })

  const requests = $derived(askingRequests(view))
  const slotName = $derived(slotStarting(view))
  const b = $derived(view.borrower)
  const code = $derived(view.code ? `${view.code.code.slice(0, 3)} ${view.code.code.slice(3)}` : '')

  type Tone = 'ok' | 'code' | 'warn' | 'lent'
  const mode = $derived.by(() => {
    if (b && view.state === 'ending') return 'ending'
    if (b && noticeAt !== null) return 'notice'
    if (b && view.state === 'preparing') return 'preparing'
    if (b) return 'lent'
    if (requests.length) return 'request'
    if (slotName !== null) return 'slot'
    if (view.code) return 'code'
    return 'free'
  })
  const tone = $derived<Tone>(({ free: 'ok', code: 'code', request: 'warn', slot: 'warn', notice: 'warn', ending: 'lent', preparing: 'lent', lent: 'lent' } as const)[mode])

  /** Le relais du code, comme le panneau du partage : lien du relais attendu 10 s au plus. */
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

  /** Le prochain créneau (ou celui en cours), à l'heure de ce navigateur. */
  const slot = $derived(nextSlot(friends, new Date(now * 1000)))
  const slotLine = $derived.by(() => {
    if (!slot) return ''
    if (slot.now) return t('lend.admin.state.free.slotNow', { name: slot.name, time: hourText(slot.end) })
    const start = new Date(slot.start * 1000)
    return t('lend.admin.state.free.nextSlot', { name: slot.name, day: dayName(weekday(start)), time: hourText(slot.start) })
  })

  const first = $derived(requests[0] ?? null)
  const others = $derived(requests.slice(1))
  const firstSteam = $derived(first ? (answers[first.id] ?? first.steam ?? 'friend') : 'friend')

  /** La part du prêt déjà passée (barre du temps), ou celle du préavis. */
  const progress = $derived.by(() => {
    if (!b) return null
    if (noticeAt !== null) return Math.max(0, Math.min(1, (noticeAt - now) / 120))
    if (!b.until) return null
    return Math.max(0, Math.min(1, (b.until - now) / Math.max(1, b.until - b.since)))
  })

  async function run(key: string, action: () => Promise<LendView>, done?: string) {
    busy = key
    try {
      setLend(await action())
      if (done) notify('ok', done)
    } catch (e) {
      notify('error', errorText(e))
      refreshStatus()
    } finally {
      busy = null
    }
  }

  async function copy() {
    if (view.code && (await copyText(view.code.web))) notify('ok', t('share.copied'))
    else notify('error', t('share.copyFailed'))
  }
</script>

<section class="state" data-tone={tone} aria-live="polite">
  <div class="body">
    {#if mode === 'free'}
      <span class="status">{t('lend.admin.state.free.status')}</span>
      <h2 class="disp">{t('lend.admin.state.free.title')}</h2>
      <p class="sub">{t('lend.admin.state.free.sub')}</p>
      {#if slotLine}<div class="meta"><span><Icon name="clock" size={14} />{slotLine}</span></div>{/if}
    {:else if mode === 'code' && view.code}
      <span class="status">{t('lend.admin.state.code.status')}</span>
      <h2 class="disp">{t('lend.admin.state.code.title')}</h2>
      <div class="code-row">
        <span class="code lit-number">{code}</span>
        <span class="qr">
          {#if relay === 'waiting'}<span class="spin" aria-label={t('share.relayPreparing')}></span>{:else}<Qr text={view.code.link} size={84} label={t('lend.qrLabel')} />{/if}
        </span>
      </div>
      <div class="meta">
        <span>{durationName(view.code.duration)}</span>
        <span>{steamName(view.code.steam)}</span>
        <span>{t('lend.admin.state.code.valid', { time: sinceText(view.code.expires - now) })}</span>
      </div>
      {#if relay === 'waiting'}<p class="hint" role="status"><span class="spin small"></span>{t('share.relayPreparing')}</p>{:else if relay === 'unreachable'}<p class="hint">{t('share.relayUnreachable')}</p>{/if}
    {:else if mode === 'request' && first}
      <span class="status">{t('lend.admin.state.request.status')}</span>
      <h2 class="disp">{t('lend.requests.asks', { name: first.name })}</h2>
      <p class="sub">{t('lend.admin.state.request.inUse')}</p>
      <div class="meta">
        <span>{first.via === 'relay' ? t('lend.requests.away', { app: lendAppName(first.app) }) : lendAppName(first.app)}</span>
        <span>{ago(first.since)}</span>
      </div>
      <div class="choose">
        <Segmented
          label={t('lend.steam.label')}
          options={(['friend', 'host'] as LendSteam[]).map((s) => ({ value: s, label: steamName(s) }))}
          value={firstSteam}
          onchange={(s: LendSteam) => (answers[first.id] = s)}
        />
      </div>
      {#if others.length}
        <p class="others">
          {others.length === 1
            ? t('lend.admin.state.request.others1', { names: others.map((r) => r.name).join(', ') })
            : t('lend.admin.state.request.othersN', { n: others.length, names: others.map((r) => r.name).join(', ') })}
        </p>
      {/if}
    {:else if mode === 'slot'}
      <span class="status">{t('lend.admin.state.slot.status')}</span>
      <h2 class="disp">{t('lend.requests.slot', { name: slotName ?? '' })}</h2>
    {:else if mode === 'preparing' && b}
      <span class="status">{t('lend.admin.state.preparing.status')}</span>
      <h2 class="disp">{t('lend.admin.state.preparing.title', { name: b.name })}</h2>
      <div class="meta"><span>{t(`lend.admin.kind.${b.kind}`)}</span><span>{lendAppName(b.app)}</span><span>{steamName(b.steam)}</span></div>
    {:else if mode === 'notice' && b && noticeAt !== null}
      <span class="status">{view.notice?.reason === 'taken_back' ? t('lend.admin.state.notice.status') : t('lend.admin.state.notice.endStatus')}</span>
      <h2 class="disp">
        {view.notice?.reason === 'taken_back'
          ? t('lend.host.takingBack', { time: countdown(noticeAt - now) })
          : t('lend.admin.state.notice.endTitle', { time: countdown(noticeAt - now) })}
      </h2>
      <p class="sub">{t('lend.admin.state.notice.warned', { name: b.name })}</p>
    {:else if mode === 'ending'}
      <span class="status">{t('lend.admin.state.ending.status')}</span>
      <h2 class="disp">{t('lend.admin.state.ending.title')}</h2>
    {:else if b}
      <span class="status">{t('lend.admin.state.lent.status')}</span>
      <h2 class="disp">{lentLine(view)}</h2>
      <div class="meta">
        <span>{t('lend.admin.state.lent.since', { time: hourText(b.since) })}</span>
        <span>{t(`lend.admin.kind.${b.kind}`)}</span>
        <span>{b.via === 'relay' ? t('lend.requests.away', { app: lendAppName(b.app) }) : lendAppName(b.app)}</span>
        <span>{steamName(b.steam)}</span>
        {#if b.until}<span>{t('lend.bar.left', { time: sinceText(b.until - now) })}</span>{/if}
        <span class:off={!b.connected}>{b.connected ? t('lend.admin.state.lent.connected') : t('lend.host.disconnected')}</span>
      </div>
    {/if}
  </div>

  <div class="acts">
    {#if mode === 'free'}
      <button type="button" class="primary" onclick={onlend}><Icon name="key" size={16} stroke={2} />{t('lend.admin.lendNow')}</button>
    {:else if mode === 'code'}
      <button type="button" class="ghost" disabled={relay === 'waiting'} onclick={copy}><Icon name="copy" size={15} stroke={2} />{t('share.copy')}</button>
      <button type="button" class="danger" disabled={busy !== null} onclick={() => run('cancel', () => adminLend.cancelCode())}>{t('lend.cancelCode')}</button>
    {:else if mode === 'request' && first}
      <button type="button" class="ghost" disabled={busy !== null} onclick={() => run('refuse', () => adminLend.answer(first.id, false, firstSteam))}>{t('lend.requests.refuse')}</button>
      <button type="button" class="primary" disabled={busy !== null} onclick={() => run('accept', () => adminLend.answer(first.id, true, firstSteam))}>{t('lend.requests.accept')}</button>
    {:else if mode === 'notice'}
      <button type="button" class="danger" disabled={busy !== null} onclick={() => run('now', () => adminLend.takeBack(true))}>{t('lend.host.takeBackNow')}</button>
    {:else if mode === 'lent'}
      <button type="button" class="ghost" disabled={busy !== null} onclick={() => run('back', () => adminLend.takeBack(false))}>{t('lend.host.takeBack')}</button>
    {/if}
  </div>

  {#if progress !== null}
    <div class="timebar" aria-hidden="true"><i style="width: {progress * 100}%"></i></div>
  {/if}
</section>

<style>
  .state {
    --tint: rgb(109 91 255 / 0.16);
    --dot: var(--ok);
    position: relative;
    overflow: hidden;
    border-radius: 18px;
    padding: 26px 28px;
    background: radial-gradient(90% 140% at 100% 0%, var(--tint), transparent 60%), var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 18px 28px;
    align-items: center;
    transition: background 0.4s;
  }

  .state[data-tone='ok'] {
    --tint: rgb(76 214 138 / 0.12);
  }

  .state[data-tone='code'] {
    --tint: rgb(109 91 255 / 0.2);
    --dot: var(--signal);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .state[data-tone='warn'] {
    --tint: rgb(245 184 65 / 0.16);
    --dot: var(--warn);
    box-shadow: inset 0 0 0 1px var(--warn-line);
  }

  .state[data-tone='lent'] {
    --tint: rgb(109 91 255 / 0.24);
    --dot: var(--signal);
    box-shadow: inset 0 0 0 1px var(--signal-line);
  }

  .body {
    min-width: 0;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-2);
  }

  .status::before {
    content: '';
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--dot);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--dot) 22%, transparent);
  }

  h2 {
    margin: 10px 0 6px;
    font-size: 30px;
    font-weight: 700;
    line-height: 1.08;
    letter-spacing: -0.02em;
    text-wrap: balance;
    font-variant-numeric: tabular-nums;
  }

  .sub {
    color: var(--text-2);
    max-width: 56ch;
    font-size: 14px;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin-top: 12px;
    color: var(--faint);
    font-size: 12.5px;
  }

  .meta span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .meta .off {
    color: var(--warn-text);
  }

  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    justify-content: flex-end;
  }

  .code-row {
    display: flex;
    align-items: center;
    gap: 20px;
    flex-wrap: wrap;
    margin-top: 6px;
  }

  .code {
    font-family: var(--mono);
    font-size: 38px;
    font-weight: 500;
    letter-spacing: 0.08em;
    color: var(--neon-core);
    text-shadow: var(--text-glow);
    white-space: nowrap;
  }

  .qr {
    width: 96px;
    height: 96px;
    border-radius: 8px;
    background: #f2f8fa;
    padding: 6px;
    display: grid;
    place-items: center;
    flex: none;
  }

  .choose {
    margin-top: 14px;
  }

  .others {
    margin-top: 12px;
    font-size: 13px;
    color: var(--warn-text);
  }

  .hint {
    margin-top: 10px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--faint);
  }

  .spin {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 3px solid rgb(5 7 10 / 0.15);
    border-top-color: var(--signal-strong);
    animation: spin 0.8s linear infinite;
  }

  .spin.small {
    width: 12px;
    height: 12px;
    border-width: 2px;
    border-color: rgb(255 255 255 / 0.15);
    border-top-color: var(--signal);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .timebar {
    grid-column: 1 / -1;
    height: 6px;
    border-radius: 6px;
    background: rgb(255 255 255 / 0.08);
    overflow: hidden;
  }

  .timebar i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, var(--signal-strong), var(--signal-light));
    border-radius: 6px;
    transition: width 1s linear;
  }

  .state[data-tone='warn'] .timebar i {
    background: var(--warn);
  }

  @media (max-width: 760px) {
    .state {
      grid-template-columns: 1fr;
      padding: 20px;
    }

    .acts {
      justify-content: flex-start;
    }

    h2 {
      font-size: 25px;
    }
  }
</style>
