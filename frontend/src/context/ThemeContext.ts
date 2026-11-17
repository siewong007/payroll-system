import { createContext, useContext } from 'react';

export type AppearanceMode = 'light' | 'dark' | 'system';

export interface ThemeContextValue {
  mode: AppearanceMode;
  resolvedTheme: 'light' | 'dark';
  setMode: (mode: AppearanceMode) => void;
  toggleTheme: () => void;
}

export const ThemeContext = createContext<ThemeContextValue>({
  mode: 'light',
  resolvedTheme: 'light',
  setMode: () => undefined,
  toggleTheme: () => undefined,
});

export function useAppearance(): ThemeContextValue {
  return useContext(ThemeContext);
}
