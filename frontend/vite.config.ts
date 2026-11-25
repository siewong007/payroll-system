/// <reference types="vitest" />
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
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
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/tests/**', 'src/types/**'],
    },
  },
})
