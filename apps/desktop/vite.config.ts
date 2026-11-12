import fs from 'node:fs'
import path from 'node:path'
import { defineConfig, type Plugin } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

/**
 * Serves pdf.js data files (character maps, standard fonts, WebAssembly
 * decoders) under /pdfjs/ in development and copies them into the build.
 */
function pdfjsAssets(): Plugin {
  const root = path.resolve('node_modules/pdfjs-dist')
  const dirs = ['cmaps', 'standard_fonts', 'wasm']
  const types: Record<string, string> = { '.wasm': 'application/wasm', '.js': 'text/javascript' }
  let outDir = 'dist'
  return {
    name: 'biblion-pdfjs-assets',
    configResolved(config) {
      outDir = path.resolve(config.root, config.build.outDir)
    },
    configureServer(server) {
      server.middlewares.use('/pdfjs', (req, res, next) => {
        const rel = decodeURIComponent((req.url ?? '').split('?')[0]).replace(/^\/+/, '')
        const file = path.resolve(root, rel)
        const allowed = dirs.some((d) => file.startsWith(path.join(root, d) + path.sep))
        if (!allowed || !fs.existsSync(file) || !fs.statSync(file).isFile()) return next()
        res.setHeader('Content-Type', types[path.extname(file)] ?? 'application/octet-stream')
        fs.createReadStream(file).pipe(res)
      })
    },
    closeBundle() {
      for (const d of dirs) fs.cpSync(path.join(root, d), path.join(outDir, 'pdfjs', d), { recursive: true })
    },
  }
}

// Tauri expects a fixed port and the WebKitGTK/WKWebView feature set.
export default defineConfig({
  plugins: [svelte(), pdfjsAssets()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    target: 'safari16',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    chunkSizeWarningLimit: 1500,
  },
  test: {
    include: ['src/**/*.test.ts'],
  },
})
