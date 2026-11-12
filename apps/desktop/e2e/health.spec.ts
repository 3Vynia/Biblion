import { expect, test, type Page } from '@playwright/test'

const calls = (page: Page, cmd: string) =>
  page.evaluate(
    (c) => (window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls.filter((x) => x.cmd === c),
    cmd,
  )

test.beforeEach(async ({ page }) => {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: 'Library health' }).click()
  await expect(page.getByRole('heading', { name: 'Library health', level: 1 })).toBeVisible()
})

test('identical copies: keep one sends the others to the trash, after confirming', async ({ page }) => {
  const groups = page.locator('.group')
  await expect(groups).toHaveCount(2)
  await expect(groups.first().locator('.row').first()).toContainText('Suggested to keep')

  const second = groups.nth(1)
  await second.getByRole('button', { name: 'Keep one' }).click()
  const dialog = page.getByRole('alertdialog')
  await expect(dialog).toContainText('2 other copies')
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(groups).toHaveCount(2)
  expect(await calls(page, 'trash_books')).toHaveLength(0)

  await second.getByRole('button', { name: 'Keep one' }).click()
  await dialog.getByRole('button', { name: 'Trash 2 copies' }).click()
  await expect(groups).toHaveCount(1)
  const [trash] = await calls(page, 'trash_books')
  expect((trash.args.ids as number[]).length).toBe(2)

  await page.getByRole('radio', { name: 'Same book' }).click()
  await expect(groups).toHaveCount(1)
  await expect(groups.first()).toContainText('2 files')
})

test('unreadable files can be read again, or trashed one by one', async ({ page }) => {
  await page.getByRole('tab', { name: /Unreadable/ }).click()
  await expect(page.locator('.summary')).toHaveText('30 books')
  await page.locator('.list .row').first().getByRole('button', { name: 'Read again' }).click()
  await expect(page.locator('.summary')).toHaveText('29 books')

  await page.locator('.list .row').first().getByRole('button', { name: /to trash/ }).click()
  await expect(page.getByRole('alertdialog')).toContainText('system trash')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Move to trash' }).click()
  await expect(page.locator('.summary')).toHaveText('28 books')

  await page.getByRole('button', { name: 'Read all again' }).click()
  await expect(page.getByRole('alertdialog')).toContainText('28 books can be read now')
  await page.getByRole('alertdialog').getByRole('button', { name: 'OK' }).click()
  await expect(page.getByText('Every file could be read.')).toBeVisible()
})

test('missing books are forgotten only after confirming', async ({ page }) => {
  await page.getByRole('tab', { name: /Missing/ }).click()
  await expect(page.locator('.summary')).toHaveText('5 books')
  await page.locator('.list .row').first().getByRole('button', { name: 'Forget' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Forget book' }).click()
  await expect(page.locator('.summary')).toHaveText('4 books')

  await page.getByRole('button', { name: 'Forget all' }).click()
  await expect(page.getByRole('alertdialog')).toContainText('Forget 4 missing books?')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(page.locator('.summary')).toHaveText('4 books')
  await page.getByRole('button', { name: 'Forget all' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Forget all' }).click()
  await expect(page.getByText('No file is missing.')).toBeVisible()
  await expect(page.getByRole('tab', { name: /Missing/ })).toContainText('0')
})
