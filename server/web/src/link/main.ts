import { mount } from 'svelte'
import '../app.css'
import App from './App.svelte'
import { keepConsole } from './journal'

// « Télécharger le journal » : les messages de la page, gardés dès son ouverture.
keepConsole()

// PyroWave : la sonde (WebGPU, décodage d'une image de test) attend qu'une Station le propose
// (`probeAtLoad`, accueil et session de l'hôte).

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
