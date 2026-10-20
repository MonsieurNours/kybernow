<!-- Étape 2 · Le compte unique : pour se connecter depuis les appareils et ouvrir la console. -->
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
  const labels = ['10 caractères au moins', 'Acceptable', 'Bon mot de passe', 'Très bon mot de passe', 'Excellent mot de passe']
  const strength = $derived(app.password.length < 10 ? labels[0] : labels[score])

  const userError = $derived.by(() => {
    const u = app.user.trim()
    if (!u) return 'Choisissez un identifiant.'
    if (u.includes(':')) return "L'identifiant ne peut pas contenir « : »."
    if (u.length > 64) return 'Identifiant trop long.'
    return null
  })
  const passError = $derived(app.password.length < 10 ? 'Le mot de passe doit faire au moins 10 caractères.' : null)
  const valid = $derived(!userError && !passError)

  function next(event?: SubmitEvent) {
    event?.preventDefault()
    touched = true
    if (valid) go('options')
  }
</script>

<form class="account" onsubmit={next} id="account">
  <div class="left">
    <div class="head">
      <span class="lbl signal kn-enter">Votre compte</span>
      <h1 class="title kn-enter kn-d1">Un seul compte pour tout.</h1>
      <p class="lead kn-enter kn-d2">Personne d'autre sur votre réseau ne pourra ouvrir cet ordinateur.</p>
    </div>
    <div class="field kn-enter kn-d2">
      <label for="user">Identifiant</label>
      <input id="user" bind:this={userInput} bind:value={app.user} autocomplete="username" spellcheck="false" aria-invalid={touched && !!userError} />
      {#if touched && userError}<span class="error">{userError}</span>{/if}
    </div>
    <div class="field kn-enter kn-d3">
      <label for="pass">Mot de passe</label>
      <div class="pass">
        <input id="pass" type={show ? 'text' : 'password'} bind:value={app.password} autocomplete="new-password" aria-invalid={touched && !!passError} />
        <button type="button" class="text-button" onclick={() => (show = !show)}>{show ? 'Masquer' : 'Afficher'}</button>
      </div>
      <div class="meter" aria-hidden="true">{#each [1, 2, 3, 4] as i}<span class:on={app.password.length >= 10 && score >= i}></span>{/each}</div>
      <span class="hint">{strength}. Il n'est enregistré que sous forme d'empreinte.</span>
    </div>
  </div>

  <div class="panel uses kn-enter kn-d2">
    <span class="lbl">Il vous servira à</span>
    <div><strong>Vous connecter depuis vos appareils</strong><span>Ils peuvent le mémoriser.</span></div>
    <div><strong>Ouvrir la console</strong><span>Pour l'écran virtuel, le son, les comptes. Vous pourrez y en ajouter d'autres.</span></div>
  </div>
</form>

<div class="footer">
  <button type="button" class="ghost" onclick={() => go('welcome')}>Retour</button>
  <div class="spacer"></div>
  <button type="submit" form="account" class="primary">Continuer</button>
</div>

<style>
  .account {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 44px;
    align-items: start;
    padding-top: 44px;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 22px;
    max-width: 460px;
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .pass {
    position: relative;
    display: flex;
  }

  .pass input {
    flex: 1;
    padding-right: 84px;
  }

  .pass .text-button {
    position: absolute;
    right: 14px;
    top: 13px;
    font-size: 12.5px;
  }

  .meter {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
  }

  .meter span {
    height: 3px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.08);
    transition: background-color 0.3s;
  }

  .meter span.on {
    background: var(--signal);
    box-shadow: 0 0 6px var(--signal);
  }

  .uses {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px;
  }

  .uses div {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .uses strong {
    font-size: 14px;
  }

  .uses div span {
    font-size: 13px;
    line-height: 1.45;
    color: var(--faint);
  }
</style>
