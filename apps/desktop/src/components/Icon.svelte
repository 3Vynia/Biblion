<script lang="ts" module>
  // A small set of line icons (24×24, stroke-based) drawn for Biblion.
  const PATHS = {
    home: 'M3 10.5 12 3l9 7.5M5 9v11h5v-6h4v6h5V9',
    books: 'M4 4h4v16H4zM10 4h4v16h-4zM16 5l3.6-1 2.4 15.4-3.6 1z',
    reading: 'M2 5c3-1.5 6.5-1.5 10 1 3.5-2.5 7-2.5 10-1v14c-3-1.5-6.5-1.5-10 1-3.5-2.5-7-2.5-10-1zM12 6v14',
    heart: 'M12 20s-7-4.4-9.2-9A5 5 0 0 1 12 6a5 5 0 0 1 9.2 5C19 15.6 12 20 12 20z',
    bookmark: 'M6 3h12v18l-6-4.5L6 21z',
    check: 'M4 12.5 9.5 18 20 6',
    user: 'M12 12a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM3.5 21a8.5 8.5 0 0 1 17 0',
    layers: 'M12 3 2 8.5 12 14l10-5.5zM2 13l10 5.5L22 13M2 17.5 12 23l10-5.5',
    collection: 'M3 7h18v13H3zM6 4h12M8 11h8',
    file: 'M6 2h8l5 5v15H6zM14 2v5h5',
    folder: 'M3 6h6l2 2h10v12H3z',
    search: 'M10.5 18a7.5 7.5 0 1 0 0-15 7.5 7.5 0 0 0 0 15zM16 16l5 5',
    chevronLeft: 'M15 5l-7 7 7 7',
    chevronRight: 'M9 5l7 7-7 7',
    plus: 'M12 5v14M5 12h14',
    close: 'M6 6l12 12M18 6 6 18',
    play: 'M8 5v14l11-7z',
    sparkle: 'M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 16l.8 2.2L22 19l-2.2.8L19 22l-.8-2.2L16 19l2.2-.8z',
    menu: 'M4 6h16M4 12h16M4 18h16',
    panel: 'M3 4h18v16H3zM9 4v16',
    settings:
      'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z',
    copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
    tag: 'M3 12V3h9l9 9-9 9zM7.5 7.5h.01',
    clock: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2',
    image: 'M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6M15 9h.01',
    text: 'M4 6h16M4 12h16M4 18h10',
    info: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5h.01',
    trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3',
    edit: 'M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4',
    more: 'M5 12h.01M12 12h.01M19 12h.01',
    filter: 'M3 5h18l-7 8v6l-4 2v-8z',
    globe: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM3 12h18M12 3c2.5 2.7 3.8 5.7 3.8 9s-1.3 6.3-3.8 9c-2.5-2.7-3.8-5.7-3.8-9S9.5 5.7 12 3z',
  } as const

  export type IconName = keyof typeof PATHS
</script>

<script lang="ts">
  interface Props {
    name: IconName
    size?: number
    filled?: boolean
    strokeWidth?: number
  }

  let { name, size = 20, filled = false, strokeWidth = 1.8 }: Props = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={PATHS[name]} />
</svg>

<style>
  svg {
    flex-shrink: 0;
    display: block;
  }
</style>
