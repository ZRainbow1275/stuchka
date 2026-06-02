import { defineConfig } from 'vite';
import { resolve } from 'node:path';

// Single UMD bundle for the WebView2 host (channel A only; no network, no externals).
// Everything (Tiptap / Yjs / ProseMirror / Lucide SVGs) is inlined so the page can run
// fully offline under CSP `connect-src 'none'`.
export default defineConfig({
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: true,
    lib: {
      entry: resolve(__dirname, 'src/main.ts'),
      name: 'StuchkaEditor',
      formats: ['umd'],
      fileName: () => 'stuchka-editor.umd.js',
    },
    rollupOptions: {
      // No externals: the bundle must be self-contained for the file:/// WebView load.
      output: {
        inlineDynamicImports: true,
      },
    },
  },
  test: {
    globals: true,
    // jsdom provides `document` for the Tiptap editor mount (18-extension assertion).
    environment: 'jsdom',
    include: ['test/**/*.test.ts'],
  },
});
