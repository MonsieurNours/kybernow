<!--
  Fenêtres de KyberNow Station pour Mac : le parcours d'installation (colonne
  et écrans), ou une fenêtre seule pour la mise à jour, la désinstallation et
  les autorisations, ouvertes depuis l'icône de la barre des menus.
-->
<script lang="ts">
  import { fade, fly } from 'svelte/transition'
  import { mode } from './bridge'
  import Rail from './Rail.svelte'
  import { app, reducedMotion, startBackend } from './state.svelte'
  import About from './screens/About.svelte'
  import Account from './screens/Account.svelte'
  import Done from './screens/Done.svelte'
  import Install from './screens/Install.svelte'
  import Options from './screens/Options.svelte'
  import Permissions from './screens/Permissions.svelte'
  import PermissionsWindow from './screens/PermissionsWindow.svelte'
  import Reinstall from './screens/Reinstall.svelte'
  import Uninstall from './screens/Uninstall.svelte'
  import Update from './screens/Update.svelte'
  import Welcome from './screens/Welcome.svelte'
  import './mac.css'

  startBackend()

  const screens = { welcome: Welcome, reinstall: Reinstall, account: Account, options: Options, install: Install, permissions: Permissions, done: Done, about: About }
  const Screen = $derived(screens[app.screen])
  const inMotion = reducedMotion ? { duration: 0 } : { x: 12, duration: 420, delay: 90 }
  const outMotion = reducedMotion ? { duration: 0 } : { duration: 90 }
</script>

<div class="mac">
  {#if mode === 'update'}
    <Update />
  {:else if mode === 'uninstall'}
    <Uninstall />
  {:else if mode === 'permissions'}
    <PermissionsWindow />
  {:else}
    <div class="installer">
      <Rail />
      <div class="stage">
        {#key app.screen}
          <main class="screen" in:fly={inMotion} out:fade={outMotion}>
            <Screen />
          </main>
        {/key}
      </div>
    </div>
  {/if}
</div>

<style>
  .installer {
    height: 100%;
    display: flex;
  }

  .stage {
    flex: 1;
    min-width: 0;
    display: grid;
  }

  .screen {
    grid-area: 1 / 1;
    min-height: 0;
    padding: 48px 56px 32px;
    display: flex;
    flex-direction: column;
  }
</style>
