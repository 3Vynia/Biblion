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
  for (const name of [/All books/, /Reading now/, /^Notes/, /Authors/, /Series/, /Subjects/, /Collections/, /Formats/, /Settings/, /Home/]) {
    await page.getByRole('button', { name }).first().click()
    await page.waitForTimeout(300)
  }
  expect(errors).toEqual([])
})

test('in a narrow window the book panel floats and the grid keeps its columns', async ({ page }) => {
  await page.setViewportSize({ width: 950, height: 1000 })
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().click()
  await expect(page.locator('.drawer').getByRole('complementary', { name: 'Book details' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'All books', level: 1 })).toBeVisible()
  const columns = await page.locator('.cell').evaluateAll((cells) => new Set(cells.map((c) => c.getBoundingClientRect().top)).size)
  expect(columns).toBeGreaterThan(0)
  const cardWidth = await page.locator('.cell').first().evaluate((el) => el.getBoundingClientRect().width)
  expect(cardWidth).toBeLessThan(210)
  // Clicking outside closes it.
  await page.locator('.scrim').click({ position: { x: 10, y: 500 } })
  await expect(page.locator('.drawer')).toHaveCount(0)
})
