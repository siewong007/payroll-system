import '@testing-library/jest-dom';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';
// Preload the English bundle synchronously so tests reading i18n.t() during
// module/component setup render localized copy immediately.
import en from '@/i18n/locales/en';
import i18n, { i18nReady } from '@/i18n';

i18n.addResourceBundle('en', 'translation', en, true, true);
await i18nReady;
if (i18n.language !== 'en') {
  await i18n.changeLanguage('en');
}

afterEach(() => {
  // A test that switches language must not leak the locale into the next one.
  if (i18n.language !== 'en') void i18n.changeLanguage('en');
});

// Turnstile is opt-in via VITE_TURNSTILE_SITE_KEY; a developer's .env.local
// must not change what components render in tests. Turnstile.test.tsx
// re-stubs the key to exercise the widget itself.
vi.stubEnv('VITE_TURNSTILE_SITE_KEY', '');

const storage = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => {
      store[key] = value;
    },
    removeItem: (key: string) => {
      delete store[key];
    },
    clear: () => {
      store = {};
    },
  };
})();

Object.defineProperty(window, 'localStorage', {
  value: storage,
  configurable: true,
});
Object.defineProperty(globalThis, 'localStorage', {
  value: storage,
  configurable: true,
});

Object.defineProperty(window, 'scrollTo', {
  value: () => {},
  writable: true,
  configurable: true,
});

// Automatically cleanup after each test
afterEach(() => {
  cleanup();
  localStorage.clear();
});
