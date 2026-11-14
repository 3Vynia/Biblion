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

test('settings are grouped and can be searched', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  const index = page.getByRole('navigation', { name: 'Settings sections' })
  await expect(index.locator('.group')).toHaveText(['General', 'Reading', 'Your data', 'Help'])

  // "/" searches the settings while they are open.
  await page.locator('h1').click()
  await page.keyboard.press('/')
  await expect(page.getByRole('searchbox', { name: 'Search settings' })).toBeFocused()
  await page.keyboard.type('wiktionary')
  await expect(index.getByRole('button')).toHaveText(['Privacy & online'])
  await expect(page.locator('#privacy')).toBeVisible()
  await expect(page.locator('#appearance')).toBeHidden()

  await page.keyboard.press('Control+a')
  await page.keyboard.type('zzz')
  await expect(index).toContainText('No settings match “zzz”.')
  await page.keyboard.press('Escape')
  await expect(page.getByRole('searchbox', { name: 'Search settings' })).toHaveValue('')
  await expect(page.locator('#appearance')).toBeVisible()
  // Esc cleared the search but did not leave Settings.
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible()
})

test('each section says what it is set to', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  const summary = page.locator('#library .summary')
  await expect(summary).toContainText('watching on')
  await page.getByText('Watch folders for changes').click()
  await expect(summary).toContainText('watching off')
  await expect(page.locator('#privacy .summary')).toHaveText('Only single words go online, to Wiktionary')
  await page.getByText('Look words up online').click()
  await expect(page.locator('#privacy .summary')).toHaveText('Nothing leaves this computer')
})

test('removing a library folder asks first', async ({ page }) => {
  const removed = () =>
    page.evaluate(() => (window as unknown as { __calls: { cmd: string }[] }).__calls.filter((c) => c.cmd === 'remove_root').length)
  await page.getByRole('button', { name: 'Settings' }).click()
  const folder = page.locator('#library .folder').first()
  await folder.getByRole('button', { name: 'Remove' }).click()
  const dialog = page.getByRole('alertdialog')
  await expect(dialog).toContainText('Stop using this folder?')
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  expect(await removed()).toBe(0)
  await folder.getByRole('button', { name: 'Remove' }).click()
  await dialog.getByRole('button', { name: 'Remove folder' }).click()
  await expect.poll(removed).toBe(1)
})

test('the index highlights the section you jumped to', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  const index = page.getByRole('navigation', { name: 'Settings sections' })
  for (const name of ['Ambience', 'Backup', 'About', 'Library']) {
    await index.getByRole('button', { name, exact: true }).click()
    await page.waitForTimeout(700)
    await expect(index.locator('[aria-current="true"]')).toHaveText(name)
  }
})
