import { useEffect, useState } from 'react';

/**
 * Tracks a CSS media query cleanly, returning `defaultMatches` when
 * `window.matchMedia` is unavailable (SSR / JSDOM environments).
 */
export function useMediaQuery(query: string, defaultMatches = true): boolean {
  const [matches, setMatches] = useState<boolean>(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return defaultMatches;
    }
    return window.matchMedia(query).matches;
  });

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return;
    }
    const media = window.matchMedia(query);
    setMatches(media.matches);
    const onChange = (event: MediaQueryListEvent) => setMatches(event.matches);
    if (typeof media.addEventListener === 'function') {
      media.addEventListener('change', onChange);
      return () => media.removeEventListener('change', onChange);
    }
    media.addListener?.(onChange);
    return () => media.removeListener?.(onChange);
  }, [query]);

  return matches;
}
