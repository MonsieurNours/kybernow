<!--
  Invitation (docs/invitations.md, textes de docs/link-ui.md « Invitations ») : « Rejoindre avec un
  code » est tombé sur un code d'invitation. La feuille « Ajouter {ordinateur} » (« Ajouter
  l'ordinateur » quand la Station ne dit pas son nom) demande le mot de passe donné avec le code
  (masqué, avec « Afficher » / « Masquer » à droite du champ ; exemple grisé `xxxx-xxxx-xxxx`, ni
  correction ni majuscule automatiques ; tirets, espaces et majuscules ne comptent pas). « Ajouter l'ordinateur » (inactif tant que le champ est vide) :
  « Ajout de {ordinateur}… » pendant l'appel (`invite_redeem`, 15 s au plus), puis l'ordinateur
  apparaît sur l'accueil, sélectionné, avec l'avis « {ordinateur} est ajouté. » (App.svelte). Un
  refus se lit sous le champ (« Mot de passe refusé. », « Ce code ne sert plus. »…), et l'on peut
  réessayer sans retaper le code.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { InviteAdded, InviteRefusal } from './lib/api'
  import { invoke } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    ticket,
    computer,
    onclose,
    onadded,
  }: {
    /** Le ticket de `share_join` (`invite`). */
    ticket: number
    /** Le nom de l'ordinateur ; vide quand la Station ne le dit pas. */
    computer: string
    onclose: () => void
    /** L'ordinateur est ajouté (ou a pris le nouvel accès) : son identifiant et son nom. */
    onadded: (added: InviteAdded) => void
  } = $props()

  let password = $state('')
  /** « Afficher » : le mot de passe en clair, le temps de le relire. */
  let shown = $state(false)
  let working = $state(false)
  let error = $state<string | null>(null)
  let field = $state<HTMLInputElement | null>(null)

  onMount(() => field?.focus())

  const title = $derived(computer ? t('invite.title', { name: computer }) : t('invite.title_unnamed'))
  const adding = $derived(computer ? t('invite.adding', { name: computer }) : t('invite.adding_unnamed'))

  async function add(e: Event) {
    e.preventDefault()
    if (working || !password.trim()) return
    working = true
    error = null
    try {
      onadded(await invoke<InviteAdded>('invite_redeem', { ticket, password }))
    } catch (err) {
      const r = err as InviteRefusal | string
      error = typeof r === 'object' && r && 'message' in r ? r.message : String(r)
      field?.focus()
    } finally {
      working = false
    }
  }

  /** Échap, la croix, « Annuler » : rien pendant l'appel (il finit en quelques secondes). */
  function dismiss() {
    if (!working) onclose()
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && dismiss()} />

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && dismiss()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="invite-title">
    <button type="button" class="close" aria-label={t('common.close')} disabled={working} onclick={dismiss}><Icon name="x" size={14} /></button>
    <h2 id="invite-title">{title}</h2>
    <p class="hint">{t('invite.help')}</p>

    <form onsubmit={add}>
      <div class="col">
        <label class="eyebrow" for="invite-password">{t('invite.password')}</label>
        <span class="secret">
          <input
            id="invite-password"
            class="field mono"
            type={shown ? 'text' : 'password'}
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
            maxlength="40"
            placeholder="xxxx-xxxx-xxxx"
            bind:value={password}
            bind:this={field}
            oninput={() => (error = null)}
            disabled={working}
          />
          <button type="button" class="reveal" aria-pressed={shown} disabled={working} onclick={() => ((shown = !shown), field?.focus())}>
            {shown ? t('invite.hide') : t('invite.show')}
          </button>
        </span>
      </div>

      {#if working}<div class="wait" role="status"><span class="pulse"></span>{adding}</div>{/if}
      {#if error}<div class="error" role="alert">{error}</div>{/if}

      <div class="row">
        <button type="submit" class="primary" disabled={working || !password.trim()}>{t('invite.add')}</button>
        <button type="button" class="ghost" disabled={working} onclick={dismiss}>{t('common.cancel')}</button>
      </div>
    </form>
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
    line-height: 1.55;
    color: var(--muted);
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .secret {
    position: relative;
    display: flex;
  }

  .secret .field {
    flex: 1;
    padding-right: 104px;
  }

  .reveal {
    position: absolute;
    right: 6px;
    top: 50%;
    transform: translateY(-50%);
    height: 32px;
    padding: 0 12px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
  }

  .reveal:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.06);
    color: var(--text);
  }

  .row {
    display: flex;
    gap: 12px;
    align-items: center;
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
