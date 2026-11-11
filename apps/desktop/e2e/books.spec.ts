import { expect, test, type Page } from '@playwright/test'

const sortsSent = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { __calls: { cmd: string; args: { query: { sort: string } } }[] }).__calls
      .filter((c) => c.cmd === 'list_books')
      .map((c) => c.args.query.sort),
  )

test.beforeEach(async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  await page.goto('/dev/mock.html')
  test.info().attach('page errors', { body: errors.join('\n') })
})

test('changing the sort order reloads the list', async ({ page }) => {
  await page.getByRole('button', { name: /All books/ }).click()
  const firstTitle = page.locator('.cell .caption .title').first()
  await expect(firstTitle).toHaveText('Abyss of the abyss 1')

  await page.selectOption('.sort select', 'added_desc')
  await expect.poll(async () => (await sortsSent(page)).at(-1)).toBe('added_desc')
  await expect(page.locator('.cell .caption .title').nth(1)).toHaveText('Silence of the river 2')

  await page.selectOption('.sort select', 'author')
  await expect.poll(async () => (await sortsSent(page)).at(-1)).toBe('author')
  await expect(page.locator('.cell').first()).toBeVisible()
})

test('sidebar sections filter the books', async ({ page }) => {
  await page.getByRole('button', { name: /Favourites/ }).click()
  await expect(page.getByRole('heading', { name: 'Favourites' })).toBeVisible()
  await expect(page.locator('.books .count')).toHaveText(/^\d+ books?$/)
  const count = Number((await page.locator('.books .count').textContent())!.split(' ')[0])
  expect(count).toBeGreaterThan(0)
  expect(count).toBeLessThan(3000)
})

test('no runtime errors while browsing', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(e.message))
  for (const name of [/All books/, /Reading now/, /Authors/, /Series/, /Collections/, /Formats/, /Home/]) {
    await page.getByRole('button', { name }).first().click()
    await page.waitForTimeout(300)
  }
  expect(errors).toEqual([])
})
