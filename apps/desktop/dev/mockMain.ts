// Development harness: runs the real UI in a normal browser against an
// in-memory fake backend, so screens can be tested without the desktop app.
// Open http://localhost:1420/dev/mock.html while `pnpm dev` is running.
import { mockConvertFileSrc, mockIPC } from '@tauri-apps/api/mocks'
import { handle } from './mockBackend'

mockConvertFileSrc('linux')
mockIPC((cmd, args) => handle(cmd, (args ?? {}) as Record<string, unknown>))
await import('../src/main')
