<!--
  La semaine des créneaux de tous les amis (page « Prêt » d'Admin) : une colonne
  par jour, aujourd'hui en avant et la ligne de maintenant, chaque créneau à la
  couleur de son ami. À l'heure de ce navigateur.
-->
<script lang="ts">
  import type { LendFriend } from './api'
  import { avatarColor, dayName, slotHour, slotMinutes, weekday } from './lend'
  import { t } from './i18n/index.svelte'

  let { friends }: { friends: LendFriend[] } = $props()

  let now = $state(new Date())
  $effect(() => {
    const timer = setInterval(() => (now = new Date()), 60000)
    return () => clearInterval(timer)
  })

  const H = 200
  const DAYS = [1, 2, 3, 4, 5, 6, 7]
  const slots = $derived(friends.filter((f) => f.enabled).flatMap((f) => f.slots.map((s) => ({ friend: f, ...s }))))
  /** Les heures montrées : de 8 h à minuit, plus tôt si un créneau commence avant. */
  const from = $derived(Math.min(8 * 60, ...slots.map((s) => Math.floor(slotMinutes(s.start) / 240) * 240)))
  const span = $derived(24 * 60 - from)
  const y = (minutes: number) => ((minutes - from) / span) * H
  const today = $derived(weekday(now))
  const nowMinutes = $derived(now.getHours() * 60 + now.getMinutes())
  const marks = $derived([0, 4, 8, 12, 16, 20].map((h) => h * 60).filter((m) => m >= from))
</script>

<div class="surface scroll">
  <div class="week" style="--h: {H}px">
    <div></div>
    {#each DAYS as d (d)}
      <div class="dh" class:today={d === today}>{dayName(d, 'short')}</div>
    {/each}
    <div class="hours" aria-hidden="true">
      {#each marks as m (m)}<span style="top: {y(m)}px">{slotHour(`${m / 60}:00`)}</span>{/each}
    </div>
    {#each DAYS as d (d)}
      <div class="col" class:today={d === today}>
        {#each slots.filter((s) => s.days.includes(d)) as s, i (i)}
          <div class="slot" style="top: {y(slotMinutes(s.start))}px; height: {Math.max(20, y(slotMinutes(s.end)) - y(slotMinutes(s.start)))}px; background: {avatarColor(s.friend.id)}" title="{s.friend.name}, {slotHour(s.start)} – {slotHour(s.end)}">
            {s.friend.name}<small>{slotHour(s.start)} – {slotHour(s.end)}</small>
          </div>
        {/each}
        {#if d === today && nowMinutes >= from}<div class="now" style="top: {y(nowMinutes)}px" aria-label={t('lend.admin.week.now')}></div>{/if}
      </div>
    {/each}
  </div>
  {#if !slots.length}<p class="empty">{t('lend.admin.week.empty')}</p>{/if}
</div>

<style>
  .surface {
    position: relative;
    border-radius: 14px;
    background: var(--card);
    box-shadow: inset 0 0 0 1px var(--line-2);
  }

  .scroll {
    overflow-x: auto;
  }

  .week {
    display: grid;
    grid-template-columns: 54px repeat(7, minmax(0, 1fr));
    padding: 14px 16px 16px;
    min-width: 620px;
  }

  .dh {
    text-align: center;
    padding-bottom: 8px;
    font-size: 12px;
    color: var(--faint);
    text-transform: capitalize;
  }

  .dh.today {
    color: var(--signal-light);
    font-weight: 700;
  }

  .hours {
    position: relative;
    height: var(--h);
  }

  .hours span {
    position: absolute;
    right: 8px;
    transform: translateY(-50%);
    font-family: var(--mono);
    font-size: 10px;
    color: var(--ghost);
    white-space: nowrap;
  }

  .col {
    position: relative;
    height: var(--h);
    border-left: 1px solid var(--line);
    background: repeating-linear-gradient(0deg, transparent 0 calc(var(--h) / 6 - 1px), var(--line) calc(var(--h) / 6 - 1px) calc(var(--h) / 6));
  }

  .col.today {
    background-color: rgb(109 91 255 / 0.05);
  }

  .slot {
    position: absolute;
    left: 4px;
    right: 4px;
    border-radius: 7px;
    padding: 4px 7px;
    font-size: 11.5px;
    font-weight: 600;
    line-height: 1.25;
    color: var(--ink-0);
    overflow: hidden;
  }

  .slot small {
    display: block;
    font-weight: 500;
    opacity: 0.75;
  }

  .now {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--bad);
    box-shadow: 0 0 6px var(--bad);
  }

  .now::before {
    content: '';
    position: absolute;
    left: -4px;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--bad);
  }

  .empty {
    position: absolute;
    left: 50%;
    top: 55%;
    transform: translate(-50%, -50%);
    padding: 8px 14px;
    border-radius: 10px;
    background: rgb(10 15 20 / 0.85);
    color: var(--faint);
    font-size: 13px;
    white-space: nowrap;
  }
</style>
