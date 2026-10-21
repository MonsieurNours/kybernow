import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { isNative } from './lib/backend'
import { i18n, t } from './lib/i18n/index.svelte'

document.documentElement.lang = i18n.lang
document.title = t('documentTitle')

// Dans l'installateur, pas de menu contextuel ni de zoom du navigateur.
if (isNative) {
  window.addEventListener('contextmenu', (e) => e.preventDefault())
  window.addEventListener('wheel', (e) => e.ctrlKey && e.preventDefault(), { passive: false })
}

export default mount(App, { target: document.getElementById('app')! })
