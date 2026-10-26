<!--
  Connexion d'une session ouverte pendant qu'une autre tourne (plusieurs
  ordinateurs, docs/plusieurs-ordinateurs.md et docs/link-ui.md) : une fenêtre
  à la place de celle de la session, sur l'écran de la fenêtre active, avec le
  même passage que l'accueil (étapes, « La Station redémarre… », Annuler). À
  la première image, la fenêtre de la session se montre par-dessus et le
  passage s'ouvre ; l'app ferme ensuite cette fenêtre (src/ui.rs,
  `open_connexion`). Si la connexion échoue, le passage se referme et la
  raison reste, avec un seul bouton « Fermer », qui ferme la fenêtre. Fermer
  la fenêtre pendant la connexion, c'est « Annuler ».
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Passage from './Passage.svelte'
  import type { AppInfo, Computer, ConnexionView, Phase, ScreensEvent, Snapshot, StationRestarting, Summary } from './lib/api'
  import { invoke, listen } from './lib/api'
  import { setLang, t } from './lib/i18n/index.svelte'

  /** L'ordinateur de cette connexion (donné par l'app ; `?id=` dans l'imitation). */
  const id: string = (window as unknown as { __KN_SESSION?: string }).__KN_SESSION ?? new URLSearchParams(location.search).get('id') ?? ''

  let computer = $state<Computer | null>(null)
  let phase = $state<Phase['phase']>('preparing')
  let restarting = $state(false)
  let opening = $state(false)
  /** La connexion a échoué avant la première image : la raison. */
  let error = $state<string | null>(null)

  /** Connectée mais sans image encore : le passage attend la première (comme l'accueil). */
  const step = (p: Phase['phase']) => (p === 'streaming' && !opening ? 'first_images' : p)

  onMount(() => {
    const off = [
      listen<Phase>('session-status', (p) => {
        if (p.id !== id) return
        restarting = false
        phase = step(p.phase)
      }),
      listen<StationRestarting>('session-station-restarting', (r) => {
        if (r.id === id) restarting = true
      }),
      listen<string>('session-shown', (s) => {
        if (s === id) opening = true
      }),
      // La connexion reprend en session : les étapes reviennent.
      listen<ScreensEvent>('session-replan', (r) => {
        if (r.id !== id) return
        opening = false
        restarting = false
        phase = 'preparing'
      }),
      listen<{ id: string; summary: Summary }>('session-ended', ({ id: ended, summary }) => {
        if (ended === id && !opening && summary.error) error = summary.error
      }),
      listen<string>('lang', setLang),
    ]
    // L'état du moment une fois les écoutes en place (rien ne passe entre deux).
    Promise.all(off).then(() => {
      invoke<ConnexionView | null>('connexion_state').then((v) => {
        if (!v) return
        phase = step(v.phase)
        restarting = v.restarting
        error = v.error
      })
      invoke<Snapshot>('snapshot').then((s) => (computer = s.computers.find((c) => c.id === id) ?? null))
      invoke<AppInfo>('app_info').then((i) => setLang(i.lang))
    })
    return () => off.forEach((p) => p.then((f) => f()))
  })

  /** « Annuler », ou « Fermer » sous un échec : l'app arrête la connexion ou ferme la fenêtre. */
  const close = () => invoke('connexion_close')
</script>

{#if computer}
  <Passage {computer} {phase} {opening} closing={error !== null} {restarting} oncancel={close} />
{/if}

{#if error}
  <div class="failed" role="alert">
    <p>{t('app.connect_failed', { error })}</p>
    <button onclick={close}>{t('common.close')}</button>
  </div>
{/if}

<style>
  .failed {
    position: absolute;
    left: 50%;
    bottom: 72px;
    z-index: 50;
    transform: translateX(-50%);
    width: min(620px, calc(100% - 48px));
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 14px 14px 14px 22px;
    border: 1px solid var(--line-2);
    border-radius: 18px;
    background: rgb(9 13 18 / 0.94);
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.5);
    animation: up 0.4s var(--ease-out) 0.3s both;
  }

  @keyframes up {
    from {
      opacity: 0;
      transform: translate(-50%, 12px);
    }
  }

  .failed p {
    flex: 1;
    margin: 0;
    font-size: 14px;
    line-height: 1.5;
    color: var(--bad-text);
  }

  .failed button {
    flex: none;
    height: 40px;
    padding: 0 20px;
    border: 0;
    border-radius: 999px;
    background: var(--signal-strong);
    color: #fff;
    font-size: 14px;
    font-weight: 600;
  }
</style>
