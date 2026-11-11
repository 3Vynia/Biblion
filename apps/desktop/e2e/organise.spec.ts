import { expect, test, type Page } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.goto('/dev/mock.html')
})

async function createCollection(page: Page, name: string) {
  await page.getByRole('button', { name: /Collections/ }).click()
  await page.getByRole('button', { name: 'New collection' }).click()
  await page.getByPlaceholder('Collection name').fill(name)
  await page.getByRole('button', { name: 'Create' }).click()
  await expect(page.getByRole('heading', { name, level: 1 })).toBeVisible()
}

test('renaming a collection asks for the name and then for confirmation', async ({ page }) => {
  await createCollection(page, 'Sci-fi')
  await page.getByRole('button', { name: 'Rename collection' }).click()
  await page.getByLabel('New name').fill('Science fiction')
  await page.getByRole('button', { name: 'Continue' }).click()
  const dialog = page.getByRole('alertdialog')
  await expect(dialog).toContainText('“Sci-fi” will be renamed to “Science fiction”')

  // Cancelling keeps the old name.
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('heading', { name: 'Sci-fi', level: 1 })).toBeVisible()

  await page.getByRole('button', { name: 'Rename collection' }).click()
  await page.getByLabel('New name').fill('Science fiction')
  await page.getByRole('button', { name: 'Continue' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Rename' }).click()
  await expect(page.getByRole('heading', { name: 'Science fiction', level: 1 })).toBeVisible()
})

test('deleting a collection needs confirmation and keeps the books', async ({ page }) => {
  await createCollection(page, 'Temporary')
  await page.getByRole('button', { name: 'Delete collection' }).click()
  const dialog = page.getByRole('alertdialog')
  await expect(dialog).toContainText("can't be undone")
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: 'Temporary', level: 1 })).toBeVisible()

  await page.getByRole('button', { name: 'Delete collection' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete collection' }).click()
  await expect(page.getByRole('heading', { name: 'Collections', level: 1 })).toBeVisible()
  await expect(page.getByRole('button', { name: /Temporary/ })).toHaveCount(0)
  await expect(page.getByRole('button', { name: /All books/ })).toContainText('3,000')
})

test('books can be filtered by subject, language and format', async ({ page }) => {
  await page.getByRole('button', { name: /All books/ }).click()
  await expect(page.locator('.books .count')).toHaveText('3,000 books')
  await page.getByRole('button', { name: 'Filters' }).click()
  await page.getByPlaceholder(/Science fiction/).fill('Hist')
  await page.getByRole('button', { name: /^History/ }).click()
  await expect(page.locator('.books .count')).toHaveText('750 books')
  await page.getByRole('button', { name: /^Spanish/ }).click()
  await expect(page.locator('.books .count')).toHaveText(/^\d+ books?$/)
  const withLanguage = Number((await page.locator('.books .count').textContent())!.split(' ')[0])
  expect(withLanguage).toBeLessThan(750)
  // Esc closes only the panel; chips remove the filters one by one.
  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog', { name: 'Filters' })).toHaveCount(0)
  await page.locator('.chip', { hasText: 'Spanish' }).click()
  await expect(page.locator('.books .count')).toHaveText('750 books')
})

test('subjects screen opens the books of a subject', async ({ page }) => {
  await page.getByRole('button', { name: /Subjects/ }).click()
  await page.getByRole('button', { name: /^Poetry/ }).click()
  await expect(page.getByRole('heading', { name: 'Poetry', level: 1 })).toBeVisible()
  await expect(page.locator('.books .count')).toHaveText('500 books')
})

test('the compact sidebar centres its icons and has no folders button', async ({ page }) => {
  await expect(page.getByRole('button', { name: /Library folders/ })).toHaveCount(0)
  await page.getByRole('button', { name: 'Hide sidebar' }).click()
  const sidebar = page.locator('.sidebar')
  await expect.poll(() => sidebar.evaluate((el) => el.getBoundingClientRect().width)).toBe(68)
  const centre = await sidebar.evaluate((el) => el.getBoundingClientRect().width / 2)
  for (const name of ['Home', 'Authors', 'Settings', 'Show sidebar']) {
    const box = (await page.getByRole('button', { name, exact: true }).locator('svg').boundingBox())!
    expect(Math.abs(box.x + box.width / 2 - centre)).toBeLessThanOrEqual(1.5)
  }
})
