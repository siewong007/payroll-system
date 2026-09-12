/**
 * Whether a Turnstile site key is configured — mirrors the backend's
 * secret check, so pages can skip token-dependent calls when the widget
 * will never render (local dev, CI without keys).
 */
export function turnstileEnabled(): boolean {
  return Boolean(import.meta.env.VITE_TURNSTILE_SITE_KEY);
}
