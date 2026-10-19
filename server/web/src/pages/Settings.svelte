<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Field, type FieldValue, type Group } from '../lib/api'
  import { markRestartPending, notify, errorText } from '../lib/store.svelte'
  import InfoTip from '../lib/InfoTip.svelte'

  // Icône de chaque groupe (tracés 20 × 20, en trait).
  const groupIcons: Record<string, string> = {
    video: 'M3 5.5h14v9H3zM7.5 17h5M8.5 8l3.5 2-3.5 2z',
    audio: 'M3.5 8h3l4-3.5v11l-4-3.5h-3zM13.5 7.5a3.5 3.5 0 010 5M15.5 5.5a6.5 6.5 0 010 9',
    network: 'M10 3.5a6.5 6.5 0 100 13 6.5 6.5 0 000-13zM3.5 10h13M10 3.5c2 2 2 11 0 13M10 3.5c-2 2-2 11 0 13',
    input: 'M3 6.5h14v8H3zM6 9.5h1M9 9.5h1M12 9.5h1M6.5 12h7',
    auth: 'M5 9V7a5 5 0 0110 0v2M4 9h12v7.5H4zM10 12v2',
    logs: 'M5 3h7l3 3v11H5zM7.5 9h5M7.5 12h5M7.5 6h2.5',
  }

  let groups = $state<Group[]>([])
  let fields = $state<Field[]>([])
  let saved = $state<Record<string, FieldValue>>({})
  // Modifications non enregistrées : valeur, ou null pour revenir au défaut de Kyber.
  let draft = $state<Record<string, unknown>>({})
  let busy = $state(false)
  let error = $state('')

  const dirty = $derived(Object.keys(draft).length > 0)

  function setValues(values: FieldValue[]) {
    saved = Object.fromEntries(values.map((v) => [v.id, v]))
  }

  onMount(async () => {
    try {
      const [schema, config] = await Promise.all([api.schema(), api.config()])
      groups = schema.groups
      fields = schema.fields
      setValues(config.values)
    } catch (e) {
      error = errorText(e)
    }
  })

  function current(f: Field): unknown {
    if (f.id in draft) return draft[f.id] === null ? f.default : draft[f.id]
    return saved[f.id]?.value ?? f.default
  }

  function isExplicit(f: Field): boolean {
    if (f.id in draft) return draft[f.id] !== null
    return saved[f.id]?.explicit ?? false
  }

  function change(f: Field, value: unknown) {
    const original = saved[f.id]
    if (original?.explicit && original.value === value) {
      delete draft[f.id]
    } else if (!original?.explicit && value === f.default) {
      delete draft[f.id]
    } else {
      draft[f.id] = value
    }
  }

  function reset(f: Field) {
    if (saved[f.id]?.explicit) draft[f.id] = null
    else delete draft[f.id]
  }

  /** Valeur lisible (libellé du choix, Activé/Désactivé, Mbit/s). */
  function readable(f: Field, v: unknown): string {
    if (f.kind.type === 'enum') return f.kind.options.find((o) => o.value === v)?.label ?? String(v)
    if (f.kind.type === 'bool') return v ? 'Activé' : 'Désactivé'
    return f.unit === 'bps' ? `${Number(v) / 1_000_000} Mbit/s` : String(v)
  }

  function choiceHint(f: Field, v: unknown): string {
    return f.kind.type === 'enum' ? (f.kind.options.find((o) => o.value === v)?.hint ?? '') : ''
  }

  // Les débits sont stockés en bit/s mais affichés en Mbit/s.
  const toDisplay = (f: Field, v: unknown) => (f.unit === 'bps' ? Number(v) / 1_000_000 : v)
  const fromDisplay = (f: Field, v: number) => (f.unit === 'bps' ? Math.round(v * 1_000_000) : v)

  async function save() {
    busy = true
    error = ''
    try {
      const res = await api.updateConfig(draft)
      setValues(res.values)
      draft = {}
      markRestartPending()
      notify('ok', 'Configuration enregistrée.')
    } catch (e) {
      error = errorText(e)
    } finally {
      busy = false
    }
  }
</script>

<header class="top">
  <div>
    <span class="eyebrow">Configuration du serveur</span>
    <h1>Réglages</h1>
    <p class="muted intro">
      Chaque réglage est expliqué : survolez ou touchez
      <span class="i" aria-hidden="true">ⓘ</span>
      pour savoir ce qu'il fait et quand le changer. Les modifications s'appliquent au prochain redémarrage de Kyber, et
      une sauvegarde de la configuration est faite à chaque enregistrement.
    </p>
  </div>
  <div class="actions">
    <button disabled={!dirty || busy} onclick={() => (draft = {})}>Annuler</button>
    <button class="primary" disabled={!dirty || busy} onclick={save}>
      {dirty ? `Enregistrer (${Object.keys(draft).length})` : 'Enregistrer'}
    </button>
  </div>
</header>

{#if error}
  <div class="notice danger">{error}</div>
{/if}

{#each groups as group (group.id)}
  <section class="card group">
    <div class="group-head">
      <span class="group-icon">
        <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true"><path d={groupIcons[group.id] ?? ''} fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </span>
      <div>
        <h2>{group.label}</h2>
        <p class="muted">{group.help}</p>
      </div>
    </div>

    <div class="fields">
      {#each fields.filter((f) => f.group === group.id) as f (f.id)}
        {@const value = current(f)}
        <div class="setting" class:changed={f.id in draft}>
          <div class="text">
            <div class="label">
              <span class="name">{f.label}</span>
              <InfoTip label={f.label}>
                <strong class="tip-title">{f.label}</strong>
                <span>{f.details}</span>
                {#if f.kind.type === 'enum'}
                  <span class="tip-choices">
                    {#each f.kind.options as o (o.value)}
                      <span class="tip-choice" class:on={o.value === value}>
                        <strong>{o.label}</strong>
                        <span>{o.hint}</span>
                      </span>
                    {/each}
                  </span>
                {/if}
                <span class="tip-advice"><strong>Conseil</strong> {f.advice}</span>
                {#if f.caution}<span class="tip-caution">{f.caution}</span>{/if}
                <span class="tip-default">Valeur par défaut de Kyber : {readable(f, f.default)}</span>
              </InfoTip>
              {#if f.id in draft}
                <span class="chip changed">modifié, à enregistrer</span>
              {:else if isExplicit(f)}
                <span class="chip custom">personnalisé</span>
              {/if}
            </div>
            <p class="help">{f.help}</p>
            {#if isExplicit(f) && value !== f.default}
              <button class="link" onclick={() => reset(f)}>Revenir à la valeur par défaut ({readable(f, f.default)})</button>
            {/if}
          </div>
          <div class="control">
            {#if f.kind.type === 'bool'}
              <button
                class="switch"
                role="switch"
                aria-checked={Boolean(value)}
                aria-label={f.label}
                class:on={Boolean(value)}
                onclick={() => change(f, !value)}
              >
                <span></span>
              </button>
              <span class="state">{value ? 'Activé' : 'Désactivé'}</span>
            {:else if f.kind.type === 'enum'}
              <div class="select">
                <select value={String(value)} aria-label={f.label} onchange={(e) => change(f, e.currentTarget.value)}>
                  {#each f.kind.options as option (option.value)}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
                <span class="hint">{choiceHint(f, value)}</span>
              </div>
            {:else}
              <div class="number">
                <input
                  type="number"
                  aria-label={f.label}
                  step={f.unit === 'bps' ? 0.5 : 1}
                  min={Number(toDisplay(f, f.kind.min))}
                  max={Number(toDisplay(f, f.kind.max))}
                  value={toDisplay(f, value)}
                  onchange={(e) => {
                    const n = e.currentTarget.valueAsNumber
                    if (!Number.isNaN(n)) change(f, fromDisplay(f, n))
                  }}
                />
                {#if f.unit === 'bps'}<span class="muted">Mbit/s</span>{/if}
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </section>
{/each}

<style>
  .top {
    display: flex;
    align-items: flex-end;
    gap: 20px;
  }

  .intro {
    max-width: 680px;
    font-size: 0.92rem;
  }

  .i {
    color: var(--accent);
  }

  .actions {
    margin-left: auto;
    display: flex;
    gap: 10px;
    flex-shrink: 0;
  }

  .group {
    padding: 20px 22px 8px;
  }

  .group-head {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    padding-bottom: 14px;
  }

  .group-head p {
    font-size: 0.88rem;
    max-width: 640px;
  }

  .group-icon {
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .fields {
    display: flex;
    flex-direction: column;
  }

  .setting {
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 20px;
    align-items: start;
    padding: 14px 0;
    border-top: 1px solid var(--border);
    transition: box-shadow 0.2s, padding 0.2s;
  }

  .setting.changed {
    box-shadow: inset 3px 0 0 var(--accent);
    padding-left: 14px;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .name {
    font-weight: 600;
    font-size: 0.95rem;
  }

  .chip {
    font-family: var(--mono);
    font-size: 0.72rem;
    padding: 1px 8px;
    border-radius: 999px;
  }

  .chip.custom {
    background: var(--surface-2);
    color: var(--muted);
  }

  .chip.changed {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .help {
    margin-top: 3px;
    font-size: 0.86rem;
    color: var(--muted);
  }

  .text .link {
    margin-top: 6px;
  }

  .control {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: flex-end;
    padding-top: 2px;
  }

  .select {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .select select {
    width: 100%;
  }

  .hint {
    font-size: 0.78rem;
    line-height: 1.45;
    color: var(--faint);
  }

  .number {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .number input {
    width: 100%;
  }

  .switch {
    width: 48px;
    height: 28px;
    min-height: 0;
    padding: 3px;
    border: none;
    border-radius: 14px;
    background: var(--border-2);
    display: flex;
    flex-shrink: 0;
    transition: background-color 0.25s;
  }

  .switch.on {
    background: var(--accent);
  }

  .switch:hover:not(:disabled) {
    background: #3a4659;
  }

  .switch.on:hover:not(:disabled) {
    background: var(--accent);
    filter: brightness(1.07);
  }

  .switch span {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #f4f7fa;
    transition: transform 0.25s cubic-bezier(0.2, 0.7, 0.2, 1);
  }

  .switch.on span {
    transform: translateX(20px);
  }

  .state {
    width: 76px;
    font-size: 0.86rem;
    color: var(--muted);
  }

  /* Contenu des infobulles */
  .tip-title {
    color: var(--text);
    font-size: 0.92rem;
  }

  .tip-choices {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .tip-choice {
    display: flex;
    flex-direction: column;
    padding: 6px 10px;
    border-radius: 8px;
    background: var(--surface-2);
    border: 1px solid transparent;
  }

  .tip-choice.on {
    border-color: var(--accent);
  }

  .tip-choice strong {
    color: var(--text);
    font-size: 0.82rem;
  }

  .tip-advice strong {
    color: var(--accent);
    margin-right: 4px;
  }

  .tip-caution {
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--warn-bg);
    border: 1px solid var(--warn-line);
    color: var(--warn-text);
  }

  .tip-default {
    font-family: var(--mono);
    font-size: 0.76rem;
    color: var(--faint);
  }

  @media (max-width: 860px) {
    .top {
      flex-wrap: wrap;
    }

    .actions {
      margin-left: 0;
    }

    .setting {
      grid-template-columns: 1fr;
    }

    .control {
      justify-content: flex-start;
    }
  }
</style>
