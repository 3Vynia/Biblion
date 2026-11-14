import { expect, test, type Page } from '@playwright/test'

const calls = (page: Page, cmd: string) =>
  page.evaluate(
    (c) => (window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls.filter((x) => x.cmd === c),
    cmd,
  )

async function openFirstBook(page: Page) {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().click()
  return page.getByRole('complementary', { name: 'Book details' })
}

test('looking a book up is off until you allow it', async ({ page }) => {
  const panel = await openFirstBook(page)
  await panel.getByRole('button', { name: 'Find details online' }).click()
  const ask = page.getByRole('alertdialog')
  await expect(ask).toContainText('Look this book up online?')
  await expect(ask).toContainText('openlibrary.org')
  await ask.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('dialog', { name: 'Find details online' })).toHaveCount(0)
  expect(await calls(page, 'ol_search')).toHaveLength(0)

  await panel.getByRole('button', { name: 'Find details online' }).click()
  await ask.getByRole('button', { name: 'Turn on and search' }).click()
  await expect(page.getByRole('dialog', { name: 'Find details online' })).toBeVisible()
  expect((await calls(page, 'set_online_metadata')).at(-1)?.args.on).toBe(true)

  // Settings shows it as on, and says so in the summary.
  await page.keyboard.press('Escape')
  await page.keyboard.press('Control+,')
  await expect(page.getByRole('switch', { name: /Find book details online/ })).toBeChecked()
  await expect(page.locator('#privacy .summary')).toContainText('Open Library')
})

test('pick a match, choose what to take, and confirm', async ({ page }) => {
  const panel = await openFirstBook(page)
  const title = (await panel.locator('h2').textContent())!.trim()
  await panel.getByRole('button', { name: 'Find details online' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Turn on and search' }).click()

  const lookup = page.getByRole('dialog', { name: 'Find details online' })
  await expect(lookup.getByLabel('Title')).toHaveValue(title)
  await expect(lookup.locator('.result')).toHaveCount(5)
  await expect(lookup.locator('.result img').first()).toBeVisible()
  await lookup.locator('.result').first().click()

  // What the book lacks is pre-ticked; replacing what it has is not.
  const field = (name: string) => lookup.locator('.field', { hasText: name })
  await expect(field('Subjects')).toContainText('+ Dragons')
  await expect(field('First published').getByRole('checkbox')).not.toBeChecked()
  await expect(field('Description').getByRole('checkbox')).not.toBeChecked()
  await expect(lookup.getByRole('checkbox', { name: 'Use this cover' })).toBeChecked()

  await field('Description').click()
  await field('Subjects').click()
  await expect(lookup.locator('.selected')).toHaveText(/\d+ changes selected/)

  // Cancelling the confirmation changes nothing.
  await lookup.getByRole('button', { name: 'Update book…' }).click()
  const ask = page.getByRole('alertdialog')
  await expect(ask).toContainText('cover')
  await ask.getByRole('button', { name: 'Cancel' }).click()
  expect(await calls(page, 'update_metadata')).toHaveLength(0)

  await lookup.getByRole('button', { name: 'Update book…' }).click()
  await ask.getByRole('button', { name: 'Update book' }).click()
  await expect(lookup).toHaveCount(0)
  await expect(panel.getByRole('status')).toContainText('from Open Library')
  const saved = (await calls(page, 'update_metadata'))[0].args.metadata as { description: string; tags: string[] }
  expect(saved.description).toContain('there and back again')
  expect(saved.tags).toContain('Dwarves')
  expect((await calls(page, 'ol_apply_cover'))[0].args.coverId).toBe(9000)
  await expect(panel).toContainText('A long journey there and back again')
})

test('no matches and lost connections are explained', async ({ page }) => {
  const panel = await openFirstBook(page)
  await panel.getByRole('button', { name: 'Find details online' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Turn on and search' }).click()
  const lookup = page.getByRole('dialog', { name: 'Find details online' })
  await lookup.getByLabel('Title').fill('zzz')
  await lookup.getByLabel('Title').press('Enter')
  await expect(lookup).toContainText('Nothing found')
  await lookup.getByLabel('Title').fill('offline')
  await lookup.getByRole('button', { name: 'Search' }).click()
  await expect(lookup).toContainText('Could not reach Open Library')
  await expect(lookup.getByRole('button', { name: 'Try again' })).toBeVisible()

  // Keys stay in the window: Esc closes it, not the book panel.
  await page.keyboard.press('Escape')
  await expect(lookup).toHaveCount(0)
  await expect(panel).toBeVisible()
})
