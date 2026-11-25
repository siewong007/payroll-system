import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import { i18nReady } from '@/i18n'
import App from './App'
import { ErrorBoundary } from './components/ErrorBoundary'
import { initPwaListeners } from '@/lib/pwa'

// Apply stored theme before first paint (CSP forbids inline scripts in index.html).
try {
  const mode = localStorage.getItem('payroll-appearance')
  const prefersDark =
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
  const isDark = mode === 'dark' || (mode === 'system' && prefersDark)
  if (isDark) {
    document.documentElement.classList.add('dark')
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', '#0b1120')
  }
} catch {
  // Ignore storage access errors.
}

initPwaListeners()

// Wait for the single detected locale chunk before initial mount so the first
// paint shows localized copy without preloading all four language bundles.
void i18nReady.finally(() => {
  // Outermost boundary: catches provider- and router-level throws that no
  // boundary inside App can see. The one around the routes is what keeps the
  // shell alive when a single page fails; this one is the last resort.
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </StrictMode>,
  )
})
