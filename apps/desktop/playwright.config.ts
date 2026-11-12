import { defineConfig } from '@playwright/test'

// End-to-end tests of the UI against the in-memory mock backend (dev/).
export default defineConfig({
  testDir: 'e2e',
  // Its own port, so it never clashes with `pnpm tauri dev` on 1420.
  use: { baseURL: 'http://localhost:1430', viewport: { width: 1400, height: 900 } },
  webServer: {
    command: 'pnpm vite --port 1430 --strictPort',
    url: 'http://localhost:1430/dev/mock.html',
    reuseExistingServer: true,
  },
})
