import { defineConfig } from 'vite'
import { devtools } from '@tanstack/devtools-vite'

import { tanstackStart } from '@tanstack/react-start/plugin/vite'

import viteReact from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { cloudflare } from '@cloudflare/vite-plugin'

const config = defineConfig(({ command }) => ({
  resolve: { tsconfigPaths: true },
  plugins: [
    devtools(),
    cloudflare({
      viteEnvironment: { name: 'ssr' },
      ...(command === 'serve' && {
        // Locally the API runs inside this dev server, so the `API` service binding
        // (AdminEntrypoint) resolves in-process. Its state is shared with
        // `wrangler dev` in apps/api, so both see the same local D1 database.
        auxiliaryWorkers: [{ configPath: '../api/wrangler.jsonc' }],
        persistState: { path: '../api/.wrangler/state' },
      }),
    }),
    tailwindcss(),
    tanstackStart(),
    viteReact(),
  ],
}))

export default config
