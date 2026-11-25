import type { ReactNode } from 'react';
import { useLocation } from 'react-router';

/**
 * Subtle rise transition on route change. Keyed by pathname so every route
 * change re-triggers without starting at opacity 0 (keeping LCP unblocked).
 */
export function PageTransition({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();

  return (
    <div key={pathname} className="animate-page-enter">
      {children}
    </div>
  );
}
