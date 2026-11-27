<!--
  Le panneau « Partager » de l'hôte (KyberNow Share) : le code, son lien et son QR code, les demandes, les invités et leurs
  droits, « Laisser les invités parler », « Nouveau code », « Arrêter le partage ». Ce que fait chaque bouton reste dans la
  barre (Bar.svelte, `on`) ; styles : bar/panels.css.
-->
<script lang="ts">
  import Switch from '../lib/Switch.svelte'
  import type { Share, ShareGuest, ShareRole } from '../lib/api'
  import { roleKbm, roleOf, rolePads } from '../lib/api'
  import { t } from '../lib/i18n/index.svelte'
  import type { RelayPhase } from '../lib/relay.svelte'
  import CodeTop from './CodeTop.svelte'
  import RelayLine from './RelayLine.svelte'

  let {
    share,
    name,
    relay,
    copied,
    acting,
    guestMeta,
    requestMeta,
    on,
  }: {
    share: Share | null
    name: string
    relay: RelayPhase
    copied: 'ok' | 'fail' | null
    acting: boolean
    guestMeta: (g: ShareGuest) => string
    requestMeta: (g: ShareGuest) => string
    on: {
      copy: () => void
      accept: (g: ShareGuest) => void
      refuse: (g: ShareGuest) => void
      setRole: (g: ShareGuest, role: ShareRole) => void
      kick: (g: ShareGuest) => void
      guestsSpeak: (on: boolean) => void
      newCode: () => void
      stop: () => void
    }
  } = $props()
</script>

{#if share?.open}
  <h3>{t('share.title', { name: name })}</h3>
  <CodeTop code={share.code} qr={share.qr} address={share.web || share.link} phase={relay} done={copied} copy={on.copy} />
  <!-- Relais joint : le partage vaut aussi hors de chez vous, et l'hôte accepte chaque invité venu de loin. -->
  <p class="help">{t(relay === 'joined' ? 'share.help_relay' : 'share.help')}</p>
  <RelayLine phase={relay} />
  {#if share.requests.length > 0}
    <div class="eyebrow list-title">{t('share.requests')}</div>
    <ul class="guests requests">
      {#each share.requests as g (g.id)}
        <li>
          <div class="who">
            <span class="gname">{t('share.wants_to_join', { name: g.name })}</span>
            <span class="gmeta">{requestMeta(g)}</span>
          </div>
          <div class="acts">
            <!-- Quatre invités ont leur place : « Accepter » est inactif (la raison est dessous : « Quatre invités au plus. »), « Refuser » reste. -->
            <button class="accept" aria-disabled={acting || share.full} onclick={() => on.accept(g)}>{t('share.accept')}</button>
            <button class="danger" aria-disabled={acting} onclick={() => on.refuse(g)}>{t('share.decline')}</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="eyebrow list-title">{t('share.guests')}</div>
  {#if share.guests.length === 0}
    <p class="nobody">{t('share.nobody')}</p>
  {:else}
    <ul class="guests">
      {#each share.guests as g (g.id)}
        <li>
          <div class="who">
            <span class="gname">{t(`share.guest_${g.role}`, { name: g.name })}</span>
            <span class="gmeta">{guestMeta(g)}</span>
          </div>
          <div class="acts">
            <!-- Deux droits indépendants, chacun donné ou repris sans toucher à l'autre : les manettes, puis le
                 clavier et la souris. Pas d'autre clic tant qu'Admin n'a pas répondu : le rôle qui part se
                 déduit de celui qui est affiché. -->
            <button aria-disabled={acting} onclick={() => on.setRole(g, roleOf(!rolePads(g.role), roleKbm(g.role)))}>{t(rolePads(g.role) ? 'share.take_pads' : 'share.give_pads')}</button>
            <button aria-disabled={acting} onclick={() => on.setRole(g, roleOf(rolePads(g.role), !roleKbm(g.role)))}>{t(roleKbm(g.role) ? 'share.take_kbm' : 'share.give_kbm')}</button>
            <button class="danger" aria-disabled={acting} onclick={() => on.kick(g)}>{t('share.remove')}</button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
  {#if share.full}<p class="note" role="note">{t('share.limit')}</p>{/if}
  {#if share.guests_speak != null}
    <!-- Micro des invités : coupé à chaque nouveau partage, vaut tout de suite pour ceux déjà là. -->
    <div class="speak">
      <span class="speak-title">{t('micro.guests')}</span>
      <Switch label={t('micro.guests')} checked={share.guests_speak} disabled={acting} onchange={on.guestsSpeak} />
    </div>
    <p class="speak-help">{t('micro.guests_help')}</p>
  {/if}
  <div class="foot">
    <button class="ghost" onclick={on.newCode}>{t('share.new_code')}</button>
    <button class="ghost danger" onclick={on.stop}>{t('share.stop')}</button>
  </div>
{:else if share?.text}
  <!-- « Partager » inactif : Station trop ancienne, partage coupé sur l'ordinateur. -->
  <p class="note" role="note">{share.text}</p>
{/if}

<style>
  /* Le style de base des boutons d'un menu de la barre (Bar.svelte, `.menu button`), pour les boutons de ce composant. */
  :global(.menu) button {
    display: grid;
    grid-template-columns: 18px auto 1fr;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text);
    text-align: left;
    font-size: 13px;
  }

  :global(.menu) button:hover {
    background: rgb(255 255 255 / 0.07);
  }
</style>
