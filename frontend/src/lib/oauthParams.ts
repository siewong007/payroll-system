/**
 * Captures single-use OAuth2 query/fragment parameters on first non-empty read
 * and immediately strips them from the URL so credentials never linger in history,
 * while keeping them in module memory across React StrictMode remounts.
 */
export function createSingleReadUrlParams(source: 'hash' | 'search'): {
  take: () => URLSearchParams;
  clear: () => void;
} {
  let memoized: URLSearchParams | null = null;

  return {
    take() {
      const raw = source === 'hash' ? window.location.hash.substring(1) : window.location.search;
      const incoming = new URLSearchParams(raw);
      if (Array.from(incoming.keys()).length > 0) {
        memoized = incoming;
        window.history.replaceState(null, '', window.location.pathname);
      }
      return memoized ?? incoming;
    },
    clear() {
      memoized = null;
    },
  };
}
