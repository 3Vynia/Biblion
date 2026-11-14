// In-app navigation: which screen is shown, with a back stack.

import type { BookQuery, Format, SortBy } from './types'

/** Filters a "books" screen applies on top of the search text. */
export type BookFilter = Pick<
  BookQuery,
  | 'author_id'
  | 'series'
  | 'tag'
  | 'language'
  | 'shelf_id'
  | 'format'
  | 'favorites_only'
  | 'reading_only'
  | 'finished_only'
>

export type Route =
  | { name: 'home' }
  | { name: 'books'; title: string; subtitle?: string; filter: BookFilter; sort?: SortBy; section?: Section }
  | { name: 'authors' }
  | { name: 'series' }
  | { name: 'subjects' }
  | { name: 'notes' }
  | { name: 'stats' }
  | { name: 'collections' }
  | { name: 'formats' }
  | { name: 'health' }
  | { name: 'settings' }

export type BooksRoute = Extract<Route, { name: 'books' }>

/** Sidebar entries; `books` routes remember which entry they belong to. */
export type Section =
  | 'home'
  | 'all'
  | 'reading'
  | 'favorites'
  | 'to-read'
  | 'finished'
  | 'authors'
  | 'series'
  | 'subjects'
  | 'notes'
  | 'stats'
  | 'collections'
  | 'formats'
  | 'health'
  | 'settings'

export function sectionOf(route: Route): Section {
  switch (route.name) {
    case 'home':
      return 'home'
    case 'books':
      return route.section ?? 'all'
    default:
      return route.name
  }
}

export const allBooks = (): BooksRoute => ({ name: 'books', title: 'All books', filter: {}, section: 'all' })
export const readingNow = (): BooksRoute => ({
  name: 'books',
  title: 'Reading now',
  filter: { reading_only: true },
  sort: 'recently_read',
  section: 'reading',
})
export const favorites = (): BooksRoute => ({ name: 'books', title: 'Favourites', filter: { favorites_only: true }, section: 'favorites' })
export const finished = (): BooksRoute => ({
  name: 'books',
  title: 'Finished',
  filter: { finished_only: true },
  sort: 'recently_read',
  section: 'finished',
})
export const shelfRoute = (id: number, name: string, section: Section = 'collections'): BooksRoute => ({
  name: 'books',
  title: name,
  subtitle: 'Collection',
  filter: { shelf_id: id },
  sort: 'added_desc',
  section,
})
export const authorRoute = (id: number, name: string): BooksRoute => ({
  name: 'books',
  title: name,
  subtitle: 'Author',
  filter: { author_id: id },
  sort: 'series',
  section: 'authors',
})
export const seriesRoute = (name: string): BooksRoute => ({
  name: 'books',
  title: name,
  subtitle: 'Series',
  filter: { series: name },
  sort: 'series',
  section: 'series',
})
export const formatRoute = (format: Format, label: string): BooksRoute => ({
  name: 'books',
  title: label,
  subtitle: 'Format',
  filter: { format },
  section: 'formats',
})

export const tagRoute = (tag: string): BooksRoute => ({
  name: 'books',
  title: tag,
  subtitle: 'Subject',
  filter: { tag },
  section: 'subjects',
})

/** Stable key so a screen re-renders (and re-animates) only when it changes. */
export const routeKey = (r: Route) => JSON.stringify(r)

class Nav {
  route = $state<Route>({ name: 'home' })
  #stack: Route[] = []
  canGoBack = $state(false)
  /** Counts every move, even to the screen already shown, so a search can end on any click. */
  moves = $state(0)
  /** True while search results replace the current screen. */
  searching = $state(false)

  go(route: Route) {
    this.moves++
    if (routeKey(route) === routeKey(this.route)) return
    this.#stack.push(this.route)
    if (this.#stack.length > 50) this.#stack.shift()
    this.route = route
    this.canGoBack = true
  }

  /** Top-level jump from the sidebar: resets the back stack. */
  jump(route: Route) {
    this.moves++
    this.#stack = []
    this.canGoBack = false
    this.route = route
  }

  back() {
    this.moves++
    const prev = this.#stack.pop()
    if (prev) this.route = prev
    this.canGoBack = this.#stack.length > 0
  }
}

export const nav = new Nav()
