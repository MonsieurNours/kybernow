<!--
  Prêt de l'ordinateur, côté ami (docs/link-ui.md « Prêt de l'ordinateur ») : une feuille
  pour chaque moment hors de la session.

  - `invite` : le lien d'invitation d'un compte d'ami (kybernow://borrow?…) : « {hôte} vous
    prête son ordinateur. », l'identifiant (déjà rempli), le mot de passe, « Retenir le mot de
    passe », « Ajouter ». Le lien ne porte pas le prénom de l'hôte : le nom de l'ordinateur.
  - `password` : « Emprunter » sans mot de passe retenu : on le redonne (« Retenir le mot de
    passe » proposé tel qu'il était), puis l'emprunt part.
  - `waiting` : l'ordinateur est utilisé, l'hôte doit accepter : « En attente de {hôte}… » et
    « Annuler » ; l'app tient la demande (`lend_wait`, 30 s par appel, rappelé). Accepté : la
    session s'ouvre ; refusé ou sans réponse : le texte du cœur, et « Fermer ».
  - `end` : la carte de fin d'une session d'emprunt : le texte du prêt (« {hôte} a repris
    l'ordinateur. »…) et « Fermer » ; ou, fenêtre fermée ou réseau perdu, « Vous pouvez revenir
    pendant 15 min. » avec « Revenir ».

  Le mot de passe retenu va au gestionnaire d'identifiants de Windows, comme celui d'un ordinateur.
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Switch from './lib/Switch.svelte'
  import type { Friend, LendSheetKind, LendWaitOutcome, LoanReady } from './lib/api'
  import { invoke } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    sheet,
    onclose,
    onadded,
    onpassword,
    onready,
    oncomeback,
  }: {
    sheet: LendSheetKind
    onclose: () => void
    /** L'ordinateur de l'ami est ajouté (son identifiant dans l'app). */
    onadded: (id: string) => void
    /** Le mot de passe est donné : l'emprunt part. */
    onpassword: (friend: Friend, password: string, remember: boolean) => void
    /** L'hôte a accepté : la session s'ouvre. */
    onready: (loan: LoanReady) => void
    /** « Revenir » : la même demande, pendant 15 min. */
    oncomeback: () => void
  } = $props()

  const s = untrack(() => sheet)
  let login = $state(s.kind === 'invite' ? s.invite.login : s.kind === 'password' ? s.friend.login : '')
  let password = $state('')
  let remember = $state(s.kind === 'password' ? s.friend.remember : true)
  let working = $state(false)
  let error = $state<string | null>(null)
  let field = $state<HTMLInputElement | null>(null)
  /** La demande en attente, tant qu'elle l'est. */
  let wait: number | null = s.kind === 'waiting' ? s.wait : null

  onMount(() => {
    field?.focus()
    if (s.kind === 'waiting') follow(s.wait)
  })

  async function add(e: Event) {
    e.preventDefault()
    if (s.kind !== 'invite' || working || !login.trim() || !password) return
    working = true
    error = null
    try {
      onadded(await invoke<string>('lend_add', { link: s.link, candidate: null, login, password, remember }))
    } catch (err) {
      error = String(err)
    } finally {
      working = false
    }
  }

  function give(e: Event) {
    e.preventDefault()
    if (s.kind !== 'password' || !password) return
    onpassword(s.friend, password, remember)
  }

  /** « En attente de {hôte}… » : rappelé tant que l'hôte n'a pas répondu (2 min au plus, ensuite « {hôte} n'a pas répondu. »). */
  async function follow(id: number) {
    try {
      while (wait === id) {
        const r = await invoke<LendWaitOutcome>('lend_wait', { wait: id })
        if (wait !== id) return
        if (r.kind === 'waiting') continue
        wait = null
        if (r.kind === 'ready') onready(r)
        else onclose()
        return
      }
    } catch (err) {
      if (wait === id) {
        wait = null
        error = String(err)
      }
    }
  }

  /** « Annuler » : la demande est retirée chez l'hôte. */
  async function cancel() {
    const id = wait
    wait = null
    onclose()
    if (id !== null) invoke('lend_cancel', { wait: id }).catch(() => {})
  }

  onDestroy(() => {
    if (wait !== null) invoke('lend_cancel', { wait }).catch(() => {})
  })

  function dismiss() {
    if (s.kind === 'waiting' && wait !== null) cancel()
    else if (!working) onclose()
  }

  const title = $derived(
    s.kind === 'invite'
      ? t('lend.invite_title', { host: s.invite.host })
      : s.kind === 'password'
        ? t('lend.computer_of', { host: s.friend.owner })
        : s.kind === 'waiting'
          ? t('lend.computer_of', { host: s.host })
          : t('lend.computer_of', { host: s.end.host }),
  )
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && dismiss()} />

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && dismiss()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="lend-title">
    <button type="button" class="close" aria-label={t('common.close')} disabled={working} onclick={dismiss}><Icon name="x" size={14} /></button>
    <h2 id="lend-title">{title}</h2>

    {#if s.kind === 'invite' || s.kind === 'password'}
      <form onsubmit={s.kind === 'invite' ? add : give}>
        {#if s.kind === 'invite'}<p class="hint">{t('lend.with_friend')}</p>{/if}
        <label class="col">
          <span class="eyebrow">{t('lend.username')}</span>
          <input class="field" autocomplete="username" bind:value={login} readonly={s.kind === 'password'} disabled={working} />
        </label>
        <label class="col">
          <span class="eyebrow">{t('lend.password')}</span>
          <input class="field" type="password" autocomplete="current-password" bind:value={password} bind:this={field} disabled={working} />
        </label>
        <div class="line">
          <Switch checked={remember} label={t('lend.remember')} disabled={working} onchange={(v) => (remember = v)} />
          <span>{t('lend.remember')}</span>
        </div>
        {#if error}<div class="error" role="alert">{error}</div>{/if}
        <div class="row">
          <button type="submit" class="primary" disabled={working || !login.trim() || !password}>
            {s.kind === 'invite' ? t('lend.add') : t('lend.borrow')}
          </button>
        </div>
      </form>
    {:else if s.kind === 'waiting'}
      {#if error}
        <div class="error" role="alert">{error}</div>
        <div class="row"><button type="button" class="ghost" onclick={onclose}>{t('common.close')}</button></div>
      {:else}
        <div class="wait" role="status"><span class="pulse"></span>{t('lend.waiting', { host: s.host })}</div>
        <div class="row"><button type="button" class="ghost" onclick={cancel}>{t('common.cancel')}</button></div>
      {/if}
    {:else}
      {#if s.end.come_back}
        <p class="lead">{t('lend.come_back_hint')}</p>
      {:else if s.end.text}
        <p class="lead">{s.end.text}</p>
      {/if}
      <div class="row">
        {#if s.end.come_back}<button type="button" class="primary" onclick={oncomeback}><Icon name="play" size={16} />{t('lend.come_back')}</button>{/if}
        <button type="button" class="ghost" onclick={onclose}>{t('common.close')}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    z-index: 30;
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
    width: min(500px, 100%);
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 36px 40px 40px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
    animation: rise 0.32s var(--ease-out);
  }

  @keyframes rise {
    from {
      transform: translateY(18px) scale(0.985);
      opacity: 0;
    }
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  h2 {
    margin: 0;
    padding-right: 36px;
    font-size: 25px;
    font-weight: 640;
    font-stretch: 125%;
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
    margin: -8px 0 0;
    font-size: 14px;
    color: var(--muted);
  }

  .lead {
    margin: 0;
    font-size: 16px;
    line-height: 1.5;
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 14px;
  }

  .row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .row .primary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
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
</style>
