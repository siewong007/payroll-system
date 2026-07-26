import type { ReactNode } from 'react';
import { useLocation } from 'react-router';
import { motion, useReducedMotion } from 'framer-motion';

/**
 * Animates route content in on navigation (fade + rise). Keyed by pathname so
 * every route change re-triggers; entrance-only to keep navigation snappy.
 */
export function PageTransition({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  const reduceMotion = useReducedMotion();

  return (
    <motion.div
      key={pathname}
      initial={reduceMotion ? false : { opacity: 0, y: 14 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.32, ease: [0.22, 1, 0.36, 1] }}
    >
      {children}
    </motion.div>
  );
}
