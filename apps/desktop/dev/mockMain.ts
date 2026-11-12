// Development harness: runs the real UI in a normal browser against an
// in-memory fake backend, so screens can be tested without the desktop app.
// Open http://localhost:1420/dev/mock.html while `pnpm dev` is running.
import { mockConvertFileSrc, mockIPC } from '@tauri-apps/api/mocks'
import { formatOf, handle } from './mockBackend'

mockConvertFileSrc('linux')

// Comic pages come from comic://<id>/<page>; use the sample pages instead.
const internals = (window as unknown as { __TAURI_INTERNALS__: { convertFileSrc: (p: string, s?: string) => string } })
  .__TAURI_INTERNALS__
const convert = internals.convertFileSrc
internals.convertFileSrc = (path, scheme = 'asset') =>
  scheme === 'comic' ? `/dev/fixtures/comic/${Number(path.split('/')[1]) % 7}.jpg` : convert(path, scheme)

// The reader fetches books from book://; serve the test EPUB instead.
const realFetch = window.fetch.bind(window)
window.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
  if (url.startsWith('book://')) {
    const sample = formatOf(Number(url.split('/').pop())) === 'pdf' ? 'sample.pdf' : 'sample.epub'
    return realFetch(`/dev/fixtures/${sample}`, init)
  }
  // Never reach the real Wiktionary from tests: answer with a fixed entry.
  if (url.startsWith('https://en.wiktionary.org/')) {
    const word = decodeURIComponent(url.split('/').pop() ?? '')
    const body = {
      en: [{ partOfSpeech: 'Noun', language: 'English', definitions: [{ definition: `A <b>test</b> sense of <i>${word}</i>.` }] }],
    }
    return Promise.resolve(new Response(JSON.stringify(body), { headers: { 'Content-Type': 'application/json' } }))
  }
  return realFetch(input, init)
}
mockIPC((cmd, args) => handle(cmd, (args ?? {}) as Record<string, unknown>))
await import('../src/main')
