import {defineConfig} from 'vitest/config'

export default defineConfig({
  server: {
    host: '0.0.0.0',
    watch: {
      usePolling: true,
    },
  },
  test: {
    setupFiles: ['vitest-browser-react', './setup-tests.ts'],
    browser: {
      enabled: false,
      headless: true,
      name: 'chromium',
      provider: 'playwright',
    },
    include: ['**/*.test.ts', '**/*.test.tsx'],
    exclude: ['node_modules', 'dist', 'public'],
    environment: 'jsdom',
  },
})
