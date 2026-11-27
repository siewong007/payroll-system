/// <reference types="vitest" />
import { defineConfig } from 'vitest/config'
import { loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

function cspConnectSrcPlugin(mode: string) {
  const env = loadEnv(mode, import.meta.dirname, 'VITE_')
  let apiOrigin = 'https://api.payrollmy.com'
  if (env.VITE_API_URL) {
    try {
      const parsed = new URL(env.VITE_API_URL)
      if (parsed.protocol === 'https:' || parsed.protocol === 'http:') {
        apiOrigin = parsed.origin
      }
    } catch {
      // Relative VITE_API_URL uses 'self'.
    }
  }
  return {
    name: 'csp-connect-src',
    transformIndexHtml(html: string, ctx?: { server?: unknown }) {
      const devSources = ctx?.server
        ? ' http://localhost:* ws://localhost:* http://127.0.0.1:* ws://127.0.0.1:*'
        : ''
      const connectSrc = `connect-src 'self'${devSources} ${apiOrigin} https://challenges.cloudflare.com https://cloudflareinsights.com;`
      return html.replace(/connect-src\s+[^;]+;/, connectSrc)
    },
  }
}

export default defineConfig(({ mode }) => ({
  plugins: [react(), tailwindcss(), cspConnectSrcPlugin(mode)],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              debugName: 'app-chunks',
              name(moduleId) {
                if (moduleId.includes('/src/i18n/locales/en')) return 'locale-en'
                if (moduleId.includes('/src/i18n/locales/ms')) return 'locale-ms'
                if (moduleId.includes('/src/i18n/locales/zh-CN')) return 'locale-zh-cn'
                if (moduleId.includes('/src/i18n/locales/zh-TW')) return 'locale-zh-tw'
                if (moduleId.includes('/src/i18n/serverErrorMap')) return 'server-error-map'
                if (moduleId.includes('node_modules')) {
                  if (moduleId.includes('html5-qrcode')) return 'vendor-qr'
                  if (moduleId.includes('/axios/')) return 'vendor-http'
                  if (moduleId.includes('lucide-react')) return 'vendor-icons'
                  if (moduleId.includes('@tanstack')) return 'vendor-tanstack'
                  if (
                    moduleId.includes('/react/') ||
                    moduleId.includes('/react-dom/') ||
                    moduleId.includes('/react-router') ||
                    moduleId.includes('/scheduler/')
                  ) {
                    return 'vendor-react'
                  }
                }
                return null
              },
            },
          ],
        },
      },
    },
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/tests/setup.ts',
    testTimeout: 15_000,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/tests/**', 'src/types/**'],
    },
  },
}))
