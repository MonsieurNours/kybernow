<script lang="ts">
  import { api, type UpdateStatus } from '../lib/api'
  import { errorText, notify } from '../lib/store.svelte'

  let status = $state<UpdateStatus | null>(null)
  let checking = $state(false)
  // Installation lancée : on attend que la console revienne dans la nouvelle version.
  let installing = $state<string | null>(null)
  let waited = $state(0)

  async function load() {
    try {
      status = await api.update()
      // Jamais vérifiée depuis le démarrage de la console : on demande à GitHub.
      if (status.checked_seconds_ago === null) await check()
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  async function check() {
    checking = true
    try {
      status = await api.updateCheck()
      if (status.error) notify('error', `GitHub : ${status.error}`)
    } catch (e) {
      notify('error', errorText(e))
    } finally {
      checking = false
    }
  }

  async function install() {
    if (!status?.latest) return
    try {
      const r = await api.updateInstall()
      installing = r.version
      waitForRestart(r.version)
    } catch (e) {
      notify('error', errorText(e))
    }
  }

  // La console s'arrête, remplace ses fichiers et repart : on la guette.
  function waitForRestart(version: string) {
    const started = Date.now()
    const timer = setInterval(async () => {
      waited = Math.round((Date.now() - started) / 1000)
      try {
        const s = await api.update()
        if (s.current === version) {
          clearInterval(timer)
          installing = null
          status = s
          notify('ok', `KyberNow ${version} est installé.`)
        }
      } catch {
        // console en cours de redémarrage
      }
      if (waited > 300) {
        clearInterval(timer)
        installing = null
        notify('error', "La mise à jour n'a pas abouti en 5 minutes : voyez le journal C:\\ProgramData\\KyberNow\\update\\<version>\\update.log.")
      }
    }, 3000)
  }

  $effect(() => {
    load()
  })

  const published = $derived(status?.published ? new Date(status.published).toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' }) : null)
</script>

<header>
  <span class="eyebrow">Mises à jour</span>
  <h1>KyberNow {status?.current ?? ''}</h1>
  <p class="muted intro">
    Les nouvelles versions sont publiées sur GitHub ({status?.repo ?? 'MonsieurNours/kybernow'}). La console vérifie toutes les
    6 heures ; l'installation remplace Kyber, la console et l'icône sans toucher à vos réglages ni aux ports, et dure environ une minute.
  </p>
</header>

{#if installing}
  <section class="card state">
    <strong>Installation de la version {installing}…</strong>
    <p class="muted">
      La console s'arrête, remplace ses fichiers et redémarre ({waited} s). Cette page se mettra à jour toute seule. Une session de
      streaming en cours sera coupée.
    </p>
  </section>
{:else if status}
  <section class="card state">
    {#if status.available}
      <div class="row">
        <div>
          <strong>Version {status.latest} disponible</strong>
          {#if published}<span class="muted"> · publiée le {published}</span>{/if}
        </div>
        <button class="primary" onclick={install} disabled={!status.installable}>Installer la version {status.latest}</button>
      </div>
      {#if !status.installable}
        <p class="muted small">Cette version n'a pas de paquet de mise à jour : installez-la avec KyberNow-Setup.exe.</p>
      {/if}
      {#if status.notes}
        <pre class="notes">{status.notes}</pre>
      {/if}
      {#if status.page}<a href={status.page} target="_blank" rel="noreferrer">Voir sur GitHub</a>{/if}
    {:else}
      <div class="row">
        <div>
          <strong>{status.latest ? 'KyberNow est à jour' : 'Aucune version publiée trouvée'}</strong>
          {#if status.checked_seconds_ago !== null}
            <span class="muted"> · vérifié il y a {Math.round(status.checked_seconds_ago / 60)} min</span>
          {/if}
        </div>
        <button onclick={check} disabled={checking}>{checking ? 'Vérification…' : 'Vérifier maintenant'}</button>
      </div>
      {#if status.error}<p class="error small">GitHub : {status.error}</p>{/if}
    {/if}
  </section>
{/if}

<style>
  .intro {
    max-width: 720px;
    font-size: 0.92rem;
  }
  .state {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .notes {
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 0.88rem;
    margin: 0;
    padding: 12px 14px;
    border-radius: 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    max-height: 320px;
    overflow: auto;
  }
  .small {
    font-size: 0.84rem;
    margin: 0;
  }
  .error {
    color: var(--danger, #ff7b72);
  }
</style>
