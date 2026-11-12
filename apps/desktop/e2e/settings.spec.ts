import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.goto('/dev/mock.html')
})

test('the sidebar collapses to icons and remembers it', async ({ page }) => {
  const sidebar = page.locator('.sidebar')
  await expect(sidebar).not.toHaveClass(/collapsed/)
  await page.getByRole('button', { name: 'Hide sidebar' }).click()
  await expect(sidebar).toHaveClass(/collapsed/)
  await expect.poll(() => sidebar.evaluate((el) => el.getBoundingClientRect().width)).toBeLessThan(80)
  await page.keyboard.press('Control+b')
  await expect(sidebar).not.toHaveClass(/collapsed/)
})

test('settings change theme, accent and cover size', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible()

  await page.getByRole('radio', { name: 'Dark' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await page.getByRole('radio', { name: 'Light' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')

  await page.getByRole('radio', { name: 'Teal' }).click()
  await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--accent'))).toBe('#0f766e')

  await page.getByRole('button', { name: 'Large', exact: true }).click()
  await page.getByRole('button', { name: /All books/ }).click()
  const width = await page.locator('.cell').first().evaluate((el) => el.getBoundingClientRect().width)
  expect(width).toBeGreaterThan(190)
})

test('the book panel shows full metadata and stats', async ({ page }) => {
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().click()
  const panel = page.getByRole('complementary', { name: 'Book details' })
  await expect(panel.getByText('Translated by')).toBeVisible()
  await expect(panel.getByText('98,765')).toBeVisible()
  await expect(panel.getByText('6h 36m')).toBeVisible()
  await expect(panel.getByText('1 image', { exact: true })).toHaveCount(0)
  await expect(panel.getByText('images', { exact: true })).toBeVisible()
  await expect(panel.getByText('EPUB 3.0')).toBeVisible()
  await expect(panel.getByText('978-84-01-35230-9')).toBeVisible()
  const subject = (await panel.locator('.tag').first().textContent())!.trim()
  await panel.locator('.tag').first().click()
  await expect(page.getByRole('heading', { name: subject, level: 1 })).toBeVisible()
})

test('dictionaries are listed and removing one asks first; online lookups can be turned off', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  const row = page.locator('.folder', { hasText: 'Test English Dictionary' })
  await expect(row).toBeVisible()
  await row.getByRole('button', { name: 'Remove' }).click()
  await expect(page.getByRole('alertdialog')).toContainText('Remove “Test English Dictionary”?')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(row).toBeVisible()
  await row.getByRole('button', { name: 'Remove' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Remove dictionary' }).click()
  await expect(page.getByText('No offline dictionaries yet.')).toBeVisible()

  const online = page.getByRole('switch', { name: /Look words up online/ })
  await expect(online).toBeChecked()
  await page.getByText('Look words up online').click()
  await expect(online).not.toBeChecked()
})

test('folder watching can be turned off, and changes found by it show up', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  const watch = page.getByRole('switch', { name: /Watch folders for changes/ })
  await expect(watch).toBeChecked()
  await page.getByText('Watch folders for changes').click()
  await expect(watch).not.toBeChecked()
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: { on: boolean } }[] }).__calls.findLast((c) => c.cmd === 'set_watch_folders')?.args.on),
    )
    .toBe(false)

  // What the backend sends after rescanning a watched folder.
  await page.evaluate(() =>
    (window as unknown as { __emit: (e: string, p: unknown) => Promise<void> }).__emit('library-changed', {
      found: 3, unchanged: 0, added: 3, updated: 0, missing: 0, errors: 0, covers: 0, elapsed_ms: 12, error_samples: [],
    }),
  )
  await expect(page.getByRole('status')).toHaveText('3 new books in your library')
  await page.getByRole('button', { name: 'Dismiss' }).click()
  await expect(page.getByRole('status')).toHaveCount(0)
})

test('books added in quick succession are announced together', async ({ page }) => {
  const emit = (added: number) =>
    page.evaluate(
      (n) =>
        (window as unknown as { __emit: (e: string, p: unknown) => Promise<void> }).__emit('library-changed', {
          found: n, unchanged: 0, added: n, updated: 0, missing: 0, errors: 0, covers: 0, elapsed_ms: 5, error_samples: [],
        }),
      added,
    )
  await page.clock.install()
  await page.goto('/dev/mock.html')
  await expect(page.getByRole('button', { name: /All books/ })).toBeVisible()
  await page.waitForTimeout(300) // let the app subscribe
  const status = page.getByRole('status')
  await emit(2)
  await expect(status).toHaveText('2 new books in your library', { useInnerText: true })
  await emit(5)
  await emit(1)
  await expect(status).toHaveText('2 new books in your library', { useInnerText: true })
  await page.clock.runFor(61_000)
  await expect(status).toHaveText('6 new books in your library', { useInnerText: true })
})
