<script lang="ts">
  // A titled, horizontally scrolling row of covers with arrow buttons.
  import BookCover from './BookCover.svelte'
  import Icon from './Icon.svelte'
  import { rise } from '../lib/motion'
  import type { BookSummary } from '../lib/types'

  interface Props {
    title: string
    books: BookSummary[]
    onselect: (book: BookSummary) => void
    onopen: (book: BookSummary) => void
    onall?: () => void
  }

  let { title, books, onselect, onopen, onall }: Props = $props()

  let scroller: HTMLDivElement
  let atStart = $state(true)
  let atEnd = $state(false)

  function update() {
    if (!scroller) return
    atStart = scroller.scrollLeft < 4
    atEnd = scroller.scrollLeft + scroller.clientWidth >= scroller.scrollWidth - 4
  }

  function page(dir: 1 | -1) {
    scroller.scrollBy({ left: dir * scroller.clientWidth * 0.8, behavior: 'smooth' })
  }

  $effect(() => {
    void books
    queueMicrotask(update)
  })
</script>

<section class="row">
  <header>
    <h2>{title}</h2>
    {#if onall}<button class="ghost all" onclick={onall}>See all</button>{/if}
    <button class="ghost arrow" disabled={atStart} onclick={() => page(-1)} aria-label="Scroll left"
      ><Icon name="chevronLeft" size={18} /></button
    >
    <button class="ghost arrow" disabled={atEnd} onclick={() => page(1)} aria-label="Scroll right"
      ><Icon name="chevronRight" size={18} /></button
    >
  </header>
  <div class="scroller" bind:this={scroller} onscroll={update}>
    {#each books as book, i (book.id)}
      <button
        class="book"
        in:rise={{ delay: Math.min(i, 10) * 35, y: 14 }}
        onclick={() => onselect(book)}
        ondblclick={() => onopen(book)}
        title={book.title}
      >
        <div class="cover">
          <BookCover title={book.title} author={book.authors[0]} coverKey={book.cover_key} />
          {#if book.progress}
            <div class="progress"><div style:width="{book.progress * 100}%"></div></div>
          {/if}
        </div>
        <span class="title">{book.title}</span>
        <span class="author">{book.authors[0] ?? 'Unknown author'}</span>
      </button>
    {/each}
  </div>
</section>

<style>
  .row {
    margin-top: 28px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 10px;
  }

  h2 {
    flex: 1;
    margin: 0;
    font-family: var(--serif);
    font-size: 20px;
    font-weight: 600;
  }

  .all {
    color: var(--accent);
    font-weight: 600;
  }

  .arrow {
    padding: 4px;
    border-radius: 50%;
  }

  .scroller {
    display: flex;
    gap: 18px;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 6px 4px 14px;
    margin: 0 -4px;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
  }

  .scroller::-webkit-scrollbar {
    display: none;
  }

  .book {
    flex: 0 0 128px;
    width: 128px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
    scroll-snap-align: start;
  }

  .book:hover:not(:disabled) {
    background: none;
  }

  .cover {
    position: relative;
    width: 128px;
    height: 192px;
    border-radius: 4px 6px 6px 4px;
    overflow: hidden;
    box-shadow: var(--shadow);
    margin-bottom: 8px;
    transition:
      transform 260ms var(--ease),
      box-shadow 260ms var(--ease);
  }

  .book:hover .cover {
    transform: translateY(-5px) rotate(-0.6deg);
    box-shadow: var(--shadow-lift);
  }

  .progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: rgb(0 0 0 / 0.35);
  }

  .progress div {
    height: 100%;
    background: var(--accent);
  }

  .title,
  .author {
    display: block;
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
    font-size: 13px;
  }

  .author {
    font-size: 12px;
    color: var(--muted);
  }
</style>
