import { type ClassValue, clsx } from 'clsx';
import i18n from '@/i18n';
import { SERVER_ERROR_KEYS, SERVER_ERROR_PATTERNS } from '@/i18n/serverErrorMap';
import { formatDate as formatDateLocalized, formatMYR as formatMYRLocalized, formatRelativeTime as formatRelativeTimeLocalized } from '@/lib/format';

// Simple cn utility without tailwind-merge for now
export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

/** Format sen (cents) to MYR display: "RM 1,234.56" (locale-aware digits). */
export function formatMYR(sen: number): string {
  return formatMYRLocalized(sen);
}

/** Format date to the active locale's short form (en: DD/MM/YYYY). */
export function formatDate(date: string | Date): string {
  return formatDateLocalized(date);
}
/**
 * Format an instant for a `datetime-local` input, which expects local wall time.
 *
 * `toISOString().slice(0, 16)` looks right but yields UTC wall time, so in
 * Asia/Kuala_Lumpur (+08:00) it shows a time 8 hours earlier than the clock.
 * A form that then submits `new Date(value).toISOString()` parses that value as
 * local, so opening and saving an unchanged record shifts the stored instant by
 * the offset every time. Pair this helper with that submit path to round-trip.
 */
export function toDateTimeLocalValue(date: string | Date): string {
  const d = new Date(date);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Today's date as YYYY-MM-DD in the viewer's local timezone (not UTC). */
export function todayLocalDate(): string {
  return toDateTimeLocalValue(new Date()).slice(0, 10);
}

/**
 * Validates a post-login `?redirect=` target, returning null if it is unsafe.
 *
 * Accepts only same-origin paths: exactly one leading slash. That rejects
 * protocol-relative targets like `//evil.com` (which the browser treats as an
 * absolute URL) and any absolute URL, so the param cannot be used as an open
 * redirect. The kiosk scan flow produces this value from
 * `location.pathname + location.search`, which always satisfies the rule.
 */
export function safeRedirectPath(raw: string | null | undefined): string | null {
  if (!raw) return null;
  if (!raw.startsWith('/')) return null;
  if (raw.startsWith('//')) return null;
  return raw;
}

/**
 * Whether an attachment URL names an image, and can therefore be previewed
 * inline rather than offered as a download.
 *
 * Matches on the stored URL, which for an API-served upload still ends in the
 * uploader's original extension even though the file is fetched as a blob.
 */
export function isImageUrl(url: string): boolean {
  return /\.(jpg|jpeg|png|gif|webp|bmp|svg)(\?|$)/i.test(url);
}

/**
 * Extract error message from Axios-style or standard Error objects.
 *
 * A `response.data.error` string emitted by the backend is first looked up in
 * SERVER_ERROR_KEYS (exact English literal → i18n key) so stable server
 * messages render in the active locale; unrecognized server text passes
 * through verbatim rather than being replaced by a vaguer local fallback.
 * The `fallback` argument should already be a translated string from `t()`.
 */
/**
 * Translate a raw server-emitted message through SERVER_ERROR_KEYS /
 * SERVER_ERROR_PATTERNS. Unknown text passes through verbatim.
 */
export function translateServerMessage(msg: string): string {
  const key = SERVER_ERROR_KEYS[msg];
  if (key) return i18n.t(key);
  for (const { re, key: pKey, args } of SERVER_ERROR_PATTERNS) {
    const m = msg.match(re);
    if (m) {
      const vars = Object.fromEntries(args.map((a, i) => [a, m[i + 1]]));
      return i18n.t(pKey, vars);
    }
  }
  return msg;
}

export function getErrorMessage(err: unknown, fallback?: string): string {
  const resolvedFallback = fallback ?? i18n.t('errors.actionFailed');
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const axiosErr = err as { response: { data?: { error?: string } } };
    const serverMsg = axiosErr.response?.data?.error;
    if (serverMsg) return translateServerMessage(serverMsg);
  }
  if (err instanceof Error) return err.message;
  return resolvedFallback;
}

/** "just now" / localized relative time via Intl.RelativeTimeFormat. */
export function formatRelativeTime(date: string | Date): string {
  return formatRelativeTimeLocalized(date);
}
