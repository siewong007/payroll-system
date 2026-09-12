import { useEffect, useImperativeHandle, useRef } from 'react';

/**
 * Cloudflare Turnstile widget, loaded straight from
 * challenges.cloudflare.com — no wrapper dependency. Renders nothing when
 * VITE_TURNSTILE_SITE_KEY is unset, matching the backend's
 * skip-when-unconfigured behaviour, so dev/CI never see a captcha.
 *
 * Tokens are single-use and expire (~300 s): after every request that sent
 * one, the parent must call `ref.current?.reset()` so the widget mints a
 * fresh token for the next attempt.
 */

const SCRIPT_SRC = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
const SCRIPT_SELECTOR = 'script[data-turnstile-widget]';

interface TurnstileApi {
  render: (container: HTMLElement, options: Record<string, unknown>) => string;
  reset: (widgetId: string) => void;
  remove: (widgetId: string) => void;
  getResponse: (widgetId: string) => string | undefined;
}

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

export interface TurnstileWidgetRef {
  reset: () => void;
}

interface TurnstileWidgetProps {
  onVerify: (token: string) => void;
  onExpire?: () => void;
  onError?: () => void;
  ref?: React.Ref<TurnstileWidgetRef>;
}

export function TurnstileWidget({ onVerify, onExpire, onError, ref }: TurnstileWidgetProps) {
  // Read per render (not module scope) so vi.stubEnv works in tests.
  const sitekey = import.meta.env.VITE_TURNSTILE_SITE_KEY as string | undefined;
  const containerRef = useRef<HTMLDivElement>(null);
  const widgetId = useRef<string | null>(null);
  // Latest callbacks — the widget is rendered once and must not be torn down
  // and re-created every time a parent re-renders with a new closure.
  const callbacks = useRef({ onVerify, onExpire, onError });
  callbacks.current = { onVerify, onExpire, onError };

  useImperativeHandle(
    ref,
    () => ({
      reset: () => {
        if (widgetId.current != null) {
          window.turnstile?.reset(widgetId.current);
        }
      },
    }),
    [],
  );

  useEffect(() => {
    if (!sitekey) return;
    let cancelled = false;

    const render = () => {
      if (cancelled || widgetId.current != null || !containerRef.current || !window.turnstile) {
        return;
      }
      widgetId.current = window.turnstile.render(containerRef.current, {
        sitekey,
        callback: (token: string) => callbacks.current.onVerify(token),
        'expired-callback': () => callbacks.current.onExpire?.(),
        'error-callback': () => callbacks.current.onError?.(),
      });
    };

    let script: HTMLScriptElement | null = null;
    if (window.turnstile) {
      render();
    } else {
      script = document.querySelector<HTMLScriptElement>(SCRIPT_SELECTOR);
      if (!script) {
        script = document.createElement('script');
        script.src = SCRIPT_SRC;
        script.async = true;
        script.defer = true;
        script.dataset.turnstileWidget = 'true';
        document.head.appendChild(script);
      }
      script.addEventListener('load', render);
      // The script may have finished loading between the turnstile check and
      // the listener attach — render immediately if so.
      render();
    }

    return () => {
      cancelled = true;
      script?.removeEventListener('load', render);
      if (widgetId.current != null) {
        window.turnstile?.remove(widgetId.current);
        widgetId.current = null;
      }
    };
  }, [sitekey]);

  if (!sitekey) return null;
  return <div ref={containerRef} />;
}
