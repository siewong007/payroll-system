import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { ThemeContext, type AppearanceMode, type ThemeContextValue } from './ThemeContext';

const STORAGE_KEY = 'payroll-appearance';

function readStoredMode(): AppearanceMode {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === 'light' || raw === 'dark' || raw === 'system') {
      return raw;
    }
  } catch {
    // Ignore storage access errors in restricted/test environments.
  }
  return 'light';
}

function detectSystemDark(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return false;
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<AppearanceMode>(() => readStoredMode());
  const [systemDark, setSystemDark] = useState<boolean>(() => detectSystemDark());

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    media.addEventListener?.('change', onChange);
    return () => media.removeEventListener?.('change', onChange);
  }, []);

  const resolvedTheme: 'light' | 'dark' =
    mode === 'system' ? (systemDark ? 'dark' : 'light') : mode;

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle('dark', resolvedTheme === 'dark');
    const themeColorMeta = document.querySelector('meta[name="theme-color"]');
    if (themeColorMeta) {
      themeColorMeta.setAttribute('content', resolvedTheme === 'dark' ? '#0b1120' : '#ffffff');
    }
  }, [resolvedTheme]);

  const setMode = useCallback((next: AppearanceMode) => {
    setModeState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Ignore storage access errors.
    }
  }, []);

  const toggleTheme = useCallback(() => {
    setMode(resolvedTheme === 'dark' ? 'light' : 'dark');
  }, [resolvedTheme, setMode]);

  const value = useMemo<ThemeContextValue>(
    () => ({ mode, resolvedTheme, setMode, toggleTheme }),
    [mode, resolvedTheme, setMode, toggleTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
