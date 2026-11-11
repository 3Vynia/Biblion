import { expect, test, type Frame, type Page } from '@playwright/test'

/** Opens the first book (an EPUB served from dev/fixtures/sample.epub). */
async function openReader(page: Page): Promise<Frame> {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().dblclick()
  await expect(page.getByRole('dialog', { name: /^Reading/ })).toBeVisible()
  let frame: Frame | undefined
  await expect
    .poll(async () => {
      for (const f of page.frames()) {
        if (f.url().startsWith('blob:') && (await f.locator('p').count().catch(() => 0)) > 0) frame = f
      }
      return !!frame
    })
    .toBe(true)
  return frame!
}

/** Selects characters [from, to) of the first paragraph on the page. */
async function selectText(frame: Frame, to: number, from = 0) {
  await frame.evaluate(([n, start]) => {
    const p = document.querySelector('p')!
    const range = document.createRange()
    range.setStart(p.firstChild!, start)
    range.setEnd(p.firstChild!, n)
    const sel = document.getSelection()!
    sel.removeAllRanges()
    sel.addRange(range)
    document.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }))
  }, [to, from] as const)
}

test('highlight a passage, see it in Notes, then delete it after confirming', async ({ page }) => {
  const frame = await openReader(page)
  await selectText(frame, 20)
  await page.getByRole('button', { name: 'Highlight in green' }).click()

  await page.getByRole('button', { name: 'Highlights and bookmarks' }).click()
  const panel = page.getByRole('complementary', { name: 'notes' })
  await expect(panel).toContainText('Highlights 1')
  await expect(panel).toContainText('The keeper kept a lo')

  await panel.locator('li').hover()
  await panel.getByRole('button', { name: 'Delete' }).click()
  await expect(page.getByRole('alertdialog')).toContainText('Delete this highlight?')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(panel).toContainText('Highlights 1')

  await panel.locator('li').hover()
  await panel.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete highlight' }).click()
  await expect(panel).toContainText('Highlights 0')
})

test('a highlight with a note', async ({ page }) => {
  const frame = await openReader(page)
  await selectText(frame, 14)
  await page.getByRole('toolbar', { name: 'Selection' }).getByRole('button', { name: 'Note', exact: true }).click()
  await page.getByPlaceholder('Write a note…').fill('Lovely opening')
  await page.getByRole('button', { name: 'Save' }).click()
  await page.getByRole('button', { name: 'Highlights and bookmarks' }).click()
  await expect(page.getByRole('complementary', { name: 'notes' })).toContainText('Lovely opening')
})

test('bookmark the page; removing it asks first', async ({ page }) => {
  await openReader(page)
  const button = page.getByRole('button', { name: 'Bookmark this page' })
  await button.click()
  const marked = page.getByRole('button', { name: 'Remove bookmark' })
  await expect(marked).toHaveAttribute('aria-pressed', 'true')

  await marked.click()
  await expect(page.getByRole('alertdialog')).toContainText('Remove this bookmark?')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Remove bookmark' }).click()
  await expect(page.getByRole('button', { name: 'Bookmark this page' })).toHaveAttribute('aria-pressed', 'false')
})

test('the Notes screen lists, filters, opens and deletes highlights', async ({ page }) => {
  const frame = await openReader(page)
  await selectText(frame, 20)
  await page.getByRole('button', { name: 'Highlight in pink' }).click()
  await page.getByRole('button', { name: 'Back to the library' }).click()

  await page.getByRole('button', { name: /^Notes/ }).click()
  const quote = page.locator('article.quote')
  await expect(quote).toHaveCount(1)
  await expect(quote).toContainText('The keeper kept a lo')

  // Colour and text filters.
  await page.getByRole('radio', { name: 'Blue' }).click()
  await expect(page.getByText('Nothing matches these filters.')).toBeVisible()
  await page.getByRole('radio', { name: 'Pink' }).click()
  await expect(quote).toHaveCount(1)
  await page.getByRole('radio', { name: 'Pink' }).click()
  await page.getByPlaceholder('Search quotes and notes').fill('windowsill')
  await expect(page.getByText('Nothing matches these filters.')).toBeVisible()
  await page.getByPlaceholder('Search quotes and notes').fill('keeper')
  await expect(quote).toHaveCount(1)

  // Opens the book at the quote.
  await quote.locator('.body').click()
  await expect(page.getByRole('dialog', { name: /^Reading/ })).toBeVisible()
  await page.getByRole('button', { name: 'Back to the library' }).click()

  // Deleting asks first.
  await quote.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(quote).toHaveCount(1)
  await quote.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete highlight' }).click()
  await expect(quote).toHaveCount(0)
})

test('define a word: offline first, Wiktionary as a fallback', async ({ page }) => {
  const frame = await openReader(page)
  // "keeper" (characters 4–10 of "The keeper kept a logbook").
  await selectText(frame, 10, 4)
  await page.getByRole('toolbar', { name: 'Selection' }).getByRole('button', { name: 'Define' }).click()
  const card = page.getByRole('dialog', { name: 'Definition of keeper' })
  await expect(card).toContainText('A person who looks after something')
  await expect(card).toContainText('Test English Dictionary')
  await page.keyboard.press('Escape')
  await expect(card).toHaveCount(0)

  // "kept" is not in the offline dictionary: Wiktionary answers.
  await selectText(frame, 15, 11)
  await page.getByRole('toolbar', { name: 'Selection' }).getByRole('button', { name: 'Define' }).click()
  const online = page.getByRole('dialog', { name: 'Definition of kept' })
  await expect(online).toContainText('A test sense of kept.')
  await expect(online).toContainText('Wiktionary')
  await expect(online.locator('b, i')).toHaveCount(0)
})

test('long selections offer no Define button', async ({ page }) => {
  const frame = await openReader(page)
  await selectText(frame, 25)
  const menu = page.getByRole('toolbar', { name: 'Selection' })
  await expect(menu.getByRole('button', { name: 'Note', exact: true })).toBeVisible()
  await expect(menu.getByRole('button', { name: 'Define' })).toHaveCount(0)
})
