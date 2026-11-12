// pdf.js setup. Books are read in ranges from the `book://` scheme, so a
// large PDF never has to be loaded whole.

import * as pdfjs from 'pdfjs-dist'
import type { PDFDocumentProxy } from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { bookUrl } from '../lib/api'
import type { BookId } from '../lib/types'

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl

export { pdfjs }
export type { PDFDocumentProxy }

const ASSETS = '/pdfjs/'
const FIRST_CHUNK = 128 * 1024

async function fetchRange(id: BookId, begin: number, end: number) {
  const res = await fetch(bookUrl(id), { headers: { Range: `bytes=${begin}-${end - 1}` } })
  if (!res.ok) throw new Error((await res.text()) || `HTTP ${res.status}`)
  const bytes = new Uint8Array(await res.arrayBuffer())
  // A server without range support answers with the whole file.
  const total = Number(res.headers.get('Content-Range')?.split('/')[1] ?? bytes.length)
  return { bytes: res.status === 206 ? bytes : bytes.subarray(begin, end), total }
}

/** Opens a book's PDF, fetching only the parts pdf.js asks for. */
export async function openPdf(id: BookId): Promise<PDFDocumentProxy> {
  const first = await fetchRange(id, 0, FIRST_CHUNK)
  const transport = new pdfjs.PDFDataRangeTransport(first.total, first.bytes)
  transport.requestDataRange = (begin: number, end: number) => {
    fetchRange(id, begin, end)
      .then(({ bytes }) => transport.onDataRange(begin, bytes))
      .catch((e) => console.error('could not read part of the PDF', e))
  }
  return pdfjs.getDocument({
    range: transport,
    cMapUrl: `${ASSETS}cmaps/`,
    cMapPacked: true,
    standardFontDataUrl: `${ASSETS}standard_fonts/`,
    wasmUrl: `${ASSETS}wasm/`,
    enableXfa: false,
    disableAutoFetch: true,
    disableStream: true,
  }).promise
}

/** Renders page 1 to a JPEG of the given width (for covers). */
export async function renderCover(doc: PDFDocumentProxy, width = 360): Promise<Uint8Array> {
  const page = await doc.getPage(1)
  const base = page.getViewport({ scale: 1 })
  const viewport = page.getViewport({ scale: width / base.width })
  const canvas = document.createElement('canvas')
  canvas.width = Math.round(viewport.width)
  canvas.height = Math.round(viewport.height)
  await page.render({ canvas, viewport }).promise
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.85))
  page.cleanup()
  if (!blob) throw new Error('could not encode the cover')
  return new Uint8Array(await blob.arrayBuffer())
}
