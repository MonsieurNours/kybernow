import { mount } from 'svelte'
import './app.css'
import './lib/nomenu'
import Fond from './Fond.svelte'

export default mount(Fond, { target: document.getElementById('app')! })
