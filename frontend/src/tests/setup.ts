import '@testing-library/jest-dom';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

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

// Automatically cleanup after each test
afterEach(() => {
  cleanup();
  localStorage.clear();
});
