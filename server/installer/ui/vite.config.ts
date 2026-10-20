import { readFileSync } from 'node:fs'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Version affichée : celle de l'installateur (Cargo.toml), une seule source.
const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
const version = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1] ?? '0.0.0'

export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
})
