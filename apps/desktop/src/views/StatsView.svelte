<script lang="ts">
  // Reading statistics for a year: time read, streaks, the yearly goal, a
  // calendar of reading days, time per month, finished and most-read books.
  import { Tween } from 'svelte/motion'
  import { cubicOut } from 'svelte/easing'
  import BookCover from '../components/BookCover.svelte'
  import Icon from '../components/Icon.svelte'
  import { api, errorMessage } from '../lib/api'
  import { prompt } from '../lib/dialog.svelte'
  import { formatCount, formatDate, plural } from '../lib/format'
  import { library } from '../lib/library.svelte'
  import { ms, rise } from '../lib/motion'
  import { localDay } from '../lib/readingLog'
  import {
    LEVELS,
    byMonth,
    calendar,
    dayToDate,
    formatReadingTime,
    formatShortTime,
    level,
    niceMax,
    streaks,
    yearBounds,
  } from '../lib/readingStats'
  import type { BookSummary, ReadingStats } from '../lib/types'
  import { ui } from '../lib/uiSettings.svelte'

  interface Props {
    onopen: (book: BookSummary) => void
  }
  let { onopen }: Props = $props()

  const today = localDay()
  const thisYear = Number(today.slice(0, 4))
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
  const WEEKDAYS = ['Mon', '', 'Wed', '', 'Fri', '', '']

  let year = $state(thisYear)
  let stats = $state<ReadingStats | null>(null)
  let error = $state<string | null>(null)
  let showTable = $state(false)

  async function load() {
    const y = year
    const b = yearBounds(y)
    try {
      const s = await api.readingStats(b.from, b.to, b.start, b.end)
      if (y === year) stats = s
    } catch (e) {
      error = errorMessage(e)
    }
  }

  $effect(() => {
    void library.revision
    void year
    load()
  })

  const total = $derived(stats?.days.reduce((n, d) => n + d.seconds, 0) ?? 0)
  const pages = $derived(stats?.days.reduce((n, d) => n + d.pages, 0) ?? 0)
  const readingDays = $derived(stats?.days.filter((d) => d.seconds > 0).length ?? 0)
  const streak = $derived(
    streaks(
      (stats?.days ?? []).filter((d) => d.seconds > 0).map((d) => d.day),
      today,
    ),
  )
  const weeks = $derived(calendar(year, stats?.days ?? [], today))
  const months = $derived(byMonth(stats?.days ?? []))
  const monthMax = $derived(niceMax(Math.max(...months.map((m) => m.seconds))))
  const busiest = $derived(months.reduce((a, b) => (b.seconds > a.seconds ? b : a), months[0]))
  const finishedCount = $derived(stats?.finished.length ?? 0)
  const goal = $derived(ui.value.readingGoal)
  const isCurrentYear = $derived(year === thisYear)
  // Where the year should be by now, to say whether the goal is on track.
  const yearElapsed = $derived.by(() => {
    if (!isCurrentYear) return 1
    const b = yearBounds(year)
    return Math.min(1, (Date.now() / 1000 - b.start) / (b.end - b.start))
  })
  const onTrack = $derived(goal > 0 && finishedCount >= Math.floor(goal * yearElapsed))

  const hero = new Tween(0, { duration: ms(900), easing: cubicOut })
  const goalFill = new Tween(0, { duration: ms(900), easing: cubicOut })
  $effect(() => {
    hero.target = total
    goalFill.target = goal > 0 ? Math.min(1, finishedCount / goal) : 0
  })

  const topBookMax = $derived(Math.max(1, ...(stats?.top_books.map((b) => b.seconds) ?? [1])))
  const topAuthorMax = $derived(Math.max(1, ...(stats?.top_authors.map((a) => a.seconds) ?? [1])))

  // Month labels above the calendar: at the first week that starts in a month.
  const monthLabels = $derived(
    weeks
      .map((w, i) => ({ i, cell: w.find((c) => !c.outside && c.day.endsWith('-01')) }))
      .filter((x) => x.cell)
      .map((x) => ({ col: x.i, label: MONTHS[Number(x.cell!.day.slice(5, 7)) - 1] })),
  )

  // One tooltip for the calendar and the month chart.
  let tip = $state<{ x: number; y: number; value: string; label: string } | null>(null)
  function showTip(e: PointerEvent | FocusEvent, value: string, label: string) {
    const target = e.currentTarget as Element
    const r = target.getBoundingClientRect()
    tip = { x: r.left + r.width / 2, y: r.top, value, label }
  }
  const hideTip = () => (tip = null)

  const longDate = (day: string) =>
    dayToDate(day).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' })

  async function editGoal() {
    const value = await prompt({
      title: goal ? 'Change your reading goal' : 'Set a reading goal',
      label: `Books to finish in ${thisYear}`,
      value: goal ? String(goal) : '',
      confirmLabel: 'Save goal',
      maxLength: 4,
    })
    if (value === null) return
    const n = Math.round(Number(value))
    if (Number.isFinite(n) && n >= 0) ui.update({ readingGoal: Math.min(1000, n) })
  }

  // Chart geometry, in pixels of the measured width.
  const CHART_H = 160
  const AXIS_W = 40
  let chartWidth = $state(640)
  const SLOT = $derived((chartWidth - AXIS_W) / 12)
  const COL_W = $derived(Math.min(24, SLOT * 0.55))
</script>

<div class="screen">
  <header in:rise={{ y: 6 }}>
    <div>
      <span class="subtitle">My reading</span>
      <h1>Reading stats</h1>
    </div>
    <div class="year" role="group" aria-label="Year">
      <button class="ghost" onclick={() => year--} aria-label="Previous year"><Icon name="chevron-left" size={16} /></button>
      <span aria-live="polite">{year}</span>
      <button class="ghost" onclick={() => year++} disabled={year >= thisYear} aria-label="Next year"><Icon name="chevron-right" size={16} /></button>
    </div>
  </header>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="top">
    <section class="card hero-card" in:rise={{ delay: 40 }}>
      <span class="label">Time read in {year}</span>
      <span class="hero" aria-label={formatReadingTime(total)}>{formatReadingTime(hero.current)}</span>
      <span class="sub">
        {formatCount(stats?.books_read ?? 0)}
        {plural(stats?.books_read ?? 0, 'book')} · {formatCount(pages)}
        {plural(pages, 'page')} turned
      </span>
    </section>

    <section class="card goal-card" in:rise={{ delay: 90 }}>
      <div class="goal-head">
        <span class="label">Books finished</span>
        {#if isCurrentYear}
          <button class="ghost small" onclick={editGoal}>{goal ? 'Change goal' : 'Set a goal'}</button>
        {/if}
      </div>
      <span class="goal-value">
        {formatCount(finishedCount)}{#if goal}<span class="of"> of {formatCount(goal)}</span>{/if}
      </span>
      {#if goal}
        <div
          class="meter"
          role="meter"
          aria-label="Reading goal"
          aria-valuemin={0}
          aria-valuemax={goal}
          aria-valuenow={finishedCount}
        >
          <div class="meter-fill" style:width="{goalFill.current * 100}%"></div>
        </div>
        <span class="sub">
          {#if finishedCount >= goal}
            <Icon name="sparkle" size={12} /> Goal reached
          {:else if isCurrentYear}
            {goal - finishedCount} to go · {onTrack ? 'on track' : 'a little behind'}
          {:else}
            {goal - finishedCount} short of the goal
          {/if}
        </span>
      {:else}
        <span class="sub">A yearly goal shows how you are doing.</span>
      {/if}
    </section>
  </div>

  <div class="tiles">
    {#each [{ label: 'Current streak', value: isCurrentYear ? streak.current : 0, unit: 'day', icon: 'sparkle' }, { label: 'Longest streak', value: streak.longest, unit: 'day', icon: 'check' }, { label: 'Reading days', value: readingDays, unit: 'day', icon: 'clock' }] as const as t, i (t.label)}
      <div class="tile card" in:rise={{ delay: 140 + i * 50 }}>
        <span class="label">{t.label}</span>
        <span class="tile-value">{formatCount(t.value)} <span class="unit">{plural(t.value, t.unit)}</span></span>
      </div>
    {/each}
    <div class="tile card" in:rise={{ delay: 290 }}>
      <span class="label">Per reading day</span>
      <span class="tile-value">{readingDays ? formatReadingTime(total / readingDays) : '—'}</span>
    </div>
  </div>

  <section class="card" in:rise={{ delay: 200 }}>
    <div class="card-head">
      <h2>Reading days</h2>
      <div class="legend" aria-hidden="true">
        Less
        {#each [0, 1, 2, 3, 4] as l (l)}<span class="cell l{l}"></span>{/each}
        More
      </div>
    </div>
    <div class="calendar-scroll">
      <div class="calendar" style:--weeks={weeks.length} role="img" aria-label="Reading days in {year}: {readingDays} days, {formatReadingTime(total)}">
        {#each monthLabels as m (m.col)}
          <span class="month" aria-hidden="true" style:grid-column="{m.col + 2} / span 4">{m.label}</span>
        {/each}
        {#each WEEKDAYS as d, i (i)}
          <span class="weekday" aria-hidden="true" style:grid-row={i + 2}>{d}</span>
        {/each}
        {#each weeks as week, w (w)}
          {#each week as c, d (c.day)}
            {#if c.outside || c.future}
              <span class="cell blank" style:grid-column={w + 2} style:grid-row={d + 2}></span>
            {:else}
              <span
                class="cell l{level(c.seconds)}"
                class:today={c.day === today}
                style:grid-column={w + 2}
                style:grid-row={d + 2}
                role="presentation"
                onpointerenter={(e) => showTip(e, c.seconds ? formatReadingTime(c.seconds) : 'No reading', `${longDate(c.day)}${c.pages ? ` · ${c.pages} ${plural(c.pages, 'page')}` : ''}`)}
                onpointerleave={hideTip}
              ></span>
            {/if}
          {/each}
        {/each}
      </div>
    </div>
    <p class="note">Shades: under {LEVELS[1]} min, under {LEVELS[2]}, under {LEVELS[3]}, an hour or more.</p>
  </section>

  <section class="card" in:rise={{ delay: 240 }}>
    <div class="card-head">
      <h2>Time per month</h2>
      <button class="ghost small" onclick={() => (showTable = !showTable)} aria-pressed={showTable}>
        {showTable ? 'Show chart' : 'Show table'}
      </button>
    </div>
    {#if showTable}
      <table>
        <thead><tr><th>Month</th><th>Time</th><th>Pages</th><th>Reading days</th></tr></thead>
        <tbody>
          {#each months as m (m.month)}
            <tr><td>{MONTHS[m.month]}</td><td>{formatReadingTime(m.seconds)}</td><td>{formatCount(m.pages)}</td><td>{m.days}</td></tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <div class="chart-box" bind:clientWidth={chartWidth}>
      <svg class="chart" width={chartWidth} height={CHART_H + 30} role="img" aria-label="Reading time per month in {year}">
        {#each [0, 0.5, 1] as f (f)}
          {@const y = CHART_H - f * (CHART_H - 16)}
          <line class="grid-line" x1={AXIS_W} x2={chartWidth} y1={y} y2={y} />
          <text class="axis" x="32" y={y + 4} text-anchor="end">{formatShortTime(monthMax * f)}</text>
        {/each}
        {#each months as m (m.month)}
          {@const h = (m.seconds / monthMax) * (CHART_H - 16)}
          {@const x = AXIS_W + m.month * SLOT + (SLOT - COL_W) / 2}
          <g
            class="col"
            role="presentation"
            onpointerenter={(e) => showTip(e, formatReadingTime(m.seconds), `${MONTHS[m.month]} ${year} · ${m.days} ${plural(m.days, 'day')}`)}
            onpointerleave={hideTip}
          >
            <rect class="hit" x={AXIS_W + m.month * SLOT} y="0" width={SLOT} height={CHART_H} />
            {#if h > 0}
              <path
                class="bar"
                style:--d="{m.month * 40}ms"
                d="M{x},{CHART_H} V{CHART_H - h + Math.min(4, h)} q0,-{Math.min(4, h)} {Math.min(4, h)},-{Math.min(4, h)} H{x + COL_W - Math.min(4, h)} q{Math.min(4, h)},0 {Math.min(4, h)},{Math.min(4, h)} V{CHART_H} Z"
              />
            {/if}
            {#if m === busiest && m.seconds > 0}
              <text class="value" x={x + COL_W / 2} y={CHART_H - h - 6} text-anchor="middle">{formatShortTime(m.seconds)}</text>
            {/if}
            <text class="axis" x={x + COL_W / 2} y={CHART_H + 18} text-anchor="middle">{MONTHS[m.month]}</text>
          </g>
        {/each}
        <line class="baseline" x1={AXIS_W} x2={chartWidth} y1={CHART_H} y2={CHART_H} />
      </svg>
      </div>
    {/if}
  </section>

  {#if stats && stats.finished.length}
    <section class="card" in:rise={{ delay: 280 }}>
      <div class="card-head"><h2>Finished in {year}</h2><span class="count">{finishedCount}</span></div>
      <div class="shelf">
        {#each stats.finished as f, i (f.book.id)}
          <button class="finished" onclick={() => onopen(f.book)} title="{f.book.title} · {formatDate(f.finished_at)}" in:rise={{ delay: Math.min(i, 12) * 30 }}>
            <span class="cover"><BookCover title={f.book.title} author={f.book.authors[0]} coverKey={f.book.cover_key} /></span>
            <span class="when">{formatDate(f.finished_at)}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if stats && (stats.top_books.length || stats.top_authors.length)}
    <div class="two">
      <section class="card" in:rise={{ delay: 320 }}>
        <h2>Most read</h2>
        <ol class="ranking">
          {#each stats.top_books as b (b.book.id)}
            <li>
              <button class="rank-row" onclick={() => onopen(b.book)}>
                <span class="mini-cover"><BookCover title={b.book.title} coverKey={b.book.cover_key} bare /></span>
                <span class="rank-text">
                  <span class="rank-name">{b.book.title}</span>
                  <span class="rank-bar"><span style:width="{(b.seconds / topBookMax) * 100}%"></span></span>
                </span>
                <span class="rank-value">{formatReadingTime(b.seconds)}</span>
              </button>
            </li>
          {/each}
        </ol>
      </section>
      <section class="card" in:rise={{ delay: 360 }}>
        <h2>Authors</h2>
        <ol class="ranking">
          {#each stats.top_authors as a (a.name)}
            <li class="rank-row static">
              <span class="rank-text">
                <span class="rank-name">{a.name}</span>
                <span class="rank-bar"><span style:width="{(a.seconds / topAuthorMax) * 100}%"></span></span>
              </span>
              <span class="rank-value">{formatReadingTime(a.seconds)}</span>
            </li>
          {/each}
        </ol>
      </section>
    </div>
  {:else if stats}
    <div class="empty" in:rise>
      <Icon name="reading" size={26} />
      <p>Open a book and start reading: your time and streaks show up here.</p>
    </div>
  {/if}
</div>

{#if tip}
  <div class="tip" role="tooltip" style:left="{tip.x}px" style:top="{tip.y}px">
    <strong>{tip.value}</strong>
    <span>{tip.label}</span>
  </div>
{/if}

<style>
  .screen {
    height: 100%;
    overflow-y: auto;
    padding: 22px 28px 48px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
  }

  .subtitle {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--accent);
  }

  h1 {
    margin: 0;
    font-family: var(--serif);
    font-size: 26px;
    font-weight: 600;
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .year {
    display: flex;
    align-items: center;
    gap: 4px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .year button {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
  }

  .card {
    padding: 16px 18px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
  }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .label {
    font-size: 13px;
    color: var(--muted);
  }

  .sub {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--muted);
  }

  .top {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
    gap: 16px;
  }

  .hero-card {
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background:
      radial-gradient(circle at 100% 0%, color-mix(in srgb, var(--accent) 22%, transparent), transparent 60%),
      var(--surface);
  }

  .hero {
    font-size: 52px;
    font-weight: 650;
    line-height: 1.05;
    letter-spacing: -0.02em;
  }

  .goal-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .goal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .goal-value {
    font-size: 32px;
    font-weight: 650;
  }

  .of {
    font-size: 16px;
    font-weight: 500;
    color: var(--muted);
  }

  .meter {
    height: 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 18%, var(--surface-2));
    overflow: hidden;
  }

  .meter-fill {
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(170px, 100%), 1fr));
    gap: 12px;
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tile-value {
    font-size: 24px;
    font-weight: 650;
  }

  .unit {
    font-size: 14px;
    font-weight: 500;
    color: var(--muted);
  }

  /* ── Calendar ── */
  .legend {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    color: var(--muted);
  }

  .legend .cell {
    width: 11px;
  }

  .calendar-scroll {
    overflow-x: auto;
    padding-bottom: 4px;
  }

  .calendar {
    display: grid;
    grid-template-columns: 30px repeat(var(--weeks), minmax(9px, 1fr));
    grid-template-rows: 16px;
    gap: 3px;
    min-width: 560px;
    max-width: 1100px;
  }

  .month {
    grid-row: 1;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }

  .weekday {
    grid-column: 1;
    align-self: center;
    font-size: 10px;
    color: var(--muted);
  }

  .cell {
    aspect-ratio: 1;
    border-radius: 3px;
    transition: transform 0.12s ease;
  }

  .calendar .cell:not(.blank):hover {
    transform: scale(1.35);
    outline: 2px solid var(--surface);
  }

  .cell.today {
    box-shadow: inset 0 0 0 1.5px var(--text);
  }

  .blank {
    background: transparent;
  }

  .l0 {
    background: var(--surface-2);
  }
  .l1 {
    background: color-mix(in srgb, var(--accent) 30%, var(--surface-2));
  }
  .l2 {
    background: color-mix(in srgb, var(--accent) 55%, var(--surface-2));
  }
  .l3 {
    background: color-mix(in srgb, var(--accent) 78%, var(--surface-2));
  }
  .l4 {
    background: var(--accent);
  }

  .note {
    margin: 8px 0 0;
    font-size: 11px;
    color: var(--muted);
  }

  /* ── Month chart ── */
  .chart-box {
    width: 100%;
  }

  .chart {
    display: block;
    overflow: visible;
  }

  .grid-line {
    stroke: var(--border);
    stroke-width: 1;
  }

  .baseline {
    stroke: var(--border);
    stroke-width: 1;
  }

  .axis {
    fill: var(--muted);
    font-size: 11px;
  }

  .value {
    fill: var(--text);
    font-size: 11px;
    font-weight: 600;
  }

  .hit {
    fill: transparent;
  }

  .bar {
    fill: var(--accent);
    transform-box: fill-box;
    transform-origin: bottom;
    animation: grow 0.6s cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: var(--d);
    transition: filter 0.15s ease;
  }

  .col:hover .bar {
    filter: brightness(1.15);
  }

  @keyframes grow {
    from {
      scale: 1 0;
    }
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }

  th,
  td {
    padding: 6px 8px;
    text-align: right;
    border-bottom: 1px solid var(--border);
  }

  th:first-child,
  td:first-child {
    text-align: left;
  }

  th {
    font-weight: 600;
    color: var(--muted);
  }

  /* ── Finished shelf ── */
  .count {
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
  }

  .shelf {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    padding-bottom: 6px;
  }

  .finished {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 92px;
    padding: 0;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 11px;
    text-align: left;
  }

  .finished .cover {
    display: block;
    width: 92px;
    height: 138px;
    border-radius: 6px;
    overflow: hidden;
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.18);
    transition: transform 0.2s ease;
  }

  .finished:hover .cover {
    transform: translateY(-3px) rotate(-1deg);
  }

  /* ── Rankings ── */
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(320px, 100%), 1fr));
    gap: 16px;
  }

  .two h2 {
    margin-bottom: 10px;
  }

  .ranking {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .rank-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 10px;
    background: none;
    color: var(--text);
    text-align: left;
  }

  button.rank-row:hover {
    background: var(--surface-2);
  }

  .mini-cover {
    flex: none;
    width: 24px;
    height: 36px;
    border-radius: 3px;
    overflow: hidden;
  }

  .rank-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .rank-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }

  .rank-bar {
    height: 4px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 14%, var(--surface-2));
  }

  .rank-bar span {
    display: block;
    height: 100%;
    border-radius: 999px;
    background: var(--accent);
    animation: fill 0.7s cubic-bezier(0.22, 1, 0.36, 1) both;
    transform-origin: left;
  }

  @keyframes fill {
    from {
      scale: 0 1;
    }
  }

  .rank-value {
    flex: none;
    min-width: 76px;
    text-align: right;
    font-size: 12px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 30px 0;
    color: var(--muted);
  }

  .empty p {
    margin: 0;
  }

  .tip {
    position: fixed;
    z-index: 70;
    translate: -50% calc(-100% - 8px);
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.2);
    font-size: 12px;
    pointer-events: none;
    white-space: nowrap;
  }

  .tip strong {
    font-size: 13px;
  }

  .tip span {
    color: var(--muted);
  }

  .error {
    color: #dc2626;
  }

  button.small {
    padding: 4px 10px;
    font-size: 12px;
  }

  @media (max-width: 760px) {
    .screen {
      padding: 16px 16px 40px;
    }

    .top {
      grid-template-columns: 1fr;
    }

    .hero {
      font-size: 42px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bar,
    .rank-bar span {
      animation: none;
    }
  }
</style>
