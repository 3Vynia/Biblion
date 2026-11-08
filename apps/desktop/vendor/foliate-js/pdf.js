// Biblion: PDF support is intentionally stubbed out. Biblion renders PDFs with
// its own pdf.js integration, so foliate's bundled copy of pdf.js (13 MB) is
// not vendored.
export const makePDF = async () => {
    throw new Error('PDF files are opened with the PDF reader, not foliate-js')
}
