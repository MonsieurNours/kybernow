import { mount } from 'svelte'
import './app.css'
import Connexion from './Connexion.svelte'

export default mount(Connexion, { target: document.getElementById('app')! })
