import { useCallback, useEffect, useRef } from 'react';
import type { TurnstileWidgetRef } from '@/components/TurnstileWidget';

export function turnstileEnabled(): boolean {
  return Boolean(import.meta.env.VITE_TURNSTILE_SITE_KEY);
}

/**
 * Coordinates single-use Cloudflare Turnstile tokens with async verification.
 * Waiters resolve cleanly on verify, expire/error, timeout, or unmount.
 */
export function useTurnstileWaiter(timeoutMs = 5_000) {
  const widgetRef = useRef<TurnstileWidgetRef>(null);
  const tokenRef = useRef<string | undefined>(undefined);
  const waiterRef = useRef<(() => void) | null>(null);
  const timerRef = useRef<number | null>(null);

  const clearPendingWaiter = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    waiterRef.current = null;
  }, []);

  useEffect(() => clearPendingWaiter, [clearPendingWaiter]);

  const takeToken = useCallback(() => {
    const token = tokenRef.current;
    tokenRef.current = undefined;
    widgetRef.current?.reset();
    return token;
  }, []);

  const waitForToken = useCallback((): Promise<string | undefined> => {
    if (!turnstileEnabled() || tokenRef.current) {
      clearPendingWaiter();
      return Promise.resolve(takeToken());
    }
    clearPendingWaiter();
    return new Promise((resolve) => {
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        waiterRef.current = null;
        resolve(takeToken());
      }, timeoutMs);
      waiterRef.current = () => {
        clearPendingWaiter();
        resolve(takeToken());
      };
    });
  }, [clearPendingWaiter, takeToken, timeoutMs]);

  const handleVerify = useCallback((token: string) => {
    tokenRef.current = token;
    waiterRef.current?.();
  }, []);

  const handleExpire = useCallback(() => {
    tokenRef.current = undefined;
  }, []);

  const handleError = useCallback(() => {
    tokenRef.current = undefined;
    waiterRef.current?.();
  }, []);

  const resetWidget = useCallback(() => {
    clearPendingWaiter();
    tokenRef.current = undefined;
    widgetRef.current?.reset();
  }, [clearPendingWaiter]);

  return {
    widgetRef,
    takeToken,
    waitForToken,
    handleVerify,
    handleExpire,
    handleError,
    resetWidget,
  };
}
