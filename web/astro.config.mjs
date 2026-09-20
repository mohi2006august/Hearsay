import { defineConfig } from 'astro/config';

// The dashboard is a static bundle that talks to hearsay-api over HTTP.
// Nothing is server-rendered: there is no session, no secret, and the API is
// the single source of truth for every number on the page.
export default defineConfig({
  output: 'static',
  devToolbar: { enabled: false },
  build: {
    inlineStylesheets: 'auto',
  },
  vite: {
    server: {
      // In dev, proxy /api to the Rust service so the browser stays
      // same-origin and CORS never enters the picture. In a deployment,
      // set PUBLIC_HEARSAY_API or put both behind one origin.
      proxy: {
        '/api': { target: 'http://127.0.0.1:8787', changeOrigin: true },
        '/healthz': { target: 'http://127.0.0.1:8787', changeOrigin: true },
      },
    },
  },
});
