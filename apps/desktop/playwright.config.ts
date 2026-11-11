import { defineConfig } from '@playwright/test'

// End-to-end tests of the UI against the in-memory mock backend (dev/).
export default defineConfig({
  testDir: 'e2e',
  use: { baseURL: 'http://localhost:1420', viewport: { width: 1400, height: 900 } },
  webServer: { command: 'pnpm dev', url: 'http://localhost:1420/dev/mock.html', reuseExistingServer: true },
})
