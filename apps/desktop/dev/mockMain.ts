// Development harness: runs the real UI in a normal browser against an
// in-memory fake backend, so screens can be tested without the desktop app.
// Open http://localhost:1420/dev/mock.html while `pnpm dev` is running.
import { mockConvertFileSrc, mockIPC } from '@tauri-apps/api/mocks'
import { handle } from './mockBackend'

mockConvertFileSrc('linux')

// The reader fetches books from book://; serve the test EPUB instead.
const realFetch = window.fetch.bind(window)
window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
  return url.startsWith('book://') ? realFetch('/dev/fixtures/sample.epub', init) : realFetch(input, init)
}
mockIPC((cmd, args) => handle(cmd, (args ?? {}) as Record<string, unknown>))
await import('../src/main')
