<!--
  Clés d'Admin à distance (STWIN-17 ; docs/comptes.md, « Admin depuis KyberNow Share », et docs/relais.md, « Clé
  d'Admin à distance ») : sous l'interrupteur « Admin à distance », la liste des clés (une passkey par appareil qui
  l'a ajoutée), « Retirer », et « Ajouter une clé » : un QR code du lien d'ajout, à scanner avec le téléphone qui
  gardera la clé, ou à ouvrir sur cet appareil ; il sert une fois, pendant 10 minutes. Seulement avec un Admin qui
  connaît les clés (`remote_keys` de `GET /api/relay`, 0.11.8) ; l'ajout se fait sur le réseau, jamais par le relais.
-->
<script lang="ts">
  import { api, type RelayStatus } from './api'
  import { dayOf } from './format'
  import { t } from './i18n/index.svelte'
  import Qr from './Qr.svelte'
  import { errorText, notify } from './store.svelte'

  let { relay, onchange, reload }: { relay: RelayStatus; onchange: (r: RelayStatus) => void; reload: () => void } = $props()

  const keys = $derived(relay.remote_keys ?? [])
  /** L'ajout se fait sur place, de bout en bout prêt (sinon Admin répond `409 not_ready` ou `403 network_only`). */
  const canAdd = $derived(!relay.via_relay && relay.remote_tls?.state === 'ready')
  /** Le lien d'ajout montré (QR code), le temps de la fenêtre. */
  let adding = $state<string | null>(null)
  let busy = $state(false)

  async function add() {
    busy = true
    try {
      adding = (await api.addRemoteKey()).url
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  async function remove(id: string) {
    busy = true
    try {
      onchange(await api.removeRemoteKey(id))
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      busy = false
    }
  }

  /** « Terminé » : le QR code se ferme, la liste est relue (la clé ajoutée sur le téléphone y paraît). */
  function done() {
    adding = null
    reload()
  }

  /** « iPhone de Léa · ajoutée le 27 novembre · utilisée le 28 novembre (autre relais) ». */
  function line(k: NonNullable<RelayStatus['remote_keys']>[number]): string {
    let s = t('devices.away.keys.added', { device: k.name, date: dayOf(k.added) })
    if (k.used) s += t('devices.away.keys.used', { date: dayOf(k.used) })
    if (!k.current) s += ` ${t('devices.away.keys.otherRelay')}`
    return s
  }
</script>

<div class="keys">
  <h4>{t('devices.away.keys.title')}</h4>
  <p>{t('devices.away.keys.help')}</p>
  {#if keys.length}
    <ul>
      {#each keys as k (k.id)}
        <li>
          <span>{line(k)}</span>
          <button type="button" class="ghost small" disabled={busy} onclick={() => remove(k.id)}>{t('devices.away.keys.remove')}</button>
        </li>
      {/each}
    </ul>
  {:else if relay.remote_admin}
    <p class="muted">{t('devices.away.keys.none')}</p>
  {/if}
  {#if adding}
    <div class="adding">
      <Qr text={adding} label={t('devices.away.keys.add')} />
      <div>
        <p>{t('devices.away.keys.scan')}</p>
        <a class="ghost small" href={adding} target="_blank" rel="noopener">{t('devices.away.keys.openHere')}</a>
        <button type="button" class="primary small" onclick={done}>{t('devices.away.keys.done')}</button>
      </div>
    </div>
  {:else}
    <button type="button" class="ghost small" disabled={busy || !canAdd} onclick={add}>{t('devices.away.keys.add')}</button>
  {/if}
</div>

<style>
  .keys {
    margin-top: 14px;
    display: grid;
    gap: 8px;
  }

  h4 {
    margin: 0;
    font-size: 14px;
  }

  p {
    margin: 0;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .adding {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .adding div {
    display: grid;
    gap: 10px;
    max-width: 320px;
  }

  .keys > button,
  .adding a,
  .adding button {
    justify-self: start;
  }
</style>
