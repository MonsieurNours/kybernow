<!--
  Réglages de l'app : version, langue, ouverture à l'ouverture de session
  (sans fenêtre, dans l'icône près de l'horloge), canal des mises à jour,
  recherche et installation ; ce PC (ce que sa carte graphique décode, vérifié au
  lancement, son écran, son son) et « Sur batterie », qui vaut pour toutes
  les sessions.

  « Comptes » (docs/link-ui.md « Comptes KyberNow ») : chaque compte connecté (nom, e-mail, relais),
  « Gérer le compte » (sa page sur le relais, dans le navigateur) et « Se déconnecter » (confirmé :
  ses ordinateurs quittent cet appareil, les appairés restent) ; « Ajouter un compte » demande d'abord
  le relais (share.kybernow.app proposé, ou « Un autre relais… » tapé), puis la page du relais s'ouvre.
  L'app ne crée ni ne supprime de compte : la page du compte le fait.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Mark from './lib/Mark.svelte'
  import Segmented from './lib/Segmented.svelte'
  import Switch from './lib/Switch.svelte'
  import type { AccountView, AppInfo, Battery, ThisPc, Update } from './lib/api'
  import { DEFAULT_RELAY, invoke } from './lib/api'
  import { setLang, t } from './lib/i18n/index.svelte'

  let {
    update,
    progress,
    thisPc,
    battery,
    check: checkOnOpen = false,
    accounts = [],
    signingIn = false,
    signInError = null,
    onsignin,
    oncancelsignin,
    onbattery,
    onclose,
    oninstall,
  }: {
    update: Update | null
    progress: number | null
    thisPc: ThisPc | null
    battery: Battery
    /** Ouverts par « Rechercher des mises à jour » (menu de l'icône) : la recherche part tout de suite. */
    check?: boolean
    /** Les comptes KyberNow connectés sur ce PC. */
    accounts?: AccountView[]
    /** « Connexion au compte… » : la page du relais est ouverte, l'app attend son retour. */
    signingIn?: boolean
    /** Le refus de la dernière connexion (retour de la page du relais). */
    signInError?: string | null
    /** Ouvre la page de connexion du relais (`null` : le relais public) ; rend le refus, ou `null`. */
    onsignin: (relay: string | null) => Promise<string | null>
    oncancelsignin: () => void
    onbattery: (b: Battery) => void
    onclose: () => void
    oninstall: () => void
  } = $props()

  const decodes = $derived(
    thisPc
      ? [
          { k: 'H.264', v: thisPc.h264 },
          { k: 'HEVC', v: thisPc.hevc },
          { k: 'HEVC 4:4:4', v: thisPc.hevc444 },
          { k: 'AV1', v: thisPc.av1 },
        ]
      : [],
  )

  let info = $state<AppInfo | null>(null)
  let checking = $state(false)
  let checked = $state(false)
  let error = $state<string | null>(null)

  onMount(() => {
    invoke<AppInfo>('app_info').then((i) => (info = i))
    if (checkOnOpen && !update) check()
  })

  async function run(f: () => Promise<unknown>) {
    checking = true
    error = null
    try {
      await f()
      checked = true
    } catch (e) {
      error = String(e)
    } finally {
      checking = false
    }
  }

  /** Auto (langue de Windows), Français, English : appliquée tout de suite. */
  async function language(l: AppInfo['language']) {
    if (info) info.language = l
    try {
      setLang(await invoke<string>('set_language', { language: l }))
      // Les textes du cœur (« Synchroniser les réglages des ordinateurs ») suivent la langue.
      info = await invoke<AppInfo>('app_info')
    } catch (e) {
      error = String(e)
    }
  }

  const check = () => run(() => invoke<Update | null>('check_update'))

  /** « Synchroniser les réglages des ordinateurs » (comptes KyberNow) : gardé sur ce PC. */
  async function syncSettings(on: boolean) {
    if (info) info.sync_settings = on
    try {
      const now = await invoke<boolean>('set_sync_settings', { on })
      if (info) info.sync_settings = now
    } catch (e) {
      accountError = String(e)
    }
  }

  /** Ouvrir à l'ouverture de session : l'état rendu est celui que Windows suivra. */
  async function openAtLogin(on: boolean) {
    if (info) info.open_at_login = on
    try {
      const now = await invoke<boolean>('set_open_at_login', { on })
      if (info) info.open_at_login = now
    } catch (e) {
      error = String(e)
    }
  }
  // MARK: - Comptes

  /** « Ajouter un compte » : le choix du relais, puis, pour « Un autre relais… », son adresse. */
  let adding = $state<'relay' | 'other' | null>(null)
  let otherRelay = $state('')
  let accountError = $state<string | null>(null)
  /** « Se déconnecter » demandé pour ce compte : la confirmation. */
  let confirmOut = $state<string | null>(null)
  /** Une déconnexion en cours (le relais a 8 s pour répondre ; le compte est oublié de toute façon). */
  let signingOut = $state<string | null>(null)

  /** La page de connexion du relais (`relay` nul : le relais public) ; un refus reste sous les boutons. */
  async function signIn(relay: string | null) {
    accountError = await onsignin(relay)
    if (!accountError) adding = null
  }

  async function manage(key: string) {
    accountError = null
    try {
      await invoke('account_manage', { key })
    } catch (e) {
      accountError = String(e)
    }
  }

  async function signOut(key: string) {
    confirmOut = null
    signingOut = key
    try {
      await invoke('account_sign_out', { key })
    } finally {
      signingOut = null
    }
  }

  const channel = (c: 'stable' | 'beta') =>
    run(async () => {
      if (info) info.channel = c
      await invoke<Update | null>('set_channel', { channel: c })
    })
</script>

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="app-title">
    <button class="close" aria-label={t('common.close')} onclick={onclose}><Icon name="x" size={14} /></button>
    <h2 id="app-title"><Mark product="link" size={40} /></h2>
    <div class="mono ver">{t('appsheet.version', { version: info?.version ?? '…', arch: info?.arch ?? '' })}</div>

    <div class="line">
      <span>{t('appsheet.language')}</span>
      <Segmented
        label="Langue / Language"
        value={info?.language ?? 'auto'}
        options={[
          { value: 'auto', label: 'Auto' },
          { value: 'fr', label: 'Français' },
          { value: 'en', label: 'English' },
        ]}
        onchange={language}
      />
    </div>

    <div class="line">
      <span>{t('appsheet.open_at_login')}</span>
      <Switch label={t('appsheet.open_at_login')} checked={info?.open_at_login ?? false} disabled={!info} onchange={openAtLogin} />
    </div>
    <p class="help">{t('appsheet.open_at_login_help')}</p>

    <div class="line">
      <span>{t('appsheet.offered')}</span>
      <Segmented
        label={t('appsheet.channel')}
        value={info?.channel ?? 'stable'}
        options={[
          { value: 'stable', label: t('appsheet.stable') },
          { value: 'beta', label: t('appsheet.beta') },
        ]}
        onchange={channel}
      />
    </div>

    {#if update}
      <div class="update">
        <div>
          <div class="eyebrow">{t('appsheet.update')}</div>
          <div class="uv">KyberNow Link {update.version}{update.channel === 'beta' ? t('appsheet.beta_suffix') : ''}</div>
        </div>
        {#if progress !== null}
          <div class="bar" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100}><span style:width="{progress}%"></span></div>
        {:else}
          <button class="primary" onclick={oninstall}>{t('app.install_relaunch')}</button>
        {/if}
      </div>
      {#if update.notes}<pre class="notes">{update.notes}</pre>{/if}
    {:else}
      <div class="row">
        <button class="ghost" disabled={checking} onclick={check}>{checking ? t('common.searching') : t('appsheet.check')}</button>
        {#if checked && !error}<span class="ok">{t('appsheet.up_to_date')}</span>{/if}
      </div>
    {/if}
    {#if error}<div class="error">{error}</div>{/if}

    <p class="help">{t('appsheet.help')}</p>

    <div class="pc accounts">
      <div class="eyebrow">{t('account.section')}</div>
      {#each accounts as a (a.key)}
        <div class="account">
          <div class="who">
            <div class="aname">{a.name}</div>
            {#if a.email && a.email !== a.name}<div class="email">{a.email}</div>{/if}
            <div class="mono relay">{a.relay}</div>
          </div>
          <div class="buttons">
            {#if confirmOut === a.key}
              <button class="ghost danger" onclick={() => signOut(a.key)}>{t('account.sign_out')}</button>
              <button class="ghost" onclick={() => (confirmOut = null)}>{t('common.cancel')}</button>
            {:else}
              <button class="ghost" onclick={() => manage(a.key)}>{t('account.manage')}</button>
              <button class="ghost" disabled={signingOut === a.key} onclick={() => (confirmOut = a.key)}>{t('account.sign_out')}</button>
            {/if}
          </div>
        </div>
        {#if confirmOut === a.key}<p class="help confirm">{t('account.sign_out_confirm', { account: a.name })}</p>{/if}
      {:else}
        <p class="help">{t('account.none')}</p>
      {/each}

      {#if signingIn}
        <div class="row">
          <button class="ghost" disabled>{t('account.signing_in')}</button>
          <button class="ghost" onclick={oncancelsignin}>{t('common.cancel')}</button>
        </div>
      {:else if accounts.length === 0}
        <div class="row">
          <button class="primary" onclick={() => signIn(null)}>{t('account.sign_in')}</button>
          <button class="ghost" onclick={() => ((adding = 'other'), (accountError = null))}>{t('account.another_relay')}</button>
        </div>
      {:else if adding === 'relay'}
        <!-- Le relais d'abord : le relais public, ou un autre. -->
        <div class="row">
          <button class="primary" onclick={() => signIn(null)}>{DEFAULT_RELAY}</button>
          <button class="ghost" onclick={() => (adding = 'other')}>{t('account.another_relay')}</button>
          <button class="ghost" onclick={() => (adding = null)}>{t('common.cancel')}</button>
        </div>
      {:else if adding !== 'other'}
        <div class="row">
          <button class="ghost" onclick={() => ((adding = 'relay'), (accountError = null))}>{t('account.add')}</button>
        </div>
      {/if}

      {#if adding === 'other' && !signingIn}
        <form class="other" onsubmit={(e) => (e.preventDefault(), signIn(otherRelay.trim()))}>
          <div class="otitle">{t('account.another_relay_title')}</div>
          <label class="relayfield">
            <span>{t('account.relay')}</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input class="field mono" bind:value={otherRelay} placeholder={t('account.relay_example')} autocomplete="off" spellcheck="false" autofocus />
          </label>
          <p class="help">{t('account.relay_help')}</p>
          <div class="row">
            <button class="primary" type="submit" disabled={!otherRelay.trim()}>{t('common.continue')}</button>
            <button class="ghost" type="button" onclick={() => ((adding = null), (accountError = null))}>{t('common.cancel')}</button>
          </div>
        </form>
      {/if}

      {#if accounts.length > 0 && info}
        <div class="line">
          <span>{info.sync_settings_title}</span>
          <Switch label={info.sync_settings_title} checked={info.sync_settings} onchange={syncSettings} />
        </div>
        <p class="help">{info.sync_settings_help}</p>
      {/if}

      {#if accountError || signInError}<div class="error">{accountError ?? signInError}</div>{/if}
      <p class="help">{t('account.sign_in_help')}</p>
      <p class="help">{t('account.section_help')}</p>
    </div>

    <div class="pc">
      <div class="eyebrow">{t('appsheet.this_pc')}</div>
      {#if thisPc}
        <div class="pcname">{thisPc.name}</div>
        <div class="mono gpu">{thisPc.gpu || t('appsheet.unknown_gpu')}</div>
        <div class="table mono">
          {#each decodes as d (d.k)}
            <div class="tr"><span>{d.k}</span><span class:ok={d.v} class:no={!d.v}>{d.v ? t('appsheet.hardware') : d.k === 'AV1' ? t('appsheet.no') : t('appsheet.processor')}</span></div>
          {/each}
          <div class="tr"><span>{t('appsheet.display')}</span><span>{thisPc.width} × {thisPc.height} · {thisPc.refresh} Hz</span></div>
          <div class="tr"><span>{t('appsheet.sound')}</span><span>{thisPc.audio_channels === 8 ? '7.1' : thisPc.audio_channels === 6 ? '5.1' : t('appsheet.stereo')}</span></div>
        </div>
        <p class="help">{t('appsheet.probe_help')}</p>
        <div class="line">
          <span class="battery"><Icon name="battery" size={16} /> {t('appsheet.battery')}</span>
          <Segmented label={t('appsheet.battery')} value={battery} options={[{ value: 'alleger', label: t('appsheet.lighten') }, { value: 'garder', label: t('appsheet.keep') }]} onchange={onbattery} />
        </div>
        <p class="help">{t('appsheet.battery_help')}{thisPc.on_battery ? t('appsheet.on_battery_now') : ''}</p>
      {:else}
        <p class="help">{t('appsheet.probing')}</p>
      {/if}
    </div>
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
  }

  .sheet {
    position: relative;
    width: min(620px, 100%);
    max-height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 32px 36px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
  }

  /* La feuille défile : ses blocs gardent leur hauteur (les notes ne s'écrasent pas). */
  .sheet > * {
    flex: none;
  }

  .close {
    position: absolute;
    right: 14px;
    top: 14px;
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

  h2 {
    margin: 0;
    display: flex;
  }

  .ver {
    font-size: 12px;
    color: var(--muted);
    margin-top: -10px;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    font-size: 14px;
  }

  .update {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 14px;
    padding: 16px 18px;
    border: 1px solid var(--signal);
    border-radius: 14px;
    background: var(--signal-soft);
  }

  .uv {
    margin-top: 4px;
    font-size: 18px;
    font-weight: 600;
  }

  .bar {
    width: 220px;
    height: 6px;
    border-radius: 3px;
    background: var(--line-2);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--signal);
    transition: width 0.2s;
  }

  .notes {
    margin: 0;
    max-height: 200px;
    overflow-y: auto;
    padding: 12px 14px;
    border-radius: 10px;
    background: var(--ink-0);
    font-family: var(--sans);
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
    white-space: pre-wrap;
    user-select: text;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }

  .ok {
    font-size: 13px;
    color: var(--ok);
  }

  .help {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--faint);
  }

  .pc {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
  }

  .pcname {
    font-size: 18px;
    font-weight: 600;
  }

  .gpu {
    margin-top: -4px;
    font-size: 11px;
    line-height: 1.6;
    color: var(--muted);
  }

  .table {
    font-size: 12px;
  }

  .tr {
    display: flex;
    justify-content: space-between;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
  }

  .tr span:last-child {
    color: var(--text);
  }

  .tr span.ok {
    color: var(--ok);
  }

  .tr span.no {
    color: var(--faint);
  }

  .battery {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* Comptes KyberNow : un compte par ligne, ses boutons à droite. */
  .account {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }

  .who {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .aname {
    font-size: 15px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .email {
    font-size: 13px;
    color: var(--muted);
    overflow-wrap: anywhere;
  }

  .relay {
    font-size: 11px;
    color: var(--faint);
  }

  .buttons {
    display: flex;
    gap: 8px;
  }

  .danger {
    color: var(--bad, #ff6b6b);
  }

  .confirm {
    color: var(--muted);
  }

  .other {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border: 1px solid var(--line-2);
    border-radius: 14px;
  }

  .otitle {
    font-size: 15px;
    font-weight: 600;
  }

  .relayfield {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--muted);
  }
</style>
