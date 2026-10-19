<script lang="ts">
  import { app, go } from '../lib/state.svelte'

  let show = $state(false)
  let touched = $state(false)
  let userInput: HTMLInputElement | undefined = $state()

  // L'écran arrive par une transition : on donne le focus une fois inséré.
  $effect(() => {
    const t = setTimeout(() => userInput?.focus(), 150)
    return () => clearTimeout(t)
  })

  const score = $derived.by(() => {
    const p = app.password
    let s = 0
    if (p.length >= 10) s++
    if (p.length >= 14) s++
    if (/[^a-zA-Z]/.test(p)) s++
    if (p.length >= 18 || (/[A-Z]/.test(p) && /[0-9]/.test(p))) s++
    return s
  })
  const labels = ['Trop court : 10 caractères minimum', 'Acceptable', 'Bon mot de passe', 'Très bon mot de passe', 'Excellent mot de passe']
  const strength = $derived(app.password.length < 10 ? labels[0] : labels[score])

  const userError = $derived.by(() => {
    const u = app.user.trim()
    if (!u) return "Choisissez un identifiant."
    if (u.includes(':')) return "L'identifiant ne peut pas contenir « : »."
    if (u.length > 64) return 'Identifiant trop long.'
    return null
  })
  const passError = $derived(app.password.length < 10 ? 'Le mot de passe doit faire au moins 10 caractères.' : null)
  const valid = $derived(!userError && !passError)

  function next() {
    touched = true
    if (valid) go('options')
  }
</script>

<span class="eyebrow kn-enter">Étape 3 sur 6</span>
<h1 class="kn-enter kn-d1">Un seul compte pour tout.</h1>
<p class="lead kn-enter kn-d2">
  Choisissez l'identifiant et le mot de passe que vous saisirez sur vos appareils. Ils protègent l'accès à ce PC :
  personne d'autre sur votre réseau ne pourra s'y connecter.
</p>

<div class="body kn-enter kn-d3">
  <form onsubmit={(e) => (e.preventDefault(), next())}>
    <label>
      Identifiant
      <input bind:this={userInput} bind:value={app.user} autocomplete="username" spellcheck="false" aria-invalid={touched && !!userError} />
      {#if touched && userError}<span class="error">{userError}</span>{/if}
    </label>
    <label>
      Mot de passe
      <div class="pass">
        <input
          type={show ? 'text' : 'password'}
          bind:value={app.password}
          autocomplete="new-password"
          placeholder="10 caractères minimum"
          aria-invalid={touched && !!passError}
        />
        <button type="button" onclick={() => (show = !show)}>{show ? 'Masquer' : 'Afficher'}</button>
      </div>
    </label>
    <div class="meter">
      <div class="bars">
        {#each [0, 1, 2, 3] as i (i)}
          <div style="background: {i < score ? (score <= 1 ? 'var(--warn)' : 'var(--accent)') : '#263040'}"></div>
        {/each}
      </div>
      <span>{strength}</span>
    </div>
    <button type="submit" hidden>Continuer</button>
  </form>

  <aside class="aside kn-enter kn-d4">
    <span class="label-mono">Ce compte vous servira à</span>
    <div class="use">
      <span class="ico"><svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><rect x="2" y="3" width="14" height="9" rx="1.5" fill="none" stroke="#E9EEF5" stroke-width="1.5" /><path d="M6 15h6" stroke="#E9EEF5" stroke-width="1.5" stroke-linecap="round" /></svg></span>
      <div>
        <strong>Vous connecter au stream</strong>
        <p>Depuis l'app Mac, l'Apple TV ou un navigateur. Vos appareils peuvent le mémoriser.</p>
      </div>
    </div>
    <div class="use">
      <span class="ico"><svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true"><circle cx="9" cy="9" r="2.6" fill="none" stroke="#E9EEF5" stroke-width="1.5" /><path d="M9 1.8v2.4M9 13.8v2.4M1.8 9h2.4M13.8 9h2.4M3.9 3.9l1.7 1.7M12.4 12.4l1.7 1.7M3.9 14.1l1.7-1.7M12.4 5.6l1.7-1.7" stroke="#E9EEF5" stroke-width="1.5" stroke-linecap="round" /></svg></span>
      <div>
        <strong>Ouvrir la console d'administration</strong>
        <p>Pour changer la qualité d'image, les manettes ou l'écran virtuel. Vous pourrez ensuite ajouter d'autres comptes.</p>
      </div>
    </div>
    <div class="lock">
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="7" width="10" height="7" rx="1.5" fill="none" stroke="#9AA7B8" stroke-width="1.4" /><path d="M5.5 7V5a2.5 2.5 0 015 0v2" fill="none" stroke="#9AA7B8" stroke-width="1.4" /></svg>
      <span>Le mot de passe n'est jamais enregistré en clair : seule une empreinte est conservée sur ce PC.</span>
    </div>
  </aside>
</div>

<div class="footer">
  <button class="btn" onclick={() => go('checks')}>Retour</button>
  <div class="spacer"></div>
  {#if touched && passError}<span class="error">{passError}</span>{/if}
  <button class="btn primary" onclick={next}>
    Continuer
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
</div>

<style>
  .body {
    margin-top: 28px;
    display: flex;
    gap: 28px;
  }

  form {
    flex: 1;
    max-width: 380px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 14px;
    font-weight: 500;
  }

  input {
    height: 46px;
    padding: 0 14px;
    border-radius: 10px;
    border: 1px solid var(--line-2);
    background: #121922;
    color: var(--text);
    font-size: 15px;
    font-weight: 400;
    width: 100%;
  }

  input[aria-invalid='true'] {
    border-color: var(--danger);
  }

  .pass {
    position: relative;
    display: flex;
  }

  .pass input {
    padding-right: 92px;
  }

  .pass button {
    position: absolute;
    right: 6px;
    top: 6px;
    height: 34px;
    padding: 0 12px;
    border-radius: 8px;
    border: none;
    background: #1e2733;
    color: var(--text-2);
    font-size: 13px;
  }

  .meter {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--muted);
  }

  .bars {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
  }

  .bars div {
    height: 6px;
    border-radius: 3px;
    transition: background-color 0.35s;
  }

  .error {
    font-size: 12px;
    font-weight: 400;
    color: var(--danger);
  }

  aside {
    flex: 1;
    padding: 20px;
    gap: 16px;
  }

  .use {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .use strong {
    font-size: 14px;
  }

  .use p {
    margin: 2px 0 0;
    color: var(--muted);
  }

  .ico {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border-radius: 9px;
    background: var(--surface-2);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .lock {
    margin-top: auto;
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 12px 14px;
    border-radius: 10px;
    background: #131b24;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }

  .lock svg {
    flex-shrink: 0;
    margin-top: 2px;
  }
</style>
