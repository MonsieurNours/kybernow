import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { isNative } from './lib/backend'
import { isMac } from './mac/bridge'
import MacApp from './mac/MacApp.svelte'
import { i18n, t } from './lib/i18n/index.svelte'

document.documentElement.lang = i18n.lang
document.title = t('documentTitle')

// Dans l'installateur, pas de menu contextuel ni de zoom du navigateur.
if (isNative) {
  window.addEventListener('contextmenu', (e) => e.preventDefault())
  window.addEventListener('wheel', (e) => e.ctrlKey && e.preventDefault(), { passive: false })
}

// Sur Mac (ou `?mac` dans un navigateur) : les fenêtres de la Station pour Mac.
export default mount(isMac ? MacApp : App, { target: document.getElementById('app')! })
