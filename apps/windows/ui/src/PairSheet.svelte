<!--
  Association d'une console PlayStation (docs/link-ui.md « Consoles PlayStation », « Association »),
  comme PairConsoleSheet de l'app Mac : titrée du nom et du modèle de la console, ou « Ajouter un
  compte » sans console. Les étapes :
  - « Avec quel compte ? » quand deux comptes gardés ou plus ne sont pas encore associés à cette
    console (un seul : étape sautée ; aucun : l'étape Compte) ;
  - Compte : « Se connecter à PlayStation Network » ouvre la page de Sony dans sa propre fenêtre
    (`ps_login`, l'app en lit l'identifiant et le pseudo) ; la feuille attend pendant ce temps.
    Fenêtre fermée sans connexion : retour au départ ; identifiant non lu : la raison, et la saisie
    à la main s'ouvre. « Saisir l'identifiant à la main » : l'identifiant (base64 de 8 octets) et un
    nom. Un compte lu ou saisi est gardé tout de suite, même si l'association échoue ensuite ;
    sans console, la feuille se ferme (l'onglet PlayStation dit « Ce compte est déjà là. » s'il
    l'était) ;
  - Code : « Compte : {pseudo} », le chemin sur la console, le code à 8 chiffres, « Associer ».
    Associée : la feuille se ferme et l'accueil choisit la console. Les refus se lisent tels que
    l'app les dit.
-->
<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { PsAccount, PsFound, PsKept, PsState } from './lib/api'
  import { invoke, psAccountName, psConsoleCount, psValidId } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    target,
    ps,
    onclose,
    onaccount,
    onpaired,
  }: {
    /** La console à associer ; nul : « Ajouter un compte », sans console. */
    target: PsFound | null
    ps: PsState
    onclose: () => void
    /** Sans console : le compte est gardé (`existed` : il l'était déjà). */
    onaccount: (existed: boolean) => void
    /** La console est associée : son identifiant (`ps-<HOSTID>`). */
    onpaired: (id: string) => void
  } = $props()

  type Step = 'choose' | 'account' | 'code'

  /** Comptes gardés pas encore associés à cette console. */
  const free = $derived.by(() => {
    if (!target) return [] as PsAccount[]
    const kept = ps.consoles.find((c) => c.host_id === target.host_id)
    return ps.accounts.filter((a) => !kept?.accounts.some((x) => x.id === a.id))
  })

  // Deux comptes ou plus pas encore associés : le choix ; un seul : le code ; aucun : l'étape Compte.
  const start = untrack(() => free)
  let step = $state<Step>(!untrack(() => target) || start.length === 0 ? 'account' : start.length === 1 ? 'code' : 'choose')
  /** Le compte avec lequel associer (étape du code). */
  let account = $state<{ id: string; name: string | null } | null>(start.length === 1 ? start[0] : null)
  /** Le compte lu était déjà gardé : « Ce compte est déjà là. » */
  let already = $state(false)
  /** La fenêtre de Sony est ouverte : la feuille attend. */
  let signing = $state(false)
  let byHand = $state(false)
  let manualId = $state('')
  let manualName = $state('')
  let code = $state('')
  let busy = $state(false)
  let error = $state<string | null>(null)
  let codeField = $state<HTMLInputElement | null>(null)

  const digits = $derived(code.replace(/\D/g, ''))
  const manualValid = $derived(psValidId(manualId))

  onMount(() => {
    if (step === 'code') codeField?.focus()
  })

  async function toCode() {
    step = 'code'
    await tick()
    codeField?.focus()
  }

  /** Un compte lu ou saisi : gardé (par l'app), puis le code ; sans console, la feuille se ferme. */
  function kept(k: PsKept) {
    if (!target) {
      onaccount(k.existed)
      onclose()
      return
    }
    account = k.account
    already = k.existed
    error = null
    toCode()
  }

  async function signIn() {
    error = null
    signing = true
    try {
      const k = await invoke<PsKept | null>('ps_login')
      if (k) kept(k)
    } catch (e) {
      error = String(e)
      byHand = true
    } finally {
      signing = false
    }
  }

  async function useManualId(e?: Event) {
    e?.preventDefault()
    if (!manualValid || busy) return
    busy = true
    error = null
    try {
      kept(await invoke<PsKept>('ps_account_manual', { psnId: manualId.trim(), name: manualName.trim() }))
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }

  async function pair(e?: Event) {
    e?.preventDefault()
    if (digits.length !== 8 || busy || !target || !account) return
    busy = true
    error = null
    try {
      const id = await invoke<string>('ps_pair', { hostId: target.host_id, address: target.address, target: target.target, account: account.id, pin: digits })
      onpaired(id)
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }

  /** « 1234 5678 » pendant la saisie. */
  const grouped = (d: string) => (d.length > 4 ? `${d.slice(0, 4)} ${d.slice(4, 8)}` : d)
  const consolesOf = (id: string) => ps.accounts.find((a) => a.id === id)?.consoles ?? 0
</script>

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && !busy && onclose()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="pair-title">
    <button type="button" class="close" aria-label={t('common.close')} disabled={busy} onclick={onclose}><Icon name="x" size={14} /></button>
    <div class="head">
      {#if target}
        <h2 id="pair-title">{target.name}</h2>
        <span class="model">{target.model}</span>
      {:else}
        <h2 id="pair-title">{t('ps.add_account')}</h2>
      {/if}
    </div>

    {#if step === 'choose'}
      <h3>{t('ps.which_account')}</h3>
      <ul class="rows">
        {#each free as a (a.id)}
          <li>
            <button
              class="pick"
              onclick={() => {
                account = a
                error = null
                toCode()
              }}
            >
              <span class="t">{psAccountName(a)}</span>
              <span class="s">{psConsoleCount(consolesOf(a.id))}</span>
              <Icon name="chevron" size={16} />
            </button>
          </li>
        {/each}
      </ul>
      <div>
        <button class="ghost" onclick={() => ((error = null), (step = 'account'))}><Icon name="plus" size={14} />{t('ps.add_account')}</button>
      </div>
    {:else if step === 'account'}
      <p class="hint">{t('ps.sign_in_help')}</p>
      {#if signing}
        <div class="wait" role="status"><span class="pulse"></span>{t('ps.signing_in')}</div>
      {:else}
        <div><button class="primary" disabled={busy} onclick={signIn}><Icon name="user" size={18} />{t('ps.sign_in')}</button></div>
      {/if}
      <div><button class="link" aria-expanded={byHand} onclick={() => (byHand = !byHand)}>{t('ps.by_hand')}</button></div>
      {#if byHand}
        <form class="manual" onsubmit={useManualId}>
          <label class="col">
            <span class="eyebrow">{t('ps.account_field')}</span>
            <input class="field mono" autocomplete="off" spellcheck="false" placeholder="AbCdEfGhIjk=" maxlength="16" bind:value={manualId} disabled={busy || signing} />
          </label>
          <label class="col">
            <span class="eyebrow">{t('ps.name_field')}</span>
            <input class="field" autocomplete="off" maxlength="40" bind:value={manualName} disabled={busy || signing} />
            <span class="help">{t('ps.name_help')}</span>
          </label>
          <div class="end"><button type="submit" class="primary" disabled={!manualValid || busy || signing}>{t('common.continue')}</button></div>
        </form>
      {/if}
    {:else}
      {#if account}
        <div class="account"><Icon name="user" size={16} />{t('ps.account_line', { name: psAccountName(account) })}</div>
      {/if}
      {#if already}<div class="info" role="status">{t('ps.already_here')}</div>{/if}
      <p class="hint">{t('ps.code_help')}</p>
      <form onsubmit={pair}>
        <input
          class="field code mono"
          inputmode="numeric"
          autocomplete="off"
          spellcheck="false"
          maxlength="9"
          placeholder="1234 5678"
          aria-label={t('ps.code_field')}
          title={t('ps.code_field')}
          value={grouped(digits)}
          bind:this={codeField}
          oninput={(e) => (code = (e.currentTarget as HTMLInputElement).value.replace(/\D/g, '').slice(0, 8))}
          disabled={busy}
        />
        <div class="end">
          <button type="submit" class="primary" disabled={digits.length !== 8 || busy}>
            {#if busy}<span class="spin" aria-hidden="true"></span>{/if}{t('ps.pair')}
          </button>
        </div>
      </form>
    {/if}

    {#if error}<div class="error" role="alert">{error}</div>{/if}
  </div>
</div>

<style>
  /* Au-dessus de l'ajout (« Ajouter un compte » depuis l'onglet PlayStation). */
  .veil {
    position: absolute;
    inset: 0;
    z-index: 32;
    background: rgb(5 7 10 / 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
    animation: fade 0.25s var(--ease-out);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .sheet {
    position: relative;
    width: min(620px, 100%);
    max-height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 36px 40px 40px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background:
      radial-gradient(520px 320px at 100% 0%, rgb(109 91 255 / 0.18), transparent 70%),
      var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
    animation: rise 0.32s var(--ease-out);
  }

  @keyframes rise {
    from {
      transform: translateY(18px) scale(0.985);
      opacity: 0;
    }
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-right: 40px;
  }

  h2 {
    margin: 0;
    font-size: 28px;
    font-weight: 640;
    font-stretch: 125%;
    overflow-wrap: anywhere;
  }

  h3 {
    margin: 0;
    font-size: 19px;
    font-weight: 600;
  }

  .model {
    font-size: 14px;
    color: var(--muted);
  }

  .close {
    position: absolute;
    right: 16px;
    top: 16px;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.06);
  }

  .hint {
    margin: 0;
    font-size: 14px;
    line-height: 1.55;
    color: var(--muted);
  }

  .help {
    font-size: 12px;
    color: var(--faint);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .pick {
    width: 100%;
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 16px;
    padding: 14px 18px;
    border: 1px solid var(--line-2);
    border-radius: 12px;
    background: var(--ink-2);
    text-align: left;
    color: var(--muted);
  }

  .pick:hover {
    border-color: var(--signal);
  }

  .pick .t {
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
  }

  .pick .s {
    font-size: 13px;
    color: var(--faint);
  }

  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    font-size: 13px;
    color: var(--signal);
  }

  .link:hover {
    text-decoration: underline;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .manual {
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-items: start;
    gap: 14px;
  }

  .manual .end {
    grid-column: 1 / -1;
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .end {
    display: flex;
    justify-content: flex-end;
  }

  .account {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 600;
  }

  .account :global(svg) {
    color: var(--signal);
  }

  .info {
    padding: 10px 14px;
    border: 1px solid rgb(154 140 255 / 0.4);
    border-radius: 10px;
    background: var(--signal-soft);
    font-size: 13px;
  }

  .code {
    height: 78px;
    text-align: center;
    font-size: 34px;
    letter-spacing: 0.14em;
    color: var(--signal);
  }

  .wait {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--ink-0);
    font-size: 14px;
  }

  .pulse {
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background: var(--signal);
    box-shadow: 0 0 0 4px rgb(154 140 255 / 0.18), 0 0 14px var(--signal);
    animation: breathe 1.6s ease-in-out infinite;
  }

  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 7px rgb(154 140 255 / 0.08), 0 0 6px var(--signal);
    }
  }

  .spin {
    width: 14px;
    height: 14px;
    border: 2px solid rgb(255 255 255 / 0.35);
    border-top-color: #fff;
    border-radius: 50%;
    animation: turn 0.8s linear infinite;
  }

  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
</style>
