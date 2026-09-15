import i18n, { intlLocale } from '@/i18n';

/**
 * Locale-aware formatting. Every helper reads the active i18n language at call
 * time, so components re-rendered by a language switch pick up the new locale
 * without plumbing. Currency stays MYR (the transaction currency) — only the
 * digits and grouping follow the UI locale.
 */

const DATE_SHORT: Intl.DateTimeFormatOptions = {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
};

const DATE_LONG: Intl.DateTimeFormatOptions = {
  day: 'numeric',
  month: 'long',
  year: 'numeric',
};

const DATE_MEDIUM: Intl.DateTimeFormatOptions = {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
};

const DATE_DAY_MONTH: Intl.DateTimeFormatOptions = {
  day: 'numeric',
  month: 'short',
};

const DATE_WEEKDAY_LONG: Intl.DateTimeFormatOptions = {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
};

const DATETIME: Intl.DateTimeFormatOptions = {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
};

const TIME: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' };

const TIME_SECONDS: Intl.DateTimeFormatOptions = {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
};

const TIME_HOUR: Intl.DateTimeFormatOptions = { hour: 'numeric', minute: '2-digit' };

function toDate(value: string | Date): Date {
  return value instanceof Date ? value : new Date(value);
}

/** DD/MM/YYYY-style short date (locale-ordered). */
export function formatDate(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), DATE_SHORT).format(toDate(value));
}

/** "16 September 2026" — locale long form. */
export function formatDateLong(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), DATE_LONG).format(toDate(value));
}

/** "16 Sep 2026" — locale medium form. */
export function formatDateMedium(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), DATE_MEDIUM).format(toDate(value));
}

/** "16 Sep" — no year, for compact ranges. */
export function formatDateDayMonth(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), DATE_DAY_MONTH).format(toDate(value));
}

/** "Wednesday, 16 September". */
export function formatDateWeekday(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), DATE_WEEKDAY_LONG).format(toDate(value));
}

/** Date + time, e.g. "16 Sep 2026, 02:30 PM". */
export function formatDateTime(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), DATETIME).format(toDate(value));
}

/** "02:30 PM" / "14:30" depending on locale conventions. */
export function formatTime(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), TIME).format(toDate(value));
}

export function formatTimeSeconds(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), TIME_SECONDS).format(toDate(value));
}

/** Non-padded hour form ("2:30 PM") used by attendance displays. */
export function formatTimeHour(value: string | Date): string {
  return new Intl.DateTimeFormat(intlLocale(), TIME_HOUR).format(toDate(value));
}

/** Grouped number, no decimals: 1,234 / 1.234 / 1,234 per locale. */
export function formatNumber(value: number): string {
  return new Intl.NumberFormat(intlLocale()).format(value);
}

/** Localized month name for a 1-12 month number ("September" / "九月"). */
export function monthName(month: number, style: 'short' | 'long' = 'long'): string {
  return new Intl.DateTimeFormat(intlLocale(), { month: style })
    .format(new Date(2000, month - 1, 1));
}

/** Localized weekday name for a JS day-of-week (0 = Sunday). */
export function weekdayName(day: number, style: 'short' | 'long' = 'short'): string {
  // 2000-01-02 was a Sunday, so day N lands on weekday N.
  return new Intl.DateTimeFormat(intlLocale(), { weekday: style })
    .format(new Date(2000, 0, 2 + day));
}

/** "September 2026" / "2026年9月" — a payroll period (month + year). */
export function formatPeriod(year: number, month: number): string {
  return i18n.t('payroll.period', {
    year,
    month: monthName(month),
    monthNum: month,
  });
}

/** Sen → "RM 1,234.56". The currency is fixed MYR; only formatting localizes. */
export function formatMYR(sen: number): string {
  const ringgit = sen / 100;
  return `RM ${new Intl.NumberFormat(intlLocale(), {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(ringgit)}`;
}

/**
 * "just now", "5 min ago", "3 hr ago", "2 days ago", then the absolute date —
 * translated via the `time.*` keys (the terse copy is deliberate UI style,
 * not a truncation of RelativeTimeFormat's "5 minutes ago").
 */
export function formatRelativeTime(value: string | Date): string {
  const seconds = Math.max(0, Math.floor((Date.now() - toDate(value).getTime()) / 1000));
  if (seconds < 60) return i18n.t('time.justNow');
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return i18n.t('time.minutesAgo', { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return i18n.t('time.hoursAgo', { count: hours });
  const days = Math.floor(hours / 24);
  if (days < 30) return i18n.t('time.daysAgo', { count: days });
  return formatDate(value);
}
